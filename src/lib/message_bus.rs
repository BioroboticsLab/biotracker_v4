use super::protocol::{Message, MessageType};
use anyhow::{anyhow, Result};
use zmq::{Context, Message as ZmqMessage, PollEvents, Socket};

pub struct Client {
    #[allow(dead_code)]
    zctx: Context,
    push: Socket,
    sub: Socket,
}

impl Client {
    pub fn new() -> Result<Self> {
        let zctx = Context::new();
        let push = zctx.socket(zmq::PUSH)?;
        push.connect("tcp://127.0.0.1:6667")?;
        let sub = zctx.socket(zmq::SUB)?;
        sub.connect("tcp://127.0.0.1:6668")?;
        Ok(Self { zctx, push, sub })
    }

    pub fn subscribe_str(&self, topic: &str) -> Result<()> {
        self.sub.set_subscribe(topic.as_bytes())?;
        Ok(())
    }

    pub fn subscribe(&self, topics: &[MessageType]) -> Result<()> {
        for topic in topics {
            self.sub.set_subscribe(topic.as_str_name().as_bytes())?;
        }
        Ok(())
    }

    pub fn poll(&self, timeout: i64) -> Result<Option<Message>> {
        if self.sub.poll(PollEvents::POLLIN, timeout)? > 0 {
            let mut zmq_msg = ZmqMessage::new();
            self.sub.recv(&mut zmq_msg, 0)?;
            if let Some(topic) = zmq_msg.as_str() {
                let ty = MessageType::from_str_name(topic).expect("Invalid topic");
                assert!(self.sub.get_rcvmore()?);
                self.sub.recv(&mut zmq_msg, 0)?;
                let deserialized = Message::deserialize(ty, &*zmq_msg)?;
                return Ok(Some(deserialized));
            } else {
                return Err(anyhow!("Failed to decode topic"));
            }
        }
        return Ok(None);
    }

    pub fn send(&self, msg: Message) -> Result<()> {
        let (ty, buf) = msg.serialize();
        self.push.send(ty.as_str_name(), zmq::SNDMORE)?;
        self.push.send(buf, 0)?;
        Ok(())
    }
}
