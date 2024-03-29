use super::{
    protocol::*, tracking::start_tracking_task, BiotrackerConfig, ChannelRequest,
    CommandLineArguments, Service, State,
};
use crate::{biotracker::observer::start_observer_task, log_error};
use anyhow::{Context, Result};
use bio_tracker_server::BioTrackerServer;
use metrics::{describe_counter, describe_histogram};
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
}

impl Core {
    pub async fn new(args: &CommandLineArguments, config: BiotrackerConfig) -> Result<Self> {
        let args = Arc::new(args.clone());
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
            args,
            command_rx,
            image_rx,
            state,
            state_rx,
        })
    }

    async fn init(&mut self) -> Result<()> {
        let components = self.state.config.components.clone();
        let port_range_start = self.args.port_range_start;
        self.state
            .connections
            .start_components(components, port_range_start)
            .await?;

        if let Some(video) = self.args.video.clone() {
            log_error!(self.state.open_video(video, &self.args.force_camera_config));
            log_error!(self.state.set_playback_state(PlaybackState::Playing as i32));
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
        // initialize metric descriptions
        describe_histogram!("latency.tracking", "Tracking");
        describe_histogram!("latency.observers", "Observer update");
        describe_histogram!("latency.matcher", "Matching");
        describe_histogram!("latency.feature_detector", "Feature detection");
        describe_histogram!("latency.image_acquisition", "Image acquisition");
        describe_histogram!("latency.video_encoding", "Video encoding");
        describe_histogram!("latency.playback", "Video playback");

        describe_counter!("count.frame_tracked", "Number of tracked frames");
        describe_counter!("count.frame_encode", "Number of encoded frames");
        describe_counter!("count.frame_decode", "Number of acquired frames");
        describe_counter!("count.frame_dropped", "Number of dropped frames");
        describe_counter!(
            "count.NaN_features_removed",
            "Number of invalid animal features"
        );
        describe_counter!(
            "count.oob_features_removed",
            "Number of out-of-bounds features"
        );
        describe_counter!(
            "count.confidence_features_removed",
            "Number of animal features not passing confidence threshold"
        );
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
        let mut observer_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut encoder_task = None;
        let mut last_frame_start = std::time::Instant::now();
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
                biased;
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
                            self.state.experiment.last_features = None;
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
                _ = image_timer => {
                    if self.state.experiment.playback_state == PlaybackState::Playing as i32 &&
                        (self.state.experiment.realtime_mode || tracking_task.is_none()) {
                        if decoder_task.is_some() {
                            metrics::increment_counter!("count.playback_dropped_frames");
                        } else {
                            metrics::histogram!("latency.playback", last_frame_start.elapsed());
                            last_frame_start = std::time::Instant::now();
                            self.start_decoder_task(&mut decoder_task, &decoder_tx);
                        }
                    } else if self.state.experiment.playback_state == PlaybackState::Paused as i32 {
                            last_frame_start = std::time::Instant::now();
                    }
                }
                Some(image_request) = self.image_rx.recv() => {
                    self.start_encoder_task(&mut encoder_task, &image_request.request).await;
                    image_request.result_tx.send(Ok(Empty {})).unwrap();
                }
                Some(image_result) = decoder_rx.recv() => {
                    decoder_task = None;
                    match image_result {
                        Ok(image) => {
                            self.state.handle_image_result(image.clone());
                            if tracking_task.is_none() {
                                start_tracking_task(
                                    &self.state,
                                    &mut tracking_task,
                                    &tracking_tx,
                                    &image);
                            }
                            self.start_encoder_task(&mut encoder_task, &image).await;
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
                        Ok(result) => {
                            let frame_number = result.frame_number;
                            self.state.handle_tracking_result(result);
                            if let Some(image) =  &self.state.experiment.last_image {
                                if image.frame_number != frame_number {
                                    start_tracking_task(
                                        &self.state,
                                        &mut tracking_task,
                                        &tracking_tx,
                                        &image);
                                }
                            }
                            start_observer_task(&self.state, &mut observer_task);
                        }
                        Err(e) => {
                            log::warn!("Tracking failed: {}", e);
                        }
                    }
                }
                _ = self.state.connections.update_connections(),
                    if self.state.connections.has_pending_connections() => {}
                Some(state_request) = self.state_rx.recv() => {
                    state_request.result_tx.send(self.state.experiment.clone()).unwrap();
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
                if state == RecordingState::Finished as i32 {
                    self.finish_recording().await?;
                }
                self.state.set_recording_state(state)?;
            }
            Command::RealtimeMode(wait) => {
                self.state.experiment.realtime_mode = wait;
            }
            Command::UndistortMode(mode) => {
                self.state.set_undistort_mode(mode)?;
            }
            Command::Seek(frame) => {
                self.state.seek(frame)?;
            }
            Command::OpenVideo(path) => {
                self.state
                    .open_video(path, &self.args.force_camera_config)?;
            }
            Command::OpenTrack(path) => {
                self.state.open_track(path)?;
            }
            Command::SaveTrack(path) => {
                self.state.save_track(&path)?;
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
                self.state.switch_entities(switch_request).await?;
            }
            Command::TargetFps(fps) => {
                if fps <= 0.0 {
                    return Err(anyhow::anyhow!("FPS must > 0"));
                }
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

    async fn start_encoder_task(
        &mut self,
        encoder_task: &mut Option<JoinHandle<()>>,
        image: &Image,
    ) {
        if self.state.experiment.recording_state != RecordingState::Recording as i32 {
            return;
        }

        if let Some(config) = &self.state.experiment.recording_config {
            let start = std::time::Instant::now();
            if config.image_stream_id != image.stream_id {
                return;
            }

            let encoder = self
                .state
                .video_encoder
                .clone()
                .expect("VideoEncoder not running");
            let image = image.clone();
            if let Some(task) = encoder_task.take() {
                task.await.expect("Error while encoding image");
            }
            *encoder_task = Some(tokio::task::spawn_blocking(move || {
                encoder
                    .lock()
                    .unwrap()
                    .add_frame(image)
                    .expect("Error while encoding image");
                metrics::histogram!("latency.video_encoding", start.elapsed());
                metrics::increment_counter!("count.frame_encode");
            }));
        }
    }

    fn start_decoder_task(
        &mut self,
        decoder_task: &mut Option<tokio::task::JoinHandle<()>>,
        result_tx: &Sender<Result<Image>>,
    ) {
        let start = std::time::Instant::now();
        let result_tx = result_tx.clone();

        if let Some(decoder) = &self.state.video_decoder {
            let decoder = decoder.clone();
            if decoder_task.is_none() {
                let undistortion = self.state.get_undistortion(UndistortMode::Image);
                *decoder_task = Some(tokio::task::spawn_blocking(move || {
                    let mut decoder = decoder.lock().unwrap();
                    if decoder.end_of_stream() {
                        return;
                    }
                    let _ = result_tx.blocking_send(decoder.get_image(undistortion));
                    metrics::histogram!("latency.image_acquisition", start.elapsed());
                    metrics::increment_counter!("count.frame_decode");
                }));
            }
        }
    }

    async fn finish_recording(&mut self) -> Result<()> {
        let recording_config = self
            .state
            .experiment
            .recording_config
            .as_ref()
            .context("Missing recording config")?;
        let track_path = format!("{}.json", recording_config.base_path);
        log_error!(self.state.save_track(&track_path));
        Ok(())
    }
}
