use super::component::ComponentConnections;
use super::undistort::UndistortMap;
use super::{
    arena::ArenaImpl, metric::DurationMetric, protocol::*, BiotrackerConfig, VideoDecoder,
    VideoEncoder,
};
use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct State {
    pub config: BiotrackerConfig,
    pub experiment: Experiment,
    pub track: Track,
    pub video_decoder: Option<Arc<Mutex<VideoDecoder>>>,
    pub video_encoder: Option<Arc<Mutex<VideoEncoder>>>,
    pub switch_request: Option<EntityIdSwitch>,
    pub undistortion: Option<UndistortMap>,
    pub metrics: Metrics,
    pub arena_impl: ArenaImpl,
    pub connections: ComponentConnections,
    entity_counter: u32,
}

#[derive(Default)]
pub struct Metrics {
    pub tracking_frame_time: DurationMetric,
    pub playback_frame_time: DurationMetric,
}

impl State {
    pub fn new(config: BiotrackerConfig) -> Self {
        let arena = match &config.arena {
            Some(arena) => arena.clone(),
            None => Arena {
                width_cm: 100,
                height_cm: 100,
                rectification_corners: vec![],
                tracking_area_corners: vec![],
            },
        };
        let components = config.components.clone();
        Self {
            experiment: Experiment {
                target_fps: 25.0,
                arena: Some(arena.clone()),
                playback_state: PlaybackState::Paused as i32,
                recording_state: RecordingState::Initial as i32,
                realtime_mode: true,
                tracking_metrics: Some(TrackingMetrics::default()),
                components,
                last_features: Some(Features::default()),
                undistort_mode: UndistortMode::None as i32,
                ..Default::default()
            },
            config,
            arena_impl: ArenaImpl::new(arena).unwrap(),
            ..Default::default()
        }
    }

    pub fn handle_image_result(&mut self, image: Image) {
        let tracking_metrics = self.experiment.tracking_metrics.as_mut().unwrap();
        self.experiment.last_image = Some(image.clone());
        tracking_metrics.playback_frame_time = self.metrics.playback_frame_time.update();
        if self.experiment.recording_state == RecordingState::Replay as i32 {
            if let Some(features) = self.track.features.get(&image.frame_number) {
                self.experiment.last_features = Some(features.clone());
            }
        }
    }

    pub fn handle_tracking_result(&mut self, frame_number: u32, features: Features) {
        let tracking_metrics = self.experiment.tracking_metrics.as_mut().unwrap();
        tracking_metrics.tracking_frame_time = self.metrics.tracking_frame_time.update();
        tracking_metrics.detected_features = features.features.len() as u32;
        self.track.features.insert(frame_number, features.clone());
        self.experiment.last_features = Some(features);
    }

    pub fn open_video(
        &mut self,
        path: String,
        force_undistortion: &Option<String>,
    ) -> Result<VideoInfo> {
        let decoder = VideoDecoder::new(
            path,
            self.experiment.target_fps as f64,
            &self.config.cameras,
        )?;
        let video_info = decoder.info.clone();

        let camera_config = match force_undistortion {
            Some(id) => self.config.cameras.iter().find(|c| c.id == *id),
            None => decoder.camera_config.as_ref(),
        };
        if let Some(config) = camera_config {
            self.undistortion = Some(UndistortMap::try_from((config, &video_info))?);
            if self.experiment.undistort_mode == UndistortMode::None as i32 {
                self.experiment.undistort_mode = UndistortMode::Poses as i32;
            }
        } else {
            self.experiment.undistort_mode = UndistortMode::None as i32;
        }
        let result = Ok(video_info.clone());
        self.experiment.video_info = Some(video_info);
        self.video_decoder = Some(Arc::new(Mutex::new(decoder)));
        self.experiment.playback_state = PlaybackState::Playing as i32;
        result
    }

