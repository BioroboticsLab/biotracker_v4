use anyhow::Result;
use cv::prelude::*;
use cv::videoio::VideoCapture;
use libtracker::{
    protocol::*, Client, CommandLineArguments, Component, DoubleBuffer, SharedBuffer,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Playback {
    width: u32,
    height: u32,
    frame_count: u32,
    frame_number: u32,
    fps: f64,
    sampler: Box<dyn VideoSampler>,
}

pub struct VideoDecoder {
    msg_bus: Client,
    buffer_manager: DoubleBuffer,
    last_frame_msg_sent: Instant,
    last_timestamp: u64,
    experiment: ExperimentState,
    playback: Option<Playback>,
}

trait VideoSampler {
    fn get_frame(&mut self, timestamp: u64) -> Result<(SharedBuffer, Image)>;
    fn seek(&mut self, _target_framenumber: u32) -> Result<()> {
        Err(anyhow::anyhow!("Seek not supported"))
    }
}

impl Playback {
    fn open_path(settings: &VideoDecoderState, fps: f64) -> Result<Playback> {
        let mut video_capture = VideoCapture::from_file(&settings.path, 0)?;
        let frame_number = video_capture.get(cv::videoio::CAP_PROP_POS_FRAMES)? as u32;
        let frame_count = video_capture.get(cv::videoio::CAP_PROP_FRAME_COUNT)? as u32;
        let width = video_capture.get(cv::videoio::CAP_PROP_FRAME_WIDTH)? as u32;
        let height = video_capture.get(cv::videoio::CAP_PROP_FRAME_HEIGHT)? as u32;
        let mut cv_fps = video_capture.get(cv::videoio::CAP_PROP_FPS)?;
        if cv_fps == 0.0 && fps > 0.0 {
            video_capture.set(cv::videoio::CAP_PROP_FPS, fps)?;
            cv_fps = fps;
        }
        Ok(Playback {
            width,
            height,
            frame_count,
            frame_number,
            fps: cv_fps,
            sampler: Box::new(video_capture),
        })
    }
}

impl VideoSampler for VideoCapture {
    fn get_frame(&mut self, timestamp: u64) -> Result<(SharedBuffer, Image)> {
        let mut img = Mat::default();
        self.read(&mut img)?;
        let mut img_rgba = Mat::default();
        cv::imgproc::cvt_color(&img, &mut img_rgba, cv::imgproc::COLOR_BGR2RGBA, 0)?;
        let data = img_rgba.data_bytes()?;
        let height = img_rgba.rows() as u32;
        let width = img_rgba.cols() as u32;
        let mut image_buffer = SharedBuffer::new(data.len())?;
        unsafe {
            image_buffer.as_slice_mut().clone_from_slice(data);
        }
        let shm_id = image_buffer.id().to_owned();
        let image = Image {
            stream_id: "Tracking".to_owned(),
            timestamp,
            shm_id,
            width,
            height,
        };
        Ok((image_buffer, image))
    }

    fn seek(&mut self, target_framenumber: u32) -> Result<()> {
        self.set(cv::videoio::CAP_PROP_POS_FRAMES, target_framenumber as f64)?;
        Ok(())
    }
}

impl Component for VideoDecoder {
    fn run(&mut self) -> Result<()> {
        self.msg_bus
            .subscribe(&[MessageType::ExperimentState, MessageType::Features])?;
        loop {
            while let Some(message) = self.msg_bus.poll(-1)? {
                match message {
                    Message::ExperimentState(experiment) => {
                        self.update_experiment(experiment)?;
                        break;
                    }
                    Message::Features(features_msg) => {
                        if features_msg.timestamp >= self.last_timestamp {
                            break;
                        }
                    }
                    _ => eprintln!("Unexpected message {:?}", message),
                }
            }
            self.next_frame()?;
        }
    }
}

impl VideoDecoder {
    pub fn new(msg_bus: Client, _args: Arc<CommandLineArguments>) -> Self {
        let buffer_manager = DoubleBuffer::new();
        Self {
            msg_bus,
            buffer_manager,
            last_frame_msg_sent: Instant::now(),
            last_timestamp: 0,
            experiment: ExperimentState::default(),
            playback: None,
        }
    }

    fn update_experiment(&mut self, experiment: ExperimentState) -> Result<()> {
        if let Some(decoder_state) = &experiment.video_decoder_state {
            if self.playback.is_none() {
                let playback = Playback::open_path(decoder_state, experiment.target_fps)?;
                let frame_count = match playback.frame_count {
                    0 => None,
                    n => Some(n),
                };
                self.msg_bus
                    .send(Message::ExperimentUpdate(ExperimentUpdate {
                        frame_count,
                        video_decoder_state: Some(VideoDecoderState {
                            path: decoder_state.path.clone(),
                            width: playback.width,
                            height: playback.height,
                            fps: playback.fps,
                        }),
                        ..Default::default()
                    }))?;
                self.playback = Some(playback);
            }
        }
        self.experiment = experiment;
        Ok(())
    }

    fn next_frame(&mut self) -> Result<()> {
        loop {
            if let Some(Playback {
                width: _,
                height: _,
                frame_count: _,
                frame_number,
                fps: _,
                sampler,
            }) = &mut self.playback
            {
                if PlaybackState::from_i32(self.experiment.playback_state)
                    == Some(PlaybackState::Playing)
                    && self.experiment.target_fps > 0.0
                {
                    let next_msg_deadline = self.last_frame_msg_sent
                        + Duration::from_secs_f64(1.0 / self.experiment.target_fps);
                    self.last_frame_msg_sent = next_msg_deadline;
                    let now = Instant::now();
                    if now < next_msg_deadline {
                        std::thread::sleep(next_msg_deadline - now);
                    }
                    let timestamp =
                        ((*frame_number as f64 / self.experiment.target_fps) * 1e9) as u64;
                    let (image_buffer, image) = sampler.get_frame(timestamp)?;
                    self.buffer_manager.push(image_buffer);
                    self.last_timestamp = image.timestamp;
                    self.msg_bus.send(Message::Image(image))?;
                    *frame_number += 1;
                }
            }
            return Ok(());
        }
    }
}
