use super::{Matcher, PythonRunner, VideoDecoder, VideoEncoder};
use anyhow::{anyhow, Result};
use libtracker::{protocol::*, Client, CommandLineArguments, Component};
use std::sync::Arc;
use std::thread::JoinHandle;
use zmq::{Context, Message as ZmqMessage, PollEvents, Socket};

pub struct Core {
    args: Arc<CommandLineArguments>,
    threads: Vec<JoinHandle<Result<()>>>,
    _zctx_raii: Context,
    pull: Socket,
    publish: Socket,
    experiment: ExperimentState,
}

impl Core {
    pub fn new(args: &CommandLineArguments) -> Result<Self> {
        let zctx = Context::new();
        let pull = zctx.socket(zmq::PULL)?;
        pull.bind("tcp://*:6667")?;
        let publish = zctx.socket(zmq::PUB)?;
        publish.bind("tcp://*:6668")?;
        Ok(Self {
            args: Arc::new(args.clone()),
            threads: vec![],
            _zctx_raii: zctx,
            pull,
            publish,
            experiment: ExperimentState::default(),
        })
    }

    pub fn start(mut self) -> Result<()> {
        std::thread::Builder::new()
            .name("BioTrackerCore".to_string())
            .spawn(move || -> Result<()> {
                self.update_experiment(&Message::ExperimentUpdate(ExperimentUpdate {
                    fps: Some(30.0),
                    ..Default::default()
                }))?;

                if let (Some(venv), Some(cmd)) = (&self.args.tracker_venv, &self.args.tracker_cmd) {
                    let (venv, cmd) = (venv.to_owned(), cmd.to_owned());
                    self.add_component(|_msg_bus, _args| PythonRunner::new(venv, cmd))?;
                }

                if let (Some(venv), Some(cmd)) = (&self.args.robofish_venv, &self.args.robofish_cmd)
                {
                    let (venv, cmd) = (venv.to_owned(), cmd.to_owned());
                    self.add_component(|_msg_bus, _args| PythonRunner::new(venv, cmd))?;
                }

                self.add_component(VideoEncoder::new)?;
                self.add_component(Matcher::new)?;
                self.add_component(VideoDecoder::new)?;

                let collector = 0;
                loop {
                    let mut poll_items = [self.pull.as_poll_item(PollEvents::POLLIN)];
                    zmq::poll(&mut poll_items, -1)?;
                    if poll_items[collector].is_readable() {
                        let mut topic = ZmqMessage::new();
                        let mut msg = ZmqMessage::new();
                        self.pull.recv(&mut topic, 0)?;
                        assert!(self.pull.get_rcvmore()?);
                        self.pull.recv(&mut msg, 0)?;
                        if let Some(topic) = topic.as_str() {
                            let ty = MessageType::from_str_name(topic).expect("Invalid topic");
                            match ty {
                                MessageType::ExperimentUpdate => {
                                    let deserialized = Message::deserialize(ty, &*msg)?;
                                    self.update_experiment(&deserialized)?;
                                }
                                MessageType::Shutdown => {
                                    self.shutdown()?;
                                    return Ok(());
                                }
                                _ => {
                                    self.publish.send(topic, zmq::SNDMORE)?;
                                    self.publish.send(msg, 0)?;
                                }
                            }
                        } else {
                            return Err(anyhow!("Invalid topic"));
                        }
                    }
                }
            })?;
        Ok(())
    }

    fn send_msg(&self, msg: Message) -> Result<()> {
        let (ty, buf) = msg.serialize();
        self.publish.send(ty.as_str_name(), zmq::SNDMORE)?;
        self.publish.send(buf, 0)?;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        self.send_msg(Message::Shutdown)?;
        for thread in self.threads.drain(..) {
            thread.join().unwrap()?;
        }
        Ok(())
    }

    fn update_experiment(&mut self, msg: &Message) -> Result<()> {
        match msg {
            Message::ExperimentUpdate(update) => {
                if let Some(playback_state) = update.playback_state {
                    self.experiment.playback_state = playback_state;
                }
                if let Some(recording_state) = update.recording_state {
                    self.experiment.recording_state = recording_state;
                }
                if let Some(entity_count) = update.entity_count {
                    self.experiment.entity_count = entity_count;
                }
                if let Some(project_directory) = &update.project_directory {
                    self.experiment.project_directory = project_directory.clone();
                }
                if let Some(fps) = update.fps {
                    self.experiment.target_fps = fps;
                }
                if let Some(frame_count) = update.frame_count {
                    self.experiment.frame_count = Some(frame_count);
                }
                if let Some(decoder_state) = &update.video_decoder_state {
                    self.experiment.video_decoder_state = Some(decoder_state.clone());
                }
                if let Some(encoder_state) = &update.video_encoder_state {
                    self.experiment.video_encoder_state = Some(encoder_state.clone());
                }
                if let Some(skeleton_descriptor) = &update.skeleton_descriptor {
                    self.experiment.skeleton_descriptor = Some(skeleton_descriptor.clone());
                }
            }
            _ => return Err(anyhow!("invalid message type: {:?}", msg))?,
        }
        self.send_msg(Message::ExperimentState(self.experiment.clone()))
    }

    fn add_component<F, T: Component + 'static>(&mut self, component_builder: F) -> Result<()>
    where
        F: FnOnce(Client, Arc<CommandLineArguments>) -> T + Send + 'static,
    {
        let thread_name = std::any::type_name::<T>();
        let msg_bus = Client::new()?;
        let args = self.args.clone();
        let thread_handle = std::thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || -> Result<()> {
                let mut component = component_builder(msg_bus, args);
                match component.run() {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        eprintln!("Error in {}: {}", thread_name, e);
                        Err(e)
                    }
                }
            })?;
        self.threads.push(thread_handle);
        Ok(())
    }
}
