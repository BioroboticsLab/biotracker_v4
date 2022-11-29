use crate::core::{
    message_bus::Client, CommandLineArguments, Component, ImageFeature, ImageFeatures, Message,
    Point, SkeletonEdge, SkeletonNode,
};
use anyhow::Result;
use std::sync::Arc;

pub struct Tracker {
    msg_bus: Client,
}

impl Component for Tracker {
    fn new(msg_bus: Client, _: Arc<CommandLineArguments>) -> Self {
        Self { msg_bus }
    }

    fn run(&mut self) -> Result<()> {
        self.msg_bus.subscribe("Image")?;
        loop {
            while let Ok(Some(msg)) = self.msg_bus.poll(-1) {
                match msg {
                    Message::Image(img) => {
                        let pts = img.pts;
                        let mut features = ImageFeatures {
                            pts,
                            features: vec![],
                        };
                        let center_x = (img.width / 2) as f32;
                        let center_y = (img.height / 2) as f32;
                        let r = img.width as f32 / 4.0;
                        for i in 0..6 {
                            let step = (pts.0 as f64 / 1000000000.0) as f32 + i as f32;
                            let x = center_x + (step.sin() * r);
                            let y = center_y + (step.cos() * r);
                            let head = SkeletonNode {
                                point: Point { x, y },
                                score: 1.0,
                            };
                            let step2 = (pts.0 as f64 / 1000000000.0) as f32 + i as f32 - 0.1;
                            let x2 = center_x + (step2.sin() * r);
                            let y2 = center_y + (step2.cos() * r);
                            let tail = SkeletonNode {
                                point: Point { x: x2, y: y2 },
                                score: 1.0,
                            };
                            let midline = SkeletonEdge { from: 0, to: 1 };
                            features.features.push(ImageFeature {
                                nodes: vec![head, tail],
                                edges: vec![midline],
                                score: 1.0,
                            })
                        }
                        self.msg_bus.send(Message::Features(features))?;
                    }
                    _ => panic!("Unexpected message"),
                }
            }
        }
    }
}