    pub fn set_undistort_mode(&mut self, mode: i32) -> Result<()> {
        let mode = UndistortMode::from_i32(mode).context("Invalid undistort mode")?;
        if mode != UndistortMode::None && self.undistortion.is_none() {
            return Err(anyhow::anyhow!("No undistortion map configured"));
        }
        self.experiment.undistort_mode = mode as i32;
        Ok(())
    }

    pub fn open_track(&mut self, path: String) -> Result<()> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let track: Track = serde_json::from_reader(reader)?;
        self.seek(track.start_frame)?;
        self.track = track;
        self.experiment.recording_state = RecordingState::Replay as i32;
        Ok(())
    }

    pub fn save_track(&mut self) -> Result<()> {
        let recording_config = self
            .experiment
            .recording_config
            .as_ref()
            .context("Missing recording config")?;
        let path = std::path::PathBuf::from(&recording_config.base_path).with_extension("json");
        let file = std::fs::File::create(path)?;
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer, &self.track)?;
        Ok(())
    }

    pub fn close_decoder(&mut self) {
        self.video_decoder = None;
        self.experiment.video_info = None;
        self.experiment.playback_state = PlaybackState::Eos as i32;
        self.experiment.recording_state = RecordingState::Initial as i32;
    }

    pub fn add_entity(&mut self) -> Result<()> {
        self.entity_counter += 1;
        self.experiment.entity_ids.push(self.entity_counter);
        Ok(())
    }

    pub fn switch_entities(&mut self, request: EntityIdSwitch) -> Result<()> {
        if self.switch_request.is_some() {
            return Err(anyhow::anyhow!("Entity switch pending"));
        }
        self.switch_request = Some(request);
        Ok(())
    }

    pub fn set_recording_state(&mut self, recording_state: i32) -> Result<()> {
        match RecordingState::from_i32(recording_state) {
            Some(RecordingState::Recording) | Some(RecordingState::Initial) => {
                let start_frame = match &self.experiment.last_image {
                    Some(image) => image.frame_number,
                    None => 0,
                };
                self.track = Track {
                    start_frame,
                    ..Default::default()
                };
            }
            Some(RecordingState::Finished) => {
                self.video_encoder = None;
                self.experiment.recording_config = None;
            }
            _ => {}
        };
        self.experiment.recording_state = recording_state;
        Ok(())
    }

    pub fn initialize_recording(&mut self, config: RecordingConfig) -> Result<()> {
        let encoder = VideoEncoder::new(config.clone())?;
        self.experiment.recording_config = Some(config);
        self.video_encoder = Some(Arc::new(Mutex::new(encoder)));
        Ok(())
    }

    pub fn remove_entity(&mut self) -> Result<()> {
        self.experiment.entity_ids.pop();
        Ok(())
    }

    pub fn seek(&mut self, frame: u32) -> Result<()> {
        if let Some(decoder) = &mut self.video_decoder {
            decoder.lock().unwrap().seek(frame)?;
        }
        Ok(())
    }

    pub fn set_playback_state(&mut self, playback_state: i32) -> Result<()> {
        self.experiment.playback_state = playback_state;
        Ok(())
    }

    pub fn update_arena(&mut self, arena: Arena) -> Result<()> {
        self.arena_impl = ArenaImpl::new(arena.clone())?;
        self.experiment.arena = Some(arena);
        Ok(())
    }

    pub fn update_component(&mut self, component: ComponentConfig) -> Result<()> {
        for c in &mut self.experiment.components {
            if c.id == component.id {
                *c = component;
                return Ok(());
            }
        }
        Err(anyhow::anyhow!("Component not found"))
    }

    pub fn save_config(&mut self, path: &str) -> Result<()> {
        self.config.arena = Some(self.arena_impl.arena.clone());
        self.config.components = self.experiment.components.clone();
        self.config.save(path)?;
        Ok(())
    }

    pub fn get_undistortion(&self, mode: UndistortMode) -> Option<UndistortMap> {
        if mode as i32 == self.experiment.undistort_mode {
            self.undistortion.clone()
        } else {
            None
        }
    }
}
