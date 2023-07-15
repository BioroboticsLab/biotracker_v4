use super::protocol::*;
pub use matcher_server::Matcher;
use pathfinding::{kuhn_munkres::kuhn_munkres_min, matrix::Matrix};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};

const MAX_DISTANCE: i64 = 1000000;

#[derive(Deserialize, Default, Clone)]
pub struct MatcherConfig {
    ignore_nan: bool,
    ignore_out_of_bounds: bool,
    confidence_threshold_feature: f32,
    confidence_threshold_node: f32,
}

#[derive(Default)]
struct MatcherState {
    config: MatcherConfig,
    last_seen: HashMap<u32, Feature>,
}

#[derive(Default)]
pub struct MatcherService {
    inner: Arc<Mutex<MatcherState>>,
}

#[tonic::async_trait]
impl Matcher for MatcherService {
    async fn match_features(
        &self,
        request: Request<MatcherRequest>,
    ) -> Result<Response<Features>, Status> {
        let request = request.into_inner();
        let MatcherRequest {
            features,
            entity_ids,
        } = request;

        let features = features.expect("features must not be None");
        let mut state = self.inner.lock().unwrap();
        Ok(Response::new(
            state.hungarian_matching(entity_ids, features),
        ))
    }

    async fn set_config(
        &self,
        request: Request<ComponentConfig>,
    ) -> Result<Response<Empty>, Status> {
        let config = request.into_inner().config_json;
        let config: MatcherConfig = serde_json::from_str(&config).map_err(|e| {
            Status::invalid_argument(format!("Could not parse config: {}", e.to_string()))
        })?;
        let mut inner = self.inner.lock().unwrap();
        (*inner).config = config;
        Ok(Response::new(Empty {}))
    }
}

impl MatcherState {
    fn hungarian_matching(&mut self, entity_ids: Vec<u32>, mut features_msg: Features) -> Features {
        let config = &self.config;
        let frame_number = features_msg.frame_number;
        // Remove out-of-bound features and features containing NaN values
        let mut nan_count = 0;
        let mut oob_count = 0;
        let mut confidence_count = 0;
        let mut features: Vec<&mut Feature> = features_msg
            .features
            .iter_mut()
            .filter(|f| {
                let mut result = true;
                if config.ignore_out_of_bounds && f.out_of_bounds.unwrap_or(false) {
                    oob_count += 1;
                    result = false;
                }
                if config.ignore_nan {
                    for node in f.image_nodes.iter() {
                        if node.x.is_nan() || node.y.is_nan() {
                            nan_count += 1;
                            result = false;
                        }
                    }
                }
                if f.score < config.confidence_threshold_feature {
                    confidence_count += 1;
                    result = false;
                }
                return result;
            })
            .collect();
        if nan_count > 0 || oob_count > 0 || confidence_count > 0 {
            for _ in 0..nan_count {
                metrics::increment_counter!("count.NaN_features_removed");
            }
            for _ in 0..oob_count {
                metrics::increment_counter!("count.oob_features_removed");
            }
            for _ in 0..confidence_count {
                metrics::increment_counter!("count.confidence_features_removed");
            }
            log::warn!(
                "Frame {} Removed {} features containing NaN values, {} out-of-bound features, {} features below confidence threshold",
                frame_number,
                nan_count,
                oob_count,
                confidence_count
            );
        }

        let last_matched_features = entity_ids
            .iter()
            .map(|id| self.last_seen.get(id))
            .collect::<Vec<_>>();
        let last_matched_features_count = last_matched_features.len();

        // Match features
        let weights = distance_matrix(
            &features,
            &last_matched_features,
            config.confidence_threshold_node,
        );
        let (_, assignment) = kuhn_munkres_min(&weights);
        for (feature_idx, last_feature_idx) in assignment.iter().enumerate() {
            if feature_idx >= features.len() || *last_feature_idx >= last_matched_features_count {
                continue;
            }
            let id = entity_ids[*last_feature_idx];
            features[feature_idx].id = Some(id);
            self.last_seen.insert(id, features[feature_idx].clone());
        }

        features_msg
    }
}

fn distance(a: &Feature, b: &Feature, confidence_threshold: f32) -> i64 {
    let mut node_squared_distance_sum = 0;
    let mut node_cnt = 0;
    for node_idx in 0..a.image_nodes.len() {
        let x1 = a.image_nodes[node_idx].x;
        let y1 = a.image_nodes[node_idx].y;
        let x2 = b.image_nodes[node_idx].x;
        let y2 = b.image_nodes[node_idx].y;
        if x1.is_nan() || y1.is_nan() || x2.is_nan() || y2.is_nan() {
            continue;
        }
        if a.score < confidence_threshold || b.score < confidence_threshold {
            continue;
        }
        node_cnt += 1;
        let distance = (x1 - x2).powi(2) + (y1 - y2).powi(2);
        node_squared_distance_sum += distance as i64;
    }
    match node_cnt {
        0 => MAX_DISTANCE,
        _ => node_squared_distance_sum / node_cnt,
    }
}

fn distance_matrix(
    features: &Vec<&mut Feature>,
    last_features: &Vec<Option<&Feature>>,
    confidence_threshold: f32,
) -> Matrix<i64> {
    let n = features.len().max(last_features.len());
    let mut distances = Matrix::new(n, n, 0);
    for feature_idx in 0..n {
        for last_feature_idx in 0..n {
            let distance_ref = distances.get_mut((feature_idx, last_feature_idx)).unwrap();
            let last_feature = if last_feature_idx >= last_features.len() {
                None
            } else {
                last_features[last_feature_idx]
            };
            *distance_ref = if feature_idx >= features.len() || last_feature.is_none() {
                MAX_DISTANCE
            } else {
                distance(
                    &features[feature_idx],
                    last_feature.unwrap(),
                    confidence_threshold,
                )
            }
        }
    }
    distances
}
