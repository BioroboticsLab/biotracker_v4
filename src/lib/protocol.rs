use prost::Message as ProstMessage;
use std::fmt::Debug;

include!(concat!(env!("OUT_DIR"), "/biotracker.tracking.rs"));
include!(concat!(env!("OUT_DIR"), "/biotracker.video.rs"));
include!(concat!(env!("OUT_DIR"), "/biotracker.experiment.rs"));

#[derive(Debug)]
pub enum Message {
    Image(Image),
    Features(Features),
    Entities(Entities),
    VideoDecoderState(VideoDecoderState),
    VideoDecoderCommand(VideoDecoderCommand),
    VideoEncoderState(VideoEncoderState),
    VideoEncoderCommand(VideoEncoderCommand),
    ExperimentState(ExperimentState),
    Shutdown,
}

impl Message {
    pub fn deserialize(ty: MessageType, buf: &[u8]) -> Result<Self, prost::DecodeError> {
        match ty {
            MessageType::Image => Ok(Message::Image(Image::decode(buf)?)),
            MessageType::Features => Ok(Message::Features(Features::decode(buf)?)),
            MessageType::Entities => Ok(Message::Entities(Entities::decode(buf)?)),
            MessageType::VideoDecoderState => {
                Ok(Message::VideoDecoderState(VideoDecoderState::decode(buf)?))
            }
            MessageType::VideoDecoderCommand => Ok(Message::VideoDecoderCommand(
                VideoDecoderCommand::decode(buf)?,
            )),
            MessageType::VideoEncoderState => {
                Ok(Message::VideoEncoderState(VideoEncoderState::decode(buf)?))
            }
            MessageType::VideoEncoderCommand => Ok(Message::VideoEncoderCommand(
                VideoEncoderCommand::decode(buf)?,
            )),
            MessageType::ExperimentState => {
                Ok(Message::ExperimentState(ExperimentState::decode(buf)?))
            }
            MessageType::Shutdown => Ok(Message::Shutdown),
        }
    }

    pub fn serialize(&self) -> (MessageType, Vec<u8>) {
        match self {
            Message::Image(msg) => (MessageType::Image, msg.encode_to_vec()),
            Message::Features(msg) => (MessageType::Features, msg.encode_to_vec()),
            Message::Entities(msg) => (MessageType::Entities, msg.encode_to_vec()),
            Message::VideoDecoderState(msg) => {
                (MessageType::VideoDecoderState, msg.encode_to_vec())
            }
            Message::VideoDecoderCommand(msg) => {
                (MessageType::VideoDecoderCommand, msg.encode_to_vec())
            }
            Message::VideoEncoderState(msg) => {
                (MessageType::VideoEncoderState, msg.encode_to_vec())
            }
            Message::VideoEncoderCommand(msg) => {
                (MessageType::VideoEncoderCommand, msg.encode_to_vec())
            }
            Message::ExperimentState(msg) => (MessageType::ExperimentState, msg.encode_to_vec()),
            Message::Shutdown => (MessageType::Shutdown, Shutdown::default().encode_to_vec()),
        }
    }
}
