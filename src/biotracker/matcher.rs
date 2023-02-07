use super::protocol::*;
pub use matcher_server::Matcher;
use pathfinding::{kuhn_munkres::kuhn_munkres_min, matrix::Matrix};
use tonic::{Request, Response, Status};

pub struct MatcherService {}

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

        Ok(Response::new(MatcherService::hungarian_matching(
            frame_number,
            last_entities,
            features,
        )))
    }

    async fn set_config(
        &self,
        _request: Request<ComponentConfiguration>,
    ) -> Result<Response<Empty>, Status> {
        Ok(Response::new(Empty {}))
    }
}

impl MatcherService {
    pub fn new() -> Self {
        Self {}
    }

    pub fn hungarian_matching(
        frame_number: u32,
        last_entities: Entities,
        mut features_msg: Features,
    ) -> Entities {
        let features = &mut features_msg.features;
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
        let n = features.len().max(last_entities.len());
        let mut weights = Matrix::new(n, n, 0);
        for feature_idx in 0..n {
            for last_feature_idx in 0..n {
                let distance_ref = weights.get_mut((feature_idx, last_feature_idx)).unwrap();
                *distance_ref = if feature_idx >= features.len()
                    || last_entities[last_feature_idx].feature.is_none()
                {
                    1000000
                } else {
                    let mut node_squared_distance_sum = 0;
                    let mut node_cnt = 0;
                    let feature = &features[feature_idx];
                    let last_feature = &last_entities[last_feature_idx]
                        .feature
                        .as_ref()
                        .expect("invalid none feature");
                    for node_idx in 0..feature.nodes.len() {
                        let x1 = feature.nodes[node_idx].x;
                        let y1 = feature.nodes[node_idx].y;
                        let x2 = last_feature.nodes[node_idx].x;
                        let y2 = last_feature.nodes[node_idx].y;
                        if x1.is_nan() || y1.is_nan() || x2.is_nan() || y2.is_nan() {
                            continue;
                        }
                        node_cnt += 1;
                        let distance = (x1 - x2).powi(2) + (y1 - y2).powi(2);
                        node_squared_distance_sum += distance as i64;
                    }
                    node_squared_distance_sum / node_cnt
                };
            }
        }

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
}
