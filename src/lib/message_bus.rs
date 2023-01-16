use super::protocol::*;
use anyhow::Result;
use prost::Message as ProstMessage;
use zmq::{Context, Message as ZmqMessage, PollEvents, Socket};

pub struct Client {
    _zctx: Context,
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
        Ok(Self {
            _zctx: zctx,
            push,
            sub,
        })
    }

    pub fn register_component(&self, component: Component) -> Result<()> {
        let component_topic =
            Topic::ComponentMessage.as_str_name().to_owned() + "." + &component.id;
        self.send(Message::ComponentMessage(ComponentMessage {
            recipient_id: "BioTracker".to_owned(),
            content: Some(component_message::Content::Registration(component)),
        }))?;
        self.subscribe_str(&component_topic)?;
        Ok(())
    }

    pub fn subscribe_str(&self, topic: &str) -> Result<()> {
        self.sub.set_subscribe(topic.as_bytes())?;
        Ok(())
    }

    pub fn subscribe_image(&self, image_stream_id: &str) -> Result<()> {
        let topic = Topic::Image.as_str_name().to_owned() + "." + image_stream_id;
        self.subscribe_str(&topic)
    }

    pub fn subscribe(&self, topics: &[Topic]) -> Result<()> {
        for topic in topics {
            let topic_str = topic.as_str_name();
            self.sub.set_subscribe(topic_str.as_bytes())?;
        }
        Ok(())
    }

    pub fn poll(&self, timeout: i64) -> Result<Option<Message>> {
        if self.sub.poll(PollEvents::POLLIN, timeout)? > 0 {
            let mut zmq_msg = ZmqMessage::new();
            self.sub.recv(&mut zmq_msg, 0)?;
            assert!(self.sub.get_rcvmore()?);
            self.sub.recv(&mut zmq_msg, 0)?;
            let deserialized = BioTrackerMessage::decode(&*zmq_msg)?;
            return Ok(deserialized.content);
        }
        return Ok(None);
    }

    pub fn send(&self, msg: Message) -> Result<()> {
        let (topic, buf) = msg.serialize();
        self.push.send(&topic, zmq::SNDMORE)?;
        self.push.send(buf, 0)?;
        Ok(())
    }
}

pub struct Server {
    _zctx: Context,
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
            _zctx: zctx,
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
