use derive_more::{Add, Div, Mul, Sub};

#[derive(Debug)]
pub enum Message {
    Event(VideoState),
    Command(VideoState),
    Sample(VideoSample),
    Seekable(VideoSeekable),
    Shutdown,
}

#[derive(Debug, PartialEq)]
pub struct VideoSeekable {
    pub start: Timestamp,
    pub end: Timestamp,
}

#[derive(Debug, PartialEq)]
pub enum VideoState {
    Open(String),
    Seek(Timestamp),
    Pause,
    Play,
    Stop,
}

#[derive(Debug)]
pub struct VideoSample {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub pts: Option<Timestamp>,
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
