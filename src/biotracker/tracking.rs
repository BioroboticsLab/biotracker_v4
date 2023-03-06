use super::{arena::ArenaImpl, protocol::*, State};
use anyhow::Result;

async fn tracking_task(
    image: Image,
    mut detector: FeatureDetectorClient<tonic::transport::Channel>,
    mut matcher: MatcherClient<tonic::transport::Channel>,
    arena: ArenaImpl,
    last_entities: Entities,
) -> Result<(u32, Features, Entities)> {
    let frame_number = image.frame_number;
    let detector_request = DetectorRequest {
        image: Some(image),
        arena: Some(arena.arena.clone()),
    };
    let mut features = detector
        .detect_features(detector_request)
        .await?
        .into_inner();

    arena.features_to_poses(&mut features)?;

    let matcher_request = MatcherRequest {
        features: Some(features.clone()),
        last_entities: Some(last_entities),
        frame_number,
    };
    let entities = matcher.match_features(matcher_request).await?.into_inner();
    Ok((frame_number, features, entities))
}

pub fn start_tracking_task(
    state: &State,
    entity_switch_request: Option<EntityIdSwitch>,
    task_handle: &mut Option<tokio::task::JoinHandle<()>>,
    tracking_tx: &tokio::sync::mpsc::Sender<Result<(u32, Features, Entities)>>,
    image: &Image,
) {
    let image = image.clone();
    let detector = state.connections.feature_detector().clone();
    let matcher = state.connections.matcher().clone();
    let last_entities = state
        .experiment
        .last_entities
        .as_ref()
        .expect("last_entities is None");

    let mut tracking_entities = Entities::default();
    for id in &state.experiment.entity_ids {
        if let Some(entity) = last_entities.entities.iter().find(|e| e.id == *id) {
            tracking_entities.entities.push(entity.clone());
        } else {
            tracking_entities.entities.push(Entity {
                id: *id,
                feature: None,
                frame_number: 0,
            });
        }
    }

    if let Some(switch_request) = entity_switch_request {
        let (mut first_idx, mut second_idx) = (None, None);
        for (idx, entity) in tracking_entities.entities.iter().enumerate() {
            if entity.id == switch_request.id1 {
                first_idx = Some(idx);
            }
            if entity.id == switch_request.id2 {
                second_idx = Some(idx);
            }
        }

        if let (Some(first_idx), Some(second_idx)) = (first_idx, second_idx) {
            tracking_entities.entities[first_idx].id = switch_request.id2;
            tracking_entities.entities[second_idx].id = switch_request.id1;
        }
    }

    let arena = state.arena_impl.clone();
    let tracking_tx = tracking_tx.clone();
    *task_handle = Some(tokio::spawn(async move {
        let result = tracking_task(image, detector, matcher, arena, tracking_entities).await;
        tracking_tx.send(result).await.unwrap();
    }));
}
