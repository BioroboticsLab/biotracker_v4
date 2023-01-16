use anyhow::{anyhow, Result};
use libtracker::{protocol::*, Client, Component};
use pathfinding::{kuhn_munkres::kuhn_munkres_min, matrix::Matrix};
use rand::Rng;
use std::collections::HashMap;

#[derive(Debug)]
struct MatchedEntity {
    id: String,
    feature: Feature,
    last_seen: u64,
}

pub struct Matcher {
    msg_bus: Client,
    last_matching: Vec<MatchedEntity>,
    entity_count: u32,
}

impl Component for Matcher {
    fn run(&mut self) -> Result<()> {
        self.msg_bus
            .subscribe(&[Topic::Features, Topic::ExperimentState])?;
        while let Some(message) = self.msg_bus.poll(-1)? {
            match message {
                Message::Features(mut features_msg) => {
                    if !features_msg.features.is_empty() {
                        let entities = self.matching(&mut features_msg);
                        self.msg_bus.send(Message::Entities(entities))?;
                    }
                }
                Message::ExperimentState(experiment) => {
                    self.entity_count = experiment.entity_count;
                }
                _ => return Err(anyhow!("Unexpected message {:?}", message)),
            }
        }
        Ok(())
    }
}

impl Matcher {
    pub fn new(msg_bus: Client) -> Self {
        Self {
            msg_bus,
            last_matching: vec![],
            entity_count: 0,
        }
    }

    fn matching(&mut self, features_msg: &mut Features) -> Entities {
        let timestamp = features_msg.timestamp;
        let features = &mut features_msg.features;
        // Remove lowest scoring features, if there is more then we expect
        if features.len() > self.entity_count as usize {
            // sort by score in descending order
            features.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            while features.len() > self.entity_count as usize {
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
            if *last_feature_idx >= self.last_matching.len() {
                let id: u64 = rand::thread_rng().gen();
                self.last_matching.push(MatchedEntity {
                    id: id.to_string(),
                    feature,
                    last_seen: timestamp,
                });
            } else {
                self.last_matching[*last_feature_idx].feature = feature;
                self.last_matching[*last_feature_idx].last_seen = timestamp;
            }
        }

        let mut entities_map = HashMap::new();
        for matched_entity in &self.last_matching {
            if matched_entity.last_seen == timestamp {
                entities_map.insert(matched_entity.id.clone(), matched_entity.feature.clone());
            }
        }

        Entities {
            timestamp,
            entities: entities_map,
            skeleton: features_msg.skeleton.clone(),
        }
    }
}
