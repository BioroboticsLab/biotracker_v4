use super::{protocol::*, BiotrackerConfig, VideoDecoder, VideoEncoder};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct State {
    pub config: BiotrackerConfig,
    pub experiment: Experiment,
    pub feature_detector: Option<FeatureDetectorClient<tonic::transport::Channel>>,
    pub matcher: Option<MatcherClient<tonic::transport::Channel>>,
    pub track_recorder: Option<TrackRecorderClient<tonic::transport::Channel>>,
    pub tracks: HashMap<u32, Track>,
    pub video_decoder: Option<Arc<Mutex<VideoDecoder>>>,
    pub video_encoder: Option<Arc<Mutex<VideoEncoder>>>,
    entity_counter: u32,
}

impl State {
    pub fn new(config: BiotrackerConfig) -> Self {
        Self {
            experiment: Experiment {
                target_fps: 25.0,
                arena: Some(config.arena.clone()),
                playback_state: PlaybackState::Paused as i32,
                recording_state: RecordingState::Initial as i32,
                realtime_mode: true,
                last_entities: Some(Entities { entities: vec![] }),
                ..Default::default()
            },
            config,
            ..Default::default()
        }
    }

    pub fn handle_tracking_result(&mut self, features: Features, entities: Entities) {
        let skeleton = features.skeleton.clone();
        for entity in &entities.entities {
            if !self.tracks.contains_key(&entity.id) {
                self.tracks.insert(
                    entity.id,
                    Track {
                        skeleton: skeleton.clone(),
                        observations: Vec::new(),
                    },
                );
            }
            let track = self.tracks.get_mut(&entity.id).expect("track not found");
            if let Some(last_observation) = track.observations.last() {
                let last_seen = last_observation.frame_number;
                assert!(last_seen <= entity.frame_number);
                if last_seen == entity.frame_number {
                    continue;
                }
            }
            track.observations.push(entity.clone());
        }
        self.experiment.last_features = Some(features);
        self.experiment.last_entities = Some(entities);
    }

    pub fn open_video(&mut self, path: String) -> Result<VideoInfo> {
        let decoder = VideoDecoder::new(path)?;
        let video_info = decoder.info.clone();
        let result = Ok(video_info.clone());
        self.experiment.video_info = Some(video_info);
        self.video_decoder = Some(Arc::new(Mutex::new(decoder)));
        self.experiment.playback_state = PlaybackState::Playing as i32;
        result
    }

    pub fn close_decoder(&mut self) {
        self.video_decoder = None;
        self.experiment.video_info = None;
        self.experiment.playback_state = PlaybackState::Eos as i32;
        self.experiment.recording_state = RecordingState::Initial as i32;
    }

    pub fn add_entity(&mut self) -> Result<()> {
        self.experiment
            .last_entities
            .as_mut()
            .expect("no entities found")
            .entities
            .push(Entity {
                id: self.entity_counter,
                frame_number: 0,
                feature: None,
            });
        self.entity_counter += 1;
        Ok(())
    }

    pub fn set_recording_state(&mut self, recording_state: i32) -> Result<()> {
        match RecordingState::from_i32(recording_state) {
            Some(RecordingState::Recording) => {
                self.tracks.clear();
            }
            Some(RecordingState::Finished) => {
                self.video_encoder = None;
                self.experiment.video_encoder_config = None;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid recording state {:?}",
                    recording_state
                ));
            }
        };
        self.experiment.recording_state = recording_state;
        Ok(())
    }

    pub fn initialize_video_encoder(&mut self, config: VideoEncoderConfig) -> Result<()> {
        let encoder = VideoEncoder::new(config.clone())?;
        self.experiment.video_encoder_config = Some(config);
        self.video_encoder = Some(Arc::new(Mutex::new(encoder)));
        Ok(())
    }

    pub fn remove_entity(&mut self) -> Result<()> {
        self.experiment
            .last_entities
            .as_mut()
            .expect("no entities found")
            .entities
            .pop();
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
}
