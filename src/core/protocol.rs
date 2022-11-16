use derive_more::{Add, Div, Mul, Sub};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    /// Playback messages
    Command(State),
    Event(State),
    Seekable(Seekable),
    Shutdown,
    /// Sample messages
    Image(ImageData),
    Features(ImageFeatures),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SkeletonNode {
    /// Location of the node in image pixels
    pub point: Point,
    /// Confidence score of the node, usually in range [0,1]
    pub score: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SkeletonEdge {
    /// Index of origin node
    pub from: usize,
    /// Index of target node
    pub to: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ImageFeature {
    /// List of detected skeleton nodes
    pub nodes: Vec<SkeletonNode>,
    /// List of detected skeleton edges, containing indices into the nodes list
    pub edges: Vec<SkeletonEdge>,
    /// Confidence score of the feature, usually in range [0,1]
    pub score: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ImageFeatures {
    pub pts: Timestamp,
    pub features: Vec<ImageFeature>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ImageData {
    pub pts: Timestamp,
    pub shm_id: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Seekable {
    pub start: Timestamp,
    pub end: Timestamp,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum State {
    Open(String),
    Seek(Timestamp),
    Pause,
    Play,
    Stop,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Add, Div, Mul, Sub)]
pub struct Timestamp(pub u64);

impl Timestamp {
    #[allow(dead_code)]
    pub fn difference(&self, other: &Timestamp) -> Timestamp {
        if self.0 > other.0 {
            return Timestamp(self.0 - other.0);
        } else {
            return Timestamp(other.0 - self.0);
        }
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let ns = self.0;
        let ms = ns / 1_000_000;
        let s = (ms / 1000) % 60;
        let m = (ms / (1000 * 60)) % 60;
        let h = (ms / (1000 * 3600)) % 24;
        write!(f, "{:02}:{:02}:{:02}", h, m, s)
    }
}
