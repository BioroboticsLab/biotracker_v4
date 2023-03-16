use super::{arena::ArenaImpl, protocol::*, undistort::UndistortMap, State};
use anyhow::Result;

async fn tracking_task(
    image: Image,
    mut detector: FeatureDetectorClient<tonic::transport::Channel>,
    mut matcher: MatcherClient<tonic::transport::Channel>,
    arena: ArenaImpl,
    last_features: Features,
    entity_ids: Vec<u32>,
    undistortion: Option<UndistortMap>,
) -> Result<(u32, Features)> {
    let frame_number = image.frame_number;
    let detector_request = DetectorRequest {
        image: Some(image),
        arena: Some(arena.arena.clone()),
    };
    let mut features = detector
        .detect_features(detector_request)
        .await?
        .into_inner();
    features.frame_number = frame_number;

    arena.features_to_poses(&mut features, undistortion)?;

    let matcher_request = MatcherRequest {
        features: Some(features.clone()),
        last_features: Some(last_features),
        entity_ids,
    };
    features = matcher.match_features(matcher_request).await?.into_inner();
    Ok((frame_number, features))
}

pub fn start_tracking_task(
    state: &State,
    task_handle: &mut Option<tokio::task::JoinHandle<()>>,
    tracking_tx: &tokio::sync::mpsc::Sender<Result<(u32, Features)>>,
    image: &Image,
) {
    if state.experiment.recording_state == RecordingState::Replay as i32 {
        return;
    }
    let image = image.clone();
    let detector = state.connections.feature_detector();
    let matcher = state.connections.matcher();
    if detector.is_none() || matcher.is_none() {
        return;
    }
    let (detector, matcher) = (detector.unwrap(), matcher.unwrap());
    let last_features = state
        .experiment
        .last_features
        .clone()
        .expect("last_features is None");
    let arena = state.arena_impl.clone();
    let tracking_tx = tracking_tx.clone();
    let entity_ids = state.experiment.entity_ids.clone();
    let undistortion = state.get_undistortion(UndistortMode::Poses);
    *task_handle = Some(tokio::spawn(async move {
        let result = tracking_task(
            image,
            detector,
            matcher,
            arena,
            last_features,
            entity_ids,
            undistortion,
        )
        .await;
        tracking_tx.send(result).await.unwrap();
    }));
}

impl Features {
    pub fn switch_ids(&mut self, switch_request: &EntityIdSwitch) {
        self.features.iter_mut().for_each(|f| {
            if f.id == Some(switch_request.id1) {
                f.id = Some(switch_request.id2);
            } else if f.id == Some(switch_request.id2) {
                f.id = Some(switch_request.id1);
            }
        });
    }
}
