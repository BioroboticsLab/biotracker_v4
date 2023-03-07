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
    ) -> Result<Response<Entities>, Status> {
        let request = request.into_inner();
        let MatcherRequest {
            features,
            frame_number,
            last_entities,
        } = request;

        let features = match features {
            Some(features) => features,
            None => return Err(Status::invalid_argument("features must not be None")),
        };

        let last_entities = match last_entities {
            Some(entities) => entities,
            None => return Err(Status::invalid_argument("entities must not be None")),
        };

        let config = self.inner.lock().unwrap().clone();

        Ok(Response::new(MatcherService::hungarian_matching(
            config,
            frame_number,
            last_entities,
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
        frame_number: u32,
        last_entities: Entities,
        features_msg: Features,
    ) -> Entities {
        // Remove out-of-bound features and features containing NaN values
        let mut nan_count = 0;
        let mut oob_count = 0;
        let mut features: Vec<&Feature> = features_msg
            .features
            .iter()
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
            eprintln!(
                "Removed {} features containing NaN values and {} out-of-bound features",
                nan_count, oob_count
            );
        }

        // Remove lowest scoring features, if there is more then we expect
        let mut last_entities = last_entities.entities;
        let entity_count = last_entities.len();

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

        // Match features
        let weights = MatcherService::distance_matrix(&features, &last_entities);
        let (_, assignment) = kuhn_munkres_min(&weights);

        for (feature_idx, last_feature_idx) in assignment.iter().enumerate() {
            if feature_idx >= features.len() {
                break;
            }

            let feature = features[feature_idx].clone();
            last_entities[*last_feature_idx].feature = Some(feature);
            last_entities[*last_feature_idx].frame_number = frame_number;
        }

        Entities {
            entities: last_entities,
        }
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

    fn distance_matrix(features: &Vec<&Feature>, last_entities: &Vec<Entity>) -> Matrix<i64> {
        let n = features.len().max(last_entities.len());
        let mut distances = Matrix::new(n, n, 0);
        let max_distance = 1000000;
        for feature_idx in 0..n {
            for last_feature_idx in 0..n {
                let distance_ref = distances.get_mut((feature_idx, last_feature_idx)).unwrap();
                *distance_ref = if feature_idx >= features.len()
                    || last_entities[last_feature_idx].feature.is_none()
                    || features[feature_idx].out_of_bounds
                {
                    max_distance
                } else {
                    let feature = &features[feature_idx];
                    let last_feature = &last_entities[last_feature_idx]
                        .feature
                        .as_ref()
                        .expect("invalid none feature");
                    MatcherService::distance(feature, last_feature)
                };
            }
        }
        distances
    }
}
