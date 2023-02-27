use super::{
    protocol::*, ChannelRequest, CommandLineArguments, ComponentConfig, MatcherService,
    PythonProcess, RobofishCommander, Service, State,
};
use anyhow::Result;
use bio_tracker_server::BioTrackerServer;
use matcher_server::MatcherServer;
use std::sync::Arc;
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinHandle,
};
use tonic::transport::{Channel as ClientChannel, Server};

pub struct Core {
    args: Arc<CommandLineArguments>,
    command_rx: Receiver<ChannelRequest<Command, Result<Empty>>>,
    image_rx: Receiver<ChannelRequest<Image, Result<Empty>>>,
    python_processes: Vec<PythonProcess>,
    state: State,
    state_rx: Receiver<ChannelRequest<(), Experiment>>,
    robofish_commander_bridge: RobofishCommander,
}

enum GrpcClient {
    Matcher(MatcherClient<ClientChannel>),
    FeatureDetector(FeatureDetectorClient<ClientChannel>),
    TrackRecorder(TrackRecorderClient<ClientChannel>),
}

impl GrpcClient {
    pub fn new(channel: ClientChannel, service: &str) -> Result<Self> {
        let service =
            ServiceType::from_str_name(service).ok_or(anyhow::anyhow!("Invalid service name"))?;
        match service {
            ServiceType::Matcher => Ok(Self::Matcher(MatcherClient::new(channel))),
            ServiceType::FeatureDetector => {
                Ok(Self::FeatureDetector(FeatureDetectorClient::new(channel)))
            }
            ServiceType::TrackRecorder => {
                Ok(Self::TrackRecorder(TrackRecorderClient::new(channel)))
            }
            ServiceType::BiotrackerCore => Err(anyhow::anyhow!("Invalid service name")),
        }
    }
}

