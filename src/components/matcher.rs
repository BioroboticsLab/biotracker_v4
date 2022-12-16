use anyhow::Result;
use libtracker::{
    message_bus::Client, Action, CommandLineArguments, Component, Entities, ImageFeature,
    ImageFeatures, Message, Timestamp,
};
use pathfinding::{kuhn_munkres::kuhn_munkres_min, matrix::Matrix};
use rand::Rng;
use std::collections::HashMap;

struct MatchedEntity {
    id: String,
    feature: ImageFeature,
    last_seen: Timestamp,
}

pub struct Matcher {
    msg_bus: Client,
    last_matching: Vec<MatchedEntity>,
    expected_entity_count: usize,
}

impl Component for Matcher {
    fn run(&mut self) -> Result<()> {
        self.msg_bus.subscribe("Feature")?;
        self.msg_bus.subscribe("UserAction")?;
        loop {
            if let Ok(Some(msg)) = self.msg_bus.poll(-1) {
                match msg {
                    Message::Features(features_msg) => {
                        if !features_msg.features.is_empty() {
                            let entities = self.matching(features_msg);
                            self.msg_bus.send(Message::Entities(entities))?;
                        }
                    }
                    Message::UserAction(action) => match action {
                        Action::AddEntity => self.expected_entity_count += 1,
                        Action::RemoveEntity => {
                            if self.expected_entity_count > 0 {
                                self.expected_entity_count -= 1;
                            }
                        }
                    },
                    _ => panic!("Unexpected message"),
                }
            }
        }
    }
}

impl Matcher {
    pub fn new(msg_bus: Client, args: CommandLineArguments) -> Self {
        let expected_entity_count = match args.entity_count {
            Some(entity_count) => entity_count as usize,
            None => 0,
        };

        Self {
            msg_bus,
            last_matching: vec![],
            expected_entity_count,
        }
    }

    fn matching(&mut self, mut features_msg: ImageFeatures) -> Entities {
        let pts = features_msg.pts;
        let features = &mut features_msg.features;
        // Remove lowest scoring features, if there is more then we expect
        if features.len() > self.expected_entity_count {
            // sort by score in descending order
            features.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            while features.len() > self.expected_entity_count {
                features.pop();
            }
        }

        // Match features
        let n = features.len().max(self.last_matching.len());
        let mut weights = Matrix::new(n, n, 0);
        for feature_idx in 0..n {
            for last_feature_idx in 0..n {
                let distance_ref = weights.get_mut((feature_idx, last_feature_idx)).unwrap();
                *distance_ref = if feature_idx >= features.len()
                    || last_feature_idx >= self.last_matching.len()
                {
                    1000000
                } else {
                    let mut node_squared_distance_sum = 0;
                    let mut node_cnt = 0;
                    let feature = &features[feature_idx];
                    let last_feature = &self.last_matching[last_feature_idx].feature;
                    for node_idx in 0..feature.nodes.len() {
                        let p1 = &feature.nodes[node_idx].point;
                        let p2 = &last_feature.nodes[node_idx].point;
                        if p1.x.is_none() || p1.y.is_none() || p2.x.is_none() || p2.y.is_none() {
                            continue;
                        }
                        let p1 = (p1.x.unwrap(), p1.y.unwrap());
                        let p2 = (p2.x.unwrap(), p2.y.unwrap());
                        node_cnt += 1;
                        let distance = (p1.0 - p2.0).powi(2) + (p1.1 - p2.0).powi(2);
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
            if *last_feature_idx >= self.last_matching.len() {
                let id: u64 = rand::thread_rng().gen();
                self.last_matching.push(MatchedEntity {
                    id: id.to_string(),
                    feature,
                    last_seen: pts,
                });
            } else {
                self.last_matching[*last_feature_idx].feature = feature;
                self.last_matching[*last_feature_idx].last_seen = pts;
            }
        }

        let mut entities_map = HashMap::new();
        for matched_entity in &self.last_matching {
            if matched_entity.last_seen == pts {
                entities_map.insert(matched_entity.id.clone(), matched_entity.feature.clone());
            }
        }

        Entities {
            pts,
            entities: entities_map,
        }
    }
}
