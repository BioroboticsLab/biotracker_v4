use super::{arena::ArenaImpl, protocol::*, undistort::UndistortMap, State};
use anyhow::{Context, Result};

#[derive(Debug)]
pub struct TrackingResult {
    pub frame_number: u32,
    pub features: Features,
    pub skeleton: SkeletonDescriptor,
}

async fn tracking_task(
    image: Image,
    mut detector: FeatureDetectorClient<tonic::transport::Channel>,
    mut matcher: MatcherClient<tonic::transport::Channel>,
    arena: ArenaImpl,
    entity_ids: Vec<u32>,
    undistortion: Option<UndistortMap>,
) -> Result<TrackingResult> {
    let frame_number = image.frame_number;
    let detector_request = DetectorRequest {
        image: Some(image),
        arena: Some(arena.arena.clone()),
    };
    let detector_start = std::time::Instant::now();
    let response = detector
        .detect_features(detector_request)
        .await?
        .into_inner();
    metrics::histogram!("latency.feature_detector", detector_start.elapsed());
    let mut features = response
        .features
        .context("Received DetectorResponse without features")?;
    let skeleton = response
        .skeleton
        .context("Received DetectorResponse without skeleton")?;
    features.frame_number = frame_number;
    arena.features_to_world(&mut features, &skeleton, undistortion)?;

    let matcher_request = MatcherRequest {
        features: Some(features.clone()),
        entity_ids,
    };
    let matcher_start = std::time::Instant::now();
    features = matcher.match_features(matcher_request).await?.into_inner();
    metrics::histogram!("latency.matcher", matcher_start.elapsed());
    Ok(TrackingResult {
        frame_number,
        features,
        skeleton,
    })
}

pub fn start_tracking_task(
    state: &State,
    task_handle: &mut Option<tokio::task::JoinHandle<()>>,
    tracking_tx: &tokio::sync::mpsc::Sender<Result<TrackingResult>>,
    image: &Image,
) {
    if !state.experiment.track_file.is_empty() {
        return;
    }
    let start = std::time::Instant::now();
    let image = image.clone();
    let detector = state.connections.feature_detector();
    let matcher = state.connections.matcher();
    if detector.is_none() || matcher.is_none() {
        return;
    }
    let (detector, matcher) = (detector.unwrap(), matcher.unwrap());
    let arena = state.arena_impl.clone();
    let tracking_tx = tracking_tx.clone();
    let entity_ids = state.experiment.entity_ids.clone();
    let undistortion = state.get_undistortion(UndistortMode::Poses);
    *task_handle = Some(tokio::spawn(async move {
        let result = tracking_task(image, detector, matcher, arena, entity_ids, undistortion).await;
        metrics::histogram!("latency.tracking", start.elapsed());
        metrics::increment_counter!("count.frame_tracked");
        tracking_tx.send(result).await.unwrap();
    }));
}
