use super::{
    protocol::*, tracking::start_tracking_task, BiotrackerConfig, ChannelRequest,
    CommandLineArguments, RobofishCommander, Service, State,
};
use crate::log_error;
use anyhow::{Context, Result};
use bio_tracker_server::BioTrackerServer;
use std::sync::Arc;
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinHandle,
};
use tonic::transport::Server;

pub struct Core {
    args: Arc<CommandLineArguments>,
    command_rx: Receiver<ChannelRequest<Command, Result<Empty>>>,
    image_rx: Receiver<ChannelRequest<Image, Result<Empty>>>,
    state: State,
    state_rx: Receiver<ChannelRequest<(), Experiment>>,
    robofish_commander_bridge: RobofishCommander,
}

impl Core {
    pub async fn new(args: &CommandLineArguments) -> Result<Self> {
        let args = Arc::new(args.clone());
        let config = BiotrackerConfig::load(&args.config)?;
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
                    log::error!("Failed to start BioTracker Service: {}", e);
                }
            };
        });
        Ok(Self {
            robofish_commander_bridge: RobofishCommander::new(args.robofish_port).await?,
            args,
            command_rx,
            image_rx,
            state,
            state_rx,
        })
    }

    async fn init(&mut self) -> Result<()> {
        let components = self.state.config.components.clone();
        self.state.connections.start_components(components).await?;

        if let Some(video) = &self.args.video {
            log_error!(self.state.open_video(video.to_owned()));
        }

        if let Some(seek) = &self.args.seek {
            log_error!(self.state.seek(seek.to_owned()));
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
        self.state.connections.stop_components().await?;
        Ok(Empty {})
    }

    pub async fn run(&mut self) -> Result<()> {
        match self.init().await {
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to initialize: {}", e);
                return Err(e);
            }
        }

        let mut fps = self.state.experiment.target_fps;
        let mut fps_interval =
            tokio::time::interval(std::time::Duration::from_secs_f64(1.0 / fps as f64));

        let mut decoder_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut tracking_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut encoder_task = None;
        let (decoder_tx, mut decoder_rx) = channel(16);
        let (tracking_tx, mut tracking_rx) = channel(16);

        loop {
            if fps != self.state.experiment.target_fps {
                fps = self.state.experiment.target_fps;
                fps_interval =
                    tokio::time::interval(std::time::Duration::from_secs_f64(1.0 / fps as f64));
            }
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
                            log::warn!("VideoDecoder too slow, dropping frame");
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
                                start_tracking_task(
                                    &self.state,
                                    switch_request,
                                    &mut tracking_task,
                                    &tracking_tx,
                                    &image);
                            }
                            self.start_encoder_task(&mut encoder_task, &image);
                        }
                        Err(e) => {
                            log::error!("Error while decoding image: {}", e);
                            self.state.close_decoder();
                        }
                    }
                }
                Some(tracking_result) = tracking_rx.recv() => {
                    tracking_task = None;
                    match tracking_result {
                        Ok((frame_number, features)) => {
                            self.robofish_commander_bridge.send(
                                &features,
                                &self.state.experiment.arena,
                                frame_number,
                                self.state.experiment.target_fps
                            ).await?;
                            self.state.handle_tracking_result(frame_number, features);
                            if let Some(image) =  &self.state.experiment.last_image {
                                if image.frame_number != frame_number {
                                    let switch_request = self.state.switch_request.take();
                                    start_tracking_task(
                                        &self.state,
                                        switch_request,
                                        &mut tracking_task,
                                        &tracking_tx,
                                        &image);
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Tracking failed: {}", e);
                        }
                    }
                }
                _ = self.robofish_commander_bridge.accept() => {
                    log::info!("Robofish commander connected");
                }
                _ = self.state.connections.update_connections(),
                    if self.state.connections.has_pending_connections() => {}
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
                if state == RecordingState::Finished as i32 {
                    self.finish_recording().await?;
                }
                self.state.set_recording_state(state)?;
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
                self.state.open_track(path)?;
            }
            Command::InitializeRecording(config) => {
                self.state.initialize_recording(config)?;
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
            Command::TargetFps(fps) => {
                self.state.experiment.target_fps = fps;
            }
            Command::UpdateArena(arena) => {
                self.state.update_arena(arena)?;
            }
            Command::UpdateComponent(config) => {
                self.state.connections.set_config(config.clone()).await?;
                self.state.update_component(config)?;
            }
            Command::SaveConfig(_) => {
                self.state.save_config(&self.args.config)?;
            }
            Command::Shutdown(_) => {}
        }
        Ok(Empty {})
    }

    fn start_encoder_task(&mut self, encoder_task: &mut Option<JoinHandle<()>>, image: &Image) {
        if self.state.experiment.recording_state != RecordingState::Recording as i32 {
            return;
        }

        if let Some(config) = &self.state.experiment.recording_config {
            if config.image_stream_id != image.stream_id {
                return;
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
                    .add_frame(image)
                    .expect("Error while encoding image");
            }));
        }
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

    async fn finish_recording(&mut self) -> Result<()> {
        let save_request = TrackSaveRequest {
            experiment: Some(self.state.experiment.clone()),
            track: Some(self.state.track.clone()),
        };
        log_error!(self.state.save_track());
        self.state
            .connections
            .track_recorder()
            .context("TrackRecorder not connected")?
            .save(save_request)
            .await?;
        Ok(())
    }
}
