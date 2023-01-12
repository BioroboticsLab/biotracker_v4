use prost::Message as ProstMessage;
use std::fmt::Debug;

include!(concat!(env!("OUT_DIR"), "/biotracker.rs"));

// converts all MessageType and corresponding Messages to a rust enum. Shutdown is a special case,
// since we don't care about it's Message.
macro_rules! message_enum {
    ($($name:ident),*) => {
        #[derive(Debug, Clone)]
        pub enum Message {
            Shutdown,
            $($name($name)),*
        }

        impl Message {
            pub fn deserialize(ty: MessageType, buf: &[u8]) -> Result<Self, prost::DecodeError> {
                match ty {
                    MessageType::Shutdown => Ok(Message::Shutdown),
                    $(MessageType::$name => Ok(Message::$name($name::decode(buf)?))),*
                }
            }

            pub fn serialize(&self) -> (MessageType, Vec<u8>) {
                match self {
                    Message::Shutdown => (MessageType::Shutdown, vec![]),
                    $(Message::$name(msg) => (MessageType::$name, msg.encode_to_vec())),*
                }
            }
        }
    }
}
message_enum!(Image, Features, Entities, ExperimentState, ExperimentUpdate);
