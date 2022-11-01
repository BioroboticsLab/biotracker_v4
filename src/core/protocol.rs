use derive_more::{Add, Div, Mul, Sub};

pub enum Message {
    Video(VideoMessage),
}

pub enum VideoMessage {
    Info(VideoInfo),
    Event(VideoEvent),
    Command(VideoCommand),
    Stream(FrameMessage),
}

#[derive(Debug)]
pub struct VideoInfo {
    pub resolution: [u32; 2],
    pub path: String,
}

pub struct VideoSeekable {
    pub start: Timestamp,
    pub end: Timestamp,
}

#[derive(PartialEq)]
pub enum VideoEvent {
    Open(Box<std::path::Path>),
    Seek(Timestamp),
    Pause,
    Play,
    Stop,
}

pub struct VideoCommand(VideoEvent);

pub struct FrameMessage {
    pts: Timestamp,
    data: FrameData,
}

pub enum FrameData {
    ImageData,
    TrackingData,
    Annotation,
    EndOfStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Add, Div, Mul, Sub)]
pub struct Timestamp(pub u64);

impl Timestamp {
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
