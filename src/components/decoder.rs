use anyhow::Result;
use cv::prelude::*;
use cv::videoio::VideoCapture;
use libtracker::{
    message_bus::Client, protocol::*, CommandLineArguments, Component, DoubleBuffer, SharedBuffer,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Playback {
    state: VideoDecoderState,
    sampler: Box<dyn VideoSampler>,
}

pub struct VideoDecoder {
    msg_bus: Client,
    buffer_manager: DoubleBuffer,
    last_frame_msg_sent: Instant,
    last_timestamp: u64,
    playback: Option<Playback>,
}

trait VideoSampler {
    fn get_frame(&mut self, timestamp: u64) -> Result<(SharedBuffer, Image)>;
    fn seek(&mut self, _target_framenumber: u32) -> Result<()> {
        Err(anyhow::anyhow!("Seek not supported"))
    }
}

impl Playback {
    fn open_path(path: &str) -> Result<Playback> {
        let mut video_capture = VideoCapture::from_file(path, 0)?;
        let frame_number = video_capture.get(cv::videoio::CAP_PROP_POS_FRAMES)? as u32;
        let frame_count = video_capture.get(cv::videoio::CAP_PROP_FRAME_COUNT)? as u32;
        let width = video_capture.get(cv::videoio::CAP_PROP_FRAME_WIDTH)? as u32;
        let height = video_capture.get(cv::videoio::CAP_PROP_FRAME_HEIGHT)? as u32;
        let mut fps = video_capture.get(cv::videoio::CAP_PROP_FPS)?;
        if fps == 0.0 {
            fps = 30.0;
            video_capture.set(cv::videoio::CAP_PROP_FPS, 30.0)?;
        }
        Ok(Playback {
            state: VideoDecoderState {
                path: path.to_string(),
                frame_number,
                frame_count,
                fps,
                width,
                height,
                state: VideoState::Playing.into(),
            },
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
    fn new(msg_bus: Client, _args: Arc<CommandLineArguments>) -> Self {
        let buffer_manager = DoubleBuffer::new();
        Self {
            msg_bus,
            buffer_manager,
            last_frame_msg_sent: Instant::now(),
            last_timestamp: 0,
            playback: None,
        }
    }

    fn run(&mut self) -> Result<()> {
        self.msg_bus
            .subscribe(&[MessageType::VideoDecoderCommand, MessageType::Features])?;
        loop {
            while let Some(message) = self.msg_bus.poll(-1)? {
                match message {
                    Message::VideoDecoderCommand(cmd) => {
                        self.handle_command(cmd)?;
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
    fn handle_command(&mut self, cmd: VideoDecoderCommand) -> Result<()> {
        if let Some(path) = cmd.path {
            self.playback = Some(Playback::open_path(&path)?);
        }
        if let Some(playback) = &mut self.playback {
            if let Some(state) = cmd.state {
                playback.state.state = state;
            }
            if let Some(fps) = cmd.fps {
                playback.state.fps = fps;
            }
            if let Some(frame_number) = cmd.frame_number {
                playback.sampler.seek(frame_number)?;
                playback.state.frame_number = frame_number;
            }
            self.send_state_update()?;
        }
        Ok(())
    }

    fn send_state_update(&self) -> Result<()> {
        if let Some(Playback { state, sampler: _ }) = &self.playback {
            self.msg_bus
                .send(Message::VideoDecoderState(state.clone()))?;
        }
        Ok(())
    }

    fn next_frame(&mut self) -> Result<()> {
        loop {
            if let Some(Playback { state, sampler }) = &mut self.playback {
                if VideoState::from_i32(state.state) == Some(VideoState::Playing) {
                    let next_msg_deadline =
                        self.last_frame_msg_sent + Duration::from_secs_f64(1.0 / state.fps);
                    self.last_frame_msg_sent = next_msg_deadline;
                    let now = Instant::now();
                    if now < next_msg_deadline {
                        std::thread::sleep(next_msg_deadline - now);
                    }
                    let timestamp = ((state.frame_number as f64 / state.fps) * 1e9) as u64;
                    let (image_buffer, image) = sampler.get_frame(timestamp)?;
                    self.buffer_manager.push(image_buffer);
                    self.last_timestamp = image.timestamp;
                    self.msg_bus.send(Message::Image(image))?;
                    state.frame_number += 1;
                    if state.frame_count > 0 && state.frame_number >= state.frame_count {
                        state.state = VideoState::Eos.into();
                        self.send_state_update()?;
                    }
                }
            }
            return Ok(());
        }
    }
}
