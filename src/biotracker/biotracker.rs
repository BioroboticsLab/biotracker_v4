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
        let biotracker_address = "[::1]:33072".parse().unwrap();
        tokio::spawn(async move {
            Server::builder()
                .add_service(biotracker_server)
                .serve(biotracker_address)
                .await
                .unwrap();
        });

        Ok(Self {
            args,
            command_rx,
            image_rx,
            python_processes: vec![],
            state,
            state_rx,
            robofish_commander_bridge: RobofishCommander::new().await?,
        })
    }

    pub async fn init(&mut self) -> Result<()> {
        let components = self.state.config.components.clone();
        for component in &components {
            self.start_component(component.clone()).await.unwrap();
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
            self.state.open_video(video.to_owned()).unwrap();
        }

        if let Some(count) = &self.args.entity_count {
            for _ in 0..*count {
                self.state.add_entity().unwrap();
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
        fps_interval.tick().await;

        let mut shutdown = false;
        let mut decoder_task = None;
        let mut tracking_task = None;
        let mut encoder_task = None;
        let (decoder_tx, mut decoder_rx) = channel(16);
        let (tracking_tx, mut tracking_rx) = channel(16);

        while !shutdown {
            let image_timer = fps_interval.tick();

            tokio::select! {
                Some(command) = self.command_rx.recv() => {
                    let result = self.handle_command(command.request, &mut shutdown).await;
                    if shutdown {
                        self.finish(&[&decoder_task, &tracking_task, &encoder_task])
                            .await.unwrap();
                    }
                    command.result_tx.send(result).unwrap()
                }
                Some(state_request) = self.state_rx.recv() => {
                    state_request.result_tx.send(self.state.experiment.clone()).unwrap();
                }
                _ = image_timer => {
                    if self.state.experiment.realtime_mode || tracking_task.is_none() {
                        self.start_decoder_task(&mut decoder_task, &decoder_tx);
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
                            self.state.experiment.last_image = Some(image.clone());
                            self.start_tracking_task(
                                &mut tracking_task,
                                &tracking_tx,
                                &image);
                            self.start_encoder_task(&mut encoder_task, &image);
                        }
                        Err(e) => {
                            eprintln!("Error while decoding image: {}", e);
                            self.state.close_decoder();
                        }
                    }
                }
                Some(tracking_result) = tracking_rx.recv() => {
                    // TODO: start next tracking task here, if there is a new image available
                    tracking_task = None;
                    match tracking_result {
                        Ok((frame_number, features, entities)) => {
                            self.robofish_commander_bridge.send(
                                &entities,
                                &self.state.experiment.arena,
                                frame_number,
                                self.state.experiment.target_fps
                            ).await?;
                            self.state.handle_tracking_result(features, entities);
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

    async fn handle_command(&mut self, command: Command, shutdown: &mut bool) -> Result<Empty> {
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
            Command::VideoEncoderConfig(config) => {
                self.state.initialize_video_encoder(config)?;
            }
            Command::AddEntity(_) => {
                self.state.add_entity()?;
            }
            Command::RemoveEntity(_) => {
                self.state.remove_entity()?;
            }
            Command::Shutdown(_) => {
                *shutdown = true;
            }
        }
        Ok(Empty {})
    }

    fn start_encoder_task(&mut self, encoder_task: &mut Option<JoinHandle<()>>, image: &Image) {
        if let Some(task) = &encoder_task {
            if !task.is_finished() {
                eprintln!(
                    "VideoEncoder too slow, dropping frame {}",
                    image.frame_number
                );
                return;
            }
        }

        if self.state.experiment.recording_state != RecordingState::Recording as i32 {
            return;
        }

        let encoder_config = &self.state.experiment.video_encoder_config.as_ref().unwrap();
        if encoder_config.image_stream_id != image.stream_id {
            return;
        }

        let encoder = self.state.video_encoder.clone().unwrap();
        let image = image.clone();
        *encoder_task = Some(tokio::task::spawn_blocking(move || {
            encoder.lock().unwrap().add_image(image).unwrap();
        }));
    }

    fn start_tracking_task(
        &self,
        tracking_task: &mut Option<tokio::task::JoinHandle<()>>,
        tracking_tx: &tokio::sync::mpsc::Sender<Result<(u32, Features, Entities)>>,
        image: &Image,
    ) {
        if tracking_task.is_some() {
            return;
        }

        let image = image.clone();
        let detector = self.state.feature_detector.clone().unwrap();
        let matcher = self.state.matcher.clone().unwrap();
        let last_entities = self.state.experiment.last_entities.clone();
        let arena = self.state.experiment.arena.clone();
        let tracking_tx = tracking_tx.clone();
        *tracking_task = Some(tokio::spawn(async move {
            let result = Core::tracking_task(image, detector, matcher, arena, last_entities).await;
            tracking_tx.send(result).await.unwrap();
        }));
    }

    async fn tracking_task(
        image: Image,
        mut detector: FeatureDetectorClient<tonic::transport::Channel>,
        mut matcher: MatcherClient<tonic::transport::Channel>,
        arena: Option<Arena>,
        last_entities: Option<Entities>,
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
            last_entities,
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
        if decoder_task.is_some() {
            eprintln!("VideoDecoder too slow, dropping frame");
            return;
        }

        if self.state.experiment.playback_state != PlaybackState::Playing as i32 {
            return;
        }

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

    async fn finish_recording(&mut self) -> Result<()> {
        let save_request = TrackSaveRequest {
            experiment: Some(self.state.experiment.clone()),
            tracks: self.state.tracks.clone(),
            save_path: "test".to_string(),
        };
        self.state
            .track_recorder
            .as_mut()
            .unwrap()
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
                        Server::builder()
                            .add_service(matcher_server)
                            .serve(address.parse().unwrap())
                            .await
                            .unwrap();
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
