use super::protocol::*;
pub use matcher_server::Matcher;
use pathfinding::{kuhn_munkres::kuhn_munkres_min, matrix::Matrix};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};

#[derive(Deserialize, Default, Clone)]
pub struct MatcherConfig {
    ignore_nan: bool,
    ignore_out_of_bounds: bool,
}

#[derive(Default)]
pub struct MatcherService {
    inner: Arc<Mutex<MatcherConfig>>,
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
            last_features,
            entity_ids,
        } = request;

        let features = features.expect("features must not be None");
        let last_features = last_features.expect("last_features must not be None");
        let config = self.inner.lock().unwrap().clone();
        Ok(Response::new(MatcherService::hungarian_matching(
            config,
            entity_ids,
            last_features,
            features,
        )))
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
        *inner = config;
        Ok(Response::new(Empty {}))
    }
}

impl MatcherService {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    fn hungarian_matching(
        config: MatcherConfig,
        entity_ids: Vec<u32>,
        last_features: Features,
        mut features_msg: Features,
    ) -> Features {
        let frame_number = features_msg.frame_number;
        // Remove out-of-bound features and features containing NaN values
        let mut nan_count = 0;
        let mut oob_count = 0;
        let mut features: Vec<&mut Feature> = features_msg
            .features
            .iter_mut()
            .filter(|f| {
                let mut result = true;
                if config.ignore_out_of_bounds && f.out_of_bounds {
                    oob_count += 1;
                    result = false;
                }
                if !config.ignore_nan {
                    return result;
                }
                for node in f.nodes.iter() {
                    if node.x.is_nan() || node.y.is_nan() {
                        nan_count += 1;
                        result = false;
                    }
                }
                return result;
            })
            .collect();
        if nan_count > 0 || oob_count > 0 {
            log::warn!(
                "Frame {} Removed {} features containing NaN values and {} out-of-bound features",
                frame_number,
                nan_count,
                oob_count
            );
        }

        // Remove lowest scoring features, if there is more then we expect
        let entity_count = entity_ids.len();

        if features.len() > entity_count {
            // sort by score in descending order
            features.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            while features.len() > entity_count {
                features.pop();
            }
        }

        let last_matched_features = entity_ids
            .iter()
            .map(|id| {
                last_features.features.iter().find(|f| match f.id {
                    Some(i) => i == *id,
                    None => false,
                })
            })
            .collect::<Vec<_>>();

        // Match features
        let weights = MatcherService::distance_matrix(&features, &last_matched_features);
        let (_, assignment) = kuhn_munkres_min(&weights);

        for (feature_idx, last_feature_idx) in assignment.iter().enumerate() {
            if feature_idx >= features.len() {
                break;
            }
            let id = entity_ids[*last_feature_idx];
            features[feature_idx].id = Some(id);
        }

        features_msg
    }

    fn distance(a: &Feature, b: &Feature) -> i64 {
        let mut node_squared_distance_sum = 0;
        let mut node_cnt = 0;
        for node_idx in 0..a.nodes.len() {
            let x1 = a.nodes[node_idx].x;
            let y1 = a.nodes[node_idx].y;
            let x2 = b.nodes[node_idx].x;
            let y2 = b.nodes[node_idx].y;
            if x1.is_nan() || y1.is_nan() || x2.is_nan() || y2.is_nan() {
                continue;
            }
            node_cnt += 1;
            let distance = (x1 - x2).powi(2) + (y1 - y2).powi(2);
            node_squared_distance_sum += distance as i64;
        }
        node_squared_distance_sum / node_cnt
    }

    fn distance_matrix(
        features: &Vec<&mut Feature>,
        last_features: &Vec<Option<&Feature>>,
    ) -> Matrix<i64> {
        let n = features.len().max(last_features.len());
        let mut distances = Matrix::new(n, n, 0);
        let max_distance = 1000000;
        for feature_idx in 0..n {
            for last_feature_idx in 0..n {
                let distance_ref = distances.get_mut((feature_idx, last_feature_idx)).unwrap();
                let last_feature = last_features[last_feature_idx];
                *distance_ref = if feature_idx >= features.len() || last_feature.is_none() {
                    max_distance
                } else {
                    MatcherService::distance(&features[feature_idx], last_feature.unwrap())
                };
            }
        }
        distances
    }
}
