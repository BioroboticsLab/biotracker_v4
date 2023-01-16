use prost::Message as ProstMessage;
use std::fmt::Debug;

include!(concat!(env!("OUT_DIR"), "/biotracker.rs"));

pub use bio_tracker_message::Content as Message;

impl Message {
    pub fn topic(&self) -> (Topic, Option<&String>) {
        match self {
            Message::Image(img) => (Topic::Image, Some(&img.stream_id)),
            Message::Features(_) => (Topic::Features, None),
            Message::Entities(_) => (Topic::Entities, None),
            Message::ExperimentState(_) => (Topic::ExperimentState, None),
            Message::ComponentMessage(msg) => (Topic::ComponentMessage, Some(&msg.recipient_id)),
            Message::Shutdown(_) => (Topic::Shutdown, None),
        }
    }

    pub fn serialize(self) -> (String, Vec<u8>) {
        let (topic, topic_suffix) = self.topic();
        let topic_string = match topic_suffix {
            Some(suffix) => format!("{}.{}", topic.as_str_name(), suffix),
            None => format!("{}", topic.as_str_name()),
        };

        let buf = BioTrackerMessage {
            topic: topic as i32,
            content: Some(self),
        }
        .encode_to_vec();
        (topic_string, buf)
    }
}
