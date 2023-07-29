use super::component::ComponentConnections;
use super::tracking::TrackingResult;
use super::undistort::UndistortMap;
use super::{arena::ArenaImpl, protocol::*, BiotrackerConfig, VideoDecoder, VideoEncoder};
use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct State {
    pub config: BiotrackerConfig,
    pub experiment: Experiment,
    pub track: Track,
    pub video_decoder: Option<Arc<Mutex<VideoDecoder>>>,
    pub video_encoder: Option<Arc<Mutex<VideoEncoder>>>,
    pub undistortion: Option<UndistortMap>,
    pub arena_impl: ArenaImpl,
    pub connections: ComponentConnections,
    recording_start_frame: u32,
    entity_counter: u32,
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
                target_fps: 30.0,
                arena: Some(arena.clone()),
                playback_state: PlaybackState::Paused as i32,
                recording_state: RecordingState::Initial as i32,
                realtime_mode: true,
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
        self.experiment.last_image = Some(image.clone());
        if !self.experiment.track_file.is_empty() {
            if let Some(features) = self.track.features.get(&image.frame_number) {
                self.experiment.last_features = Some(features.clone());
            }
        }
    }

    pub fn handle_tracking_result(&mut self, result: TrackingResult) {
        let TrackingResult {
            frame_number,
            mut features,
            skeleton,
        } = result;
        self.experiment.skeleton = Some(skeleton.clone());
        metrics::counter!("count.detected_features", features.features.len() as u64);
        // Adjust the track frame numbers to start at 0
        let recording_frame_number = frame_number - self.recording_start_frame;
        features.frame_number = recording_frame_number;
        self.track
            .features
            .insert(recording_frame_number, features.clone());
        self.experiment.last_features = Some(features.clone());
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
        self.experiment.last_image = None;
        self.experiment.last_features = None;
        self.video_decoder = Some(Arc::new(Mutex::new(decoder)));
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
        let file = std::fs::File::open(path.clone())?;
        let reader = std::io::BufReader::new(file);
        let track: Track = serde_json::from_reader(reader)?;
        self.experiment.skeleton = track.skeleton.clone();
        self.experiment.track_file = path;
        let entities = track
            .features
            .values()
            .flat_map(|f| f.features.iter())
            .filter(|f| f.id.is_some())
            .map(|f| f.id.unwrap())
            .collect::<std::collections::HashSet<_>>();
        self.experiment.entity_ids = entities.into_iter().collect();
        self.track = track;
        Ok(())
    }

    pub fn save_track(&mut self, path: &str) -> Result<()> {
        self.track.skeleton = self.experiment.skeleton.clone();
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

    pub async fn switch_entities(&mut self, request: EntityIdSwitch) -> Result<()> {
        if let Some(features) = self.experiment.last_features.as_mut() {
            features.switch_ids(&request);
        }
        if !self.experiment.track_file.is_empty() {
            // If we are in replay mode, we immediately apply the id switch for all future features
            // in the track, then return
            let last_features_frame = match self.experiment.last_features {
                Some(ref features) => features.frame_number,
                None => 0,
            };
            self.track
                .features
                .iter_mut()
                .for_each(|(frame_number, features)| {
                    if *frame_number >= last_features_frame {
                        features.switch_ids(&request);
                    }
                });
            return Ok(());
        }
        // switch IDs for future frames in the matcher
        if let Some(matcher) = self.connections.matcher().as_mut() {
            return match matcher.switch_ids(request).await {
                Ok(_) => Ok(()),
                Err(e) => Err(anyhow::anyhow!("Matcher failed to switch ids: {}", e)),
            };
        }
        Ok(())
    }

    pub fn start_recording(&mut self) -> Result<()> {
        self.recording_start_frame = match &self.experiment.last_image {
            Some(image) => image.frame_number,
            None => 0,
        };
        self.track = Track::default();
        Ok(())
    }

    pub fn set_recording_state(&mut self, recording_state: i32) -> Result<()> {
        match RecordingState::from_i32(recording_state) {
            Some(RecordingState::Recording) => {
                self.start_recording()?;
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
