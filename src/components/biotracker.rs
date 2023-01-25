use super::{
    BiotrackerConfig, ComponentConfig, Matcher, PythonRunner, VideoDecoder, VideoEncoder,
    VideoEncoderConfig,
};
use anyhow::Result;
use libtracker::{
    message_bus::{Client, Server},
    protocol::*,
    CommandLineArguments, Component,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::thread::JoinHandle;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BioTrackerCommand {
    PlaybackState(PlaybackState),
    RecordingState(RecordingState),
    OpenVideo(String),
    Seek(u32),
    AddEntity,
    RemoveEntity,
}

pub struct BioTracker {
    state: ExperimentState,
    config: BiotrackerConfig,
    video_decoder: Option<VideoDecoder>,
    video_encoder: Option<VideoEncoder>,
    args: Arc<CommandLineArguments>,
    _components: Vec<ComponentThread>,
    _server_thread: JoinHandle<Result<()>>,
    msg_bus: Client,
}

struct ComponentThread {
    _config: ComponentConfig,
    _thread: std::thread::JoinHandle<Result<()>>,
}

impl BioTracker {
    pub fn new(args: &CommandLineArguments) -> Result<Self> {
        let server = Server::new()?;
        let _server_thread = std::thread::Builder::new()
            .name("BioTrackerServer".to_string())
            .spawn(move || -> Result<()> { server.run() })?;
        let msg_bus = Client::new()?;
        let args = Arc::new(args.clone());
        let config_json = std::fs::read(args.config.clone())?;
        let config: BiotrackerConfig = serde_json::from_slice(config_json.as_slice())?;
        let state = ExperimentState {
            target_fps: 25.0,
            arena: Some(config.arena.clone()),
            ..Default::default()
        };
        let mut biotracker = Self {
            args,
            state,
            config: config.clone(),
            _components: vec![],
            _server_thread,
            msg_bus,
            video_decoder: None,
            video_encoder: None,
        };
        for component in &config.components {
            biotracker.add_component(component.clone())?;
        }
        Ok(biotracker)
    }

    fn handle_registration(&mut self, component: libtracker::protocol::Component) {
        for config in &self.config.components {
            if config.id == component.id {
                let component_config = config.config_json.clone();
                self.msg_bus
                    .send(Message::ComponentMessage(ComponentMessage {
                        recipient_id: component.id.to_owned(),
                        content: Some(component_message::Content::ConfigJson(
                            component_config.to_string(),
                        )),
                    }))
                    .unwrap();
            }
        }
        self.state.registered_components.push(component);
        self.send_state_update().unwrap();
    }

    fn handle_component_message(&mut self, message: component_message::Content) {
        match message {
            component_message::Content::Registration(component) => {
                self.handle_registration(component);
            }
            component_message::Content::CommandJson(json) => {
                let command: BioTrackerCommand = serde_json::from_str(&json).unwrap();
                self.handle_command(command);
            }
            _ => {}
        }
    }

    fn handle_command(&mut self, command: BioTrackerCommand) {
        match command {
            BioTrackerCommand::PlaybackState(state) => self.set_playback_state(state).unwrap(),
            BioTrackerCommand::RecordingState(state) => self.set_recording_state(state).unwrap(),
            BioTrackerCommand::OpenVideo(path) => self.open_video(path).unwrap(),
            BioTrackerCommand::Seek(target) => self.seek(target).unwrap(),
            BioTrackerCommand::AddEntity => self.add_entity(),
            BioTrackerCommand::RemoveEntity => self.remove_entity(),
        }
    }

    pub fn run(mut self) {
        self.msg_bus
            .register_component(libtracker::protocol::Component {
                id: "BioTracker".to_owned(),
                typ: ComponentType::BiotrackerCore as i32,
            })
            .unwrap();

        if let Some(video) = &self.args.video {
            self.open_video(video.to_owned()).unwrap();
        }
        if let Some(count) = &self.args.entity_count {
            for _ in 0..*count {
                self.add_entity();
            }
        }
        loop {
            while let Some(message) = self.msg_bus.poll(0).unwrap() {
                match message {
                    Message::ComponentMessage(msg) => {
                        self.handle_component_message(msg.content.unwrap());
                        self.send_state_update().unwrap();
                    }
                    _ => {}
                }
            }
            if let Some(decoder) = &mut self.video_decoder {
                if self.state.playback_state == PlaybackState::Playing as i32 {
                    if let Some(image_msg) = decoder.next_frame(self.state.target_fps).unwrap() {
                        self.msg_bus
                            .send(Message::Image(image_msg.clone()))
                            .unwrap();
                    }
                }
            }
        }
    }

    fn send_state_update(&self) -> Result<()> {
        self.msg_bus
            .send(Message::ExperimentState(self.state.clone()))
    }

    fn open_video(&mut self, path: String) -> Result<()> {
        let video_decoder = VideoDecoder::new(path)?;
        self.state.video_info = Some(video_decoder.info.clone());
        self.video_decoder = Some(video_decoder);
        Ok(())
    }

    fn start_video_encoder(&mut self, filename: String) -> Result<()> {
        if let Some(video_info) = &self.state.video_info {
            let config = VideoEncoderConfig {
                video_path: filename,
                width: video_info.width,
                height: video_info.height,
                fps: video_info.fps,
                image_stream_id: "Annotated".to_owned(),
            };
            self.msg_bus
                .send(Message::ComponentMessage(ComponentMessage {
                    recipient_id: "VideoEncoder".to_owned(),
                    content: Some(component_message::Content::ConfigJson(
                        serde_json::to_string(&config).unwrap(),
                    )),
                }))?;
        }
        Ok(())
    }

    fn set_recording_state(&mut self, new_state: RecordingState) -> Result<()> {
        if self.state.recording_state == new_state as i32 {
            return Ok(());
        }
        match new_state {
            RecordingState::Recording => {
                if let Some(_) = &self.state.video_info {
                    self.state.recording_state = RecordingState::Recording as i32;
                    if let Some(video) = &self.args.save_video {
                        self.start_video_encoder(video.to_owned())?;
                    }
                }
                Ok(())
            }
            RecordingState::Finished | RecordingState::Initial => {
                self.state.recording_state = RecordingState::Finished as i32;
                self.video_encoder = None;
                Ok(())
            }
        }
    }

    fn set_playback_state(&mut self, new_state: PlaybackState) -> Result<()> {
        self.state.playback_state = new_state as i32;
        Ok(())
    }

    fn seek(&mut self, target: u32) -> Result<()> {
        if let Some(decoder) = &mut self.video_decoder {
            decoder.seek(target)?;
        }
        Ok(())
    }

    fn add_entity(&mut self) {
        self.state.entity_count += 1;
    }

    fn remove_entity(&mut self) {
        if self.state.entity_count > 0 {
            self.state.entity_count -= 1;
        }
    }

    fn add_component(&mut self, config: ComponentConfig) -> Result<ComponentThread> {
        if let Some(python_config) = &config.python_config {
            let (venv, cmd) = (python_config.venv.to_owned(), python_config.cmd.to_owned());
            let _thread = self.start_thread(|_msg_bus| PythonRunner::new(venv, cmd))?;
            return Ok(ComponentThread {
                _config: config,
                _thread,
            });
        }
        let _thread = match config.id.as_str() {
            "HungarianMatcher" => self.start_thread(Matcher::new)?,
            "VideoEncoder" => self.start_thread(VideoEncoder::new)?,
            _ => panic!("Unknown component type"),
        };
        Ok(ComponentThread {
            _config: config,
            _thread,
        })
    }

    fn start_thread<F, T: Component + 'static>(
        &mut self,
        component_builder: F,
    ) -> Result<JoinHandle<Result<()>>>
    where
        F: FnOnce(Client) -> T + Send + 'static,
    {
        let thread_name = std::any::type_name::<T>();
        let msg_bus = Client::new()?;
        let thread_handle = std::thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || -> Result<()> {
                let mut component = component_builder(msg_bus);
                match component.run() {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        eprintln!("Error in {}: {}", thread_name, e);
                        Err(e)
                    }
                }
            })?;
        Ok(thread_handle)
    }
}