impl Core {
    pub async fn new(args: &CommandLineArguments) -> Result<Self> {
        let args = Arc::new(args.clone());
        let config_json = std::fs::read(args.config.clone())?;
        let config = serde_json::from_slice(config_json.as_slice())?;
        let state = State::new(config);
        let (command_tx, command_rx) = channel(1);
        let (state_tx, state_rx) = channel(1);
        let (image_tx, image_rx) = channel(1);

        let biotracker_server = BioTrackerServer::new(Service {
            command_tx,
            state_tx,
            image_tx,
        });
        let address = format!("127.0.0.1:{}", args.port).parse()?;
        tokio::spawn(async move {
            match Server::builder()
                .add_service(biotracker_server)
                .serve(address)
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to start BioTracker Service: {}", e);
                }
            };
        });
        Ok(Self {
            robofish_commander_bridge: RobofishCommander::new(args.robofish_port).await?,
            args,
            command_rx,
            image_rx,
            python_processes: vec![],
            state,
            state_rx,
        })
    }

    pub async fn init(&mut self) -> Result<()> {
        let components = self.state.config.components.clone();
        for component in &components {
            self.start_component(component.clone()).await?;
        }
        for component in &components {
            for client in self.connect_component(&component).await? {
                match client {
                    GrpcClient::Matcher(mut client) => {
                        client
                            .set_config(ComponentConfiguration {
                                config_json: component.config_json.to_string(),
                            })
                            .await?;
                        self.state.matcher = Some(client);
                    }
                    GrpcClient::FeatureDetector(mut client) => {
                        client
                            .set_config(ComponentConfiguration {
                                config_json: component.config_json.to_string(),
                            })
                            .await?;
                        self.state.feature_detector = Some(client);
                    }
                    GrpcClient::TrackRecorder(mut client) => {
                        client
                            .set_config(ComponentConfiguration {
                                config_json: component.config_json.to_string(),
                            })
                            .await?;
                        self.state.track_recorder = Some(client);
                    }
                }
            }
        }

        if self.state.matcher.is_none() {
            return Err(anyhow::anyhow!("No matcher in config"));
        }
        if self.state.feature_detector.is_none() {
            return Err(anyhow::anyhow!("No feature detector in config"));
        }
        if self.state.track_recorder.is_none() {
            return Err(anyhow::anyhow!("No track recorder in config"));
        }

        if let Some(video) = &self.args.video {
            match self.state.open_video(video.to_owned()) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to open video: {}", e);
                }
            }
        }

        if let Some(seek) = &self.args.seek {
            match self.state.seek(seek.to_owned()) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to seek: {}", e);
                }
            }
        }

        if let Some(count) = &self.args.entity_count {
            for _ in 0..*count {
                self.state.add_entity()?;
            }
        }

        if let Some(realtime) = &self.args.realtime {
            self.state.experiment.realtime_mode = *realtime;
        }
        Ok(())
    }

    pub async fn finish(&mut self, tasks: &[&Option<JoinHandle<()>>]) -> Result<Empty> {
        if self.state.experiment.recording_state == RecordingState::Recording as i32 {
            self.finish_recording().await?;
        }
        for task in tasks {
            if let Some(task) = task {
                task.abort();
            }
        }
        self.python_processes.clear();
        Ok(Empty {})
    }

    pub async fn run(mut self) -> Result<()> {
        self.init().await?;
        let fps = self.state.experiment.target_fps;
        let mut fps_interval = tokio::time::interval(std::time::Duration::from_secs_f64(1.0 / fps));

        let mut decoder_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut tracking_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut encoder_task = None;
        let (decoder_tx, mut decoder_rx) = channel(16);
        let (tracking_tx, mut tracking_rx) = channel(16);

        loop {
            let image_timer = fps_interval.tick();

            tokio::select! {
                Some(command) = self.command_rx.recv() => {
                    let result = self.handle_command(command.request.clone()).await;
                    match command.request {
                        Command::Seek(_) => {
                            if let Some(task) = decoder_task.take() {
                                task.abort();
                            }
                            if let Some(task) = tracking_task.take() {
                                task.abort();
                            }
                            self.start_decoder_task(&mut decoder_task, &decoder_tx);
                        },
                        Command::Shutdown(_) => {
                            self.finish(&[&decoder_task, &tracking_task, &encoder_task])
                                .await?;
                            command.result_tx.send(Ok(Empty {})).unwrap();
                            break;
                        },
                        _ => {}
                    }
                    command.result_tx.send(result).unwrap();
                }
                Some(state_request) = self.state_rx.recv() => {
                    state_request.result_tx.send(self.state.experiment.clone()).unwrap();
                }
                _ = image_timer => {
                    if self.state.experiment.playback_state == PlaybackState::Playing as i32 &&
                        (self.state.experiment.realtime_mode || tracking_task.is_none()) {
                        if decoder_task.is_some() {
                            eprintln!("VideoDecoder too slow, dropping frame");
                            self.state
                                .experiment
                                .tracking_metrics
                                .as_mut()
                                .unwrap()
                                .playback_dropped_frames += 1;
                        } else {
                            self.start_decoder_task(&mut decoder_task, &decoder_tx);
                        }
                    }
                }
                Some(image_request) = self.image_rx.recv() => {
                    self.start_encoder_task(&mut encoder_task, &image_request.request);
                    image_request.result_tx.send(Ok(Empty {})).unwrap();
                }
                Some(image_result) = decoder_rx.recv() => {
                    decoder_task = None;
                    match image_result {
                        Ok(image) => {
                            self.state.handle_image_result(image.clone());
                            if tracking_task.is_none() {
                                let switch_request = self.state.switch_request.take();
                                self.start_tracking_task(
                                    switch_request,
                                    &mut tracking_task,
                                    &tracking_tx,
                                    &image);
                            }
                            self.start_encoder_task(&mut encoder_task, &image);
                        }
                        Err(e) => {
                            eprintln!("Error while decoding image: {}", e);
                            self.state.close_decoder();
                        }
                    }
                }
                Some(tracking_result) = tracking_rx.recv() => {
                    tracking_task = None;
                    match tracking_result {
                        Ok((frame_number, features, entities)) => {
                            self.robofish_commander_bridge.send(
                                &entities,
                                &self.state.experiment.arena,
                                frame_number,
                                self.state.experiment.target_fps
                            ).await?;
                            self.state.handle_tracking_result(frame_number, features, entities);
                            if let Some(image) =  &self.state.experiment.last_image {
                                if image.frame_number != frame_number {
                                    let switch_request = self.state.switch_request.take();
                                    self.start_tracking_task(
                                        switch_request,
                                        &mut tracking_task,
                                        &tracking_tx,
                                        &image);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error while tracking: {}", e);
                        }
                    }
                }
                _ = self.robofish_commander_bridge.accept() => {
                    eprintln!("Robofish commander connected");
                }
            }
        }
        Ok(())
    }

    async fn handle_command(&mut self, command: Command) -> Result<Empty> {
        match command {
            Command::PlaybackState(state) => {
                self.state.set_playback_state(state)?;
            }
            Command::RecordingState(state) => {
                self.state.set_recording_state(state)?;
                if state == RecordingState::Finished as i32 {
                    self.finish_recording().await?;
                }
            }
            Command::RealtimeMode(wait) => {
                self.state.experiment.realtime_mode = wait;
            }
            Command::Seek(frame) => {
                self.state.seek(frame)?;
            }
            Command::OpenVideo(path) => {
                self.state.open_video(path)?;
            }
            Command::OpenTrack(path) => {
                self.open_track(path).await?;
            }
            Command::VideoEncoderConfig(config) => {
                self.state.initialize_video_encoder(config)?;
            }
            Command::AddEntity(_) => {
                self.state.add_entity()?;
            }
            Command::RemoveEntity(_) => {
                self.state.remove_entity()?;
            }
            Command::SwitchEntities(switch_request) => {
                self.state.switch_entities(switch_request)?;
            }
            Command::Shutdown(_) => {}
        }
        Ok(Empty {})
    }

    fn start_encoder_task(&mut self, encoder_task: &mut Option<JoinHandle<()>>, image: &Image) {
        if self.state.experiment.recording_state != RecordingState::Recording as i32 {
            return;
        }

        let encoder_config = &self
            .state
            .experiment
            .video_encoder_config
            .as_ref()
            .expect("VideoEncoder Config not set");
        if encoder_config.image_stream_id != image.stream_id {
            return;
        }

        if let Some(task) = &encoder_task {
            if !task.is_finished() {
                self.state
                    .experiment
                    .tracking_metrics
                    .as_mut()
                    .unwrap()
                    .encoder_dropped_frames += 1;
                eprintln!(
                    "VideoEncoder too slow, dropping frame {}",
                    image.frame_number
                );
                return;
            }
        }

        let encoder = self
            .state
            .video_encoder
            .clone()
            .expect("VideoEncoder not running");
        let image = image.clone();
        *encoder_task = Some(tokio::task::spawn_blocking(move || {
            encoder
                .lock()
                .unwrap()
                .add_image(image)
                .expect("Error while encoding image");
        }));
    }

    fn start_tracking_task(
        &self,
        entity_switch_request: Option<EntityIdSwitch>,
        tracking_task: &mut Option<tokio::task::JoinHandle<()>>,
        tracking_tx: &tokio::sync::mpsc::Sender<Result<(u32, Features, Entities)>>,
        image: &Image,
    ) {
        let image = image.clone();
        let detector = self
            .state
            .feature_detector
            .clone()
            .expect("Feature Detector not running");
        let matcher = self.state.matcher.clone().unwrap();
        let last_entities = self
            .state
            .experiment
            .last_entities
            .as_ref()
            .expect("last_entities is None");

        let mut tracking_entities = Entities::default();
        for id in &self.state.experiment.entity_ids {
            if let Some(entity) = last_entities.entities.iter().find(|e| e.id == *id) {
                tracking_entities.entities.push(entity.clone());
            } else {
                tracking_entities.entities.push(Entity {
                    id: *id,
                    feature: None,
                    frame_number: 0,
                });
            }
        }

        if let Some(switch_request) = entity_switch_request {
            let (mut first_idx, mut second_idx) = (None, None);
            for (idx, entity) in tracking_entities.entities.iter().enumerate() {
                if entity.id == switch_request.id1 {
                    first_idx = Some(idx);
                }
                if entity.id == switch_request.id2 {
                    second_idx = Some(idx);
                }
            }

            if let (Some(first_idx), Some(second_idx)) = (first_idx, second_idx) {
                tracking_entities.entities[first_idx].id = switch_request.id2;
                tracking_entities.entities[second_idx].id = switch_request.id1;
            }
        }

        let arena = self.state.experiment.arena.clone();
        let tracking_tx = tracking_tx.clone();
        *tracking_task = Some(tokio::spawn(async move {
            let result =
                Core::tracking_task(image, detector, matcher, arena, tracking_entities).await;
            tracking_tx.send(result).await.unwrap();
        }));
    }

    async fn tracking_task(
        image: Image,
        mut detector: FeatureDetectorClient<tonic::transport::Channel>,
        mut matcher: MatcherClient<tonic::transport::Channel>,
        arena: Option<Arena>,
        last_entities: Entities,
    ) -> Result<(u32, Features, Entities)> {
        let frame_number = image.frame_number;
        let detector_request = DetectorRequest {
            image: Some(image),
            arena,
        };
        let features = detector
            .detect_features(detector_request)
            .await?
            .into_inner();
        let matcher_request = MatcherRequest {
            features: Some(features.clone()),
            last_entities: Some(last_entities),
            frame_number,
        };
        let entities = matcher.match_features(matcher_request).await?.into_inner();
        Ok((frame_number, features, entities))
    }

    fn start_decoder_task(
        &self,
        decoder_task: &mut Option<tokio::task::JoinHandle<()>>,
        result_tx: &Sender<Result<Image>>,
    ) {
        let result_tx = result_tx.clone();
        if let Some(decoder) = &self.state.video_decoder {
            let decoder = decoder.clone();
            if decoder_task.is_none() {
                *decoder_task = Some(tokio::task::spawn_blocking(move || {
                    let _ = result_tx.blocking_send(decoder.lock().unwrap().get_image());
                }));
            }
        }
    }

    async fn open_track(&mut self, path: String) -> Result<()> {
        let tracks_response = self
            .state
            .track_recorder
            .as_mut()
            .expect("track recorder not running")
            .load(TrackLoadRequest { load_path: path })
            .await?;
        self.state.tracks = tracks_response.into_inner().tracks;
        Ok(())
    }

    async fn finish_recording(&mut self) -> Result<()> {
        let save_request = TrackSaveRequest {
            experiment: Some(self.state.experiment.clone()),
            tracks: self.state.tracks.clone(),
            save_path: "test".to_string(),
        };
        self.state
            .track_recorder
            .as_mut()
            .expect("TrackRecorder not running")
            .save(save_request)
            .await?;
        Ok(())
    }

    async fn start_component(&mut self, config: ComponentConfig) -> Result<()> {
        let address = config.address.to_owned();
        if let Some(python_config) = &config.python_config {
            let process = PythonProcess::new(&config, python_config)?;
            self.python_processes.push(process);
        } else {
            match config.id.as_str() {
                "HungarianMatcher" => {
                    tokio::spawn(async move {
                        let matcher_service = Arc::new(MatcherService::new());
                        let matcher_server = MatcherServer::from_arc(matcher_service.clone());
                        match Server::builder()
                            .add_service(matcher_server)
                            .serve(address.parse().expect("Invalid address"))
                            .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("HungarianMatcher failed: {}", e);
                            }
                        };
                    });
                }
                _ => panic!("Unknown component {}", config.id),
            };
        };
        Ok(())
    }

    async fn poll_connect(&self, addr: &str) -> Result<tonic::transport::Channel> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            let addr = addr.to_owned();
            match tonic::transport::Endpoint::new(addr)?.connect().await {
                Ok(conn) => return Ok(conn),
                Err(_) => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        Err(anyhow::anyhow!("Could not connect to {}", addr))
    }

    async fn connect_component(&self, config: &ComponentConfig) -> Result<Vec<GrpcClient>> {
        let mut clients = vec![];
        let address = format!("http://{}", config.address);
        let channel = self.poll_connect(&address).await?;
        for service in &config.services {
            clients.push(GrpcClient::new(channel.clone(), service)?);
        }
        Ok(clients)
    }
}
