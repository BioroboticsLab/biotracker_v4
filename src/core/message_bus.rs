use anyhow::{anyhow, Result};

use zmq::{Context, Message, PollEvents, Socket};

use super::protocol::Message as BioTrackerMessage;

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
                let mut topic = Message::new();
                let mut msg = Message::new();
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

fn to_topic(msg: &BioTrackerMessage) -> &str {
    let topic = match msg {
        BioTrackerMessage::Command(_) => "Command",
        BioTrackerMessage::Event(_) => "Event",
        BioTrackerMessage::Seekable(_) => "Seekable",
        BioTrackerMessage::Shutdown => "Shutdown",
        BioTrackerMessage::Image(_) => "Image",
        BioTrackerMessage::Features(_) => "Feature",
    };
    topic
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

    pub fn subscribe(&self, topic: &str) -> Result<()> {
        self.sub
            .set_subscribe(topic.as_bytes())
            .map_err(|e| anyhow!("{e}"))
    }

    pub fn poll(&self, timeout: i64) -> Result<Option<BioTrackerMessage>> {
        if self.sub.poll(PollEvents::POLLIN, timeout)? > 0 {
            let mut msg = Message::new();
            self.sub.recv(&mut msg, 0)?;
            assert!(self.sub.get_rcvmore()?);
            self.sub.recv(&mut msg, 0)?;
            if let Some(msg_str) = msg.as_str() {
                let deserialized: BioTrackerMessage = serde_json::from_str(msg_str)?;
                return Ok(Some(deserialized));
            } else {
                return Err(anyhow!("Failed to get message string"));
            }
        } else {
            return Ok(None);
        }
    }

    pub fn send(&self, msg: BioTrackerMessage) -> Result<()> {
        let serialized = serde_json::to_string(&msg)?;
        let topic = to_topic(&msg);
        self.push.send(topic, zmq::SNDMORE)?;
        self.push.send(&serialized, 0)?;
        Ok(())
    }
}
