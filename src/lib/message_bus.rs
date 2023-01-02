use super::protocol::{Message, MessageType};
use anyhow::{anyhow, Result};
use zmq::{Context, Message as ZmqMessage, PollEvents, Socket};

pub struct Server {
    #[allow(dead_code)]
    zctx: Context,
    pull: Socket,
    publish: Socket,
}

impl Server {
    pub fn new() -> Result<Self> {
        let zctx = Context::new();
        let pull = zctx.socket(zmq::PULL)?;
        pull.bind("tcp://*:6667")?;
        let publish = zctx.socket(zmq::PUB)?;
        publish.bind("tcp://*:6668")?;

        Ok(Self {
            zctx,
            pull,
            publish,
        })
    }

    pub fn run(&self) -> Result<()> {
        let collector = 0;
        let mut poll_items = [self.pull.as_poll_item(PollEvents::POLLIN)];

        loop {
            zmq::poll(&mut poll_items, -1)?;
            if poll_items[collector].is_readable() {
                let mut topic = ZmqMessage::new();
                let mut msg = ZmqMessage::new();
                self.pull.recv(&mut topic, 0)?;
                assert!(self.pull.get_rcvmore()?);
                self.pull.recv(&mut msg, 0)?;
                self.publish.send(topic, zmq::SNDMORE)?;
                self.publish.send(msg, 0)?;
            }
        }
    }
}

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
