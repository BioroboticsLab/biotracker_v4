use anyhow::{anyhow, Result};
use cv::prelude::*;
use cv::videoio::VideoCapture;
use libtracker::{
    message_bus::Client, BufferHistory, Component, ImageData, Message, Seekable, SharedBuffer,
    State, Timestamp,
};
use std::time::{Duration, Instant};

pub struct Video {
    video_capture: VideoCapture,
    frame_number: u64,
    frame_count: u64,
    fps: f64,
    frame_duration: Duration,
}

pub struct Sampler {
    msg_bus: Client,
    buffer_manager: BufferHistory,
    play_state: State,
    playback: Option<Video>,
}

impl Component for Sampler {
    fn run(&mut self) -> Result<()> {
        self.msg_bus.subscribe("Command")?;
        self.msg_bus.subscribe("Shutdown")?;
        self.msg_bus.subscribe("Feature")?;

        let mut last_frame_msg_sent = Instant::now();
        let mut last_pts = Timestamp(0);
        let mut img = Mat::default();
        loop {
            if let Some(playback) = &mut self.playback {
                if self.play_state == State::Play {
                    let next_msg_deadline = last_frame_msg_sent + playback.frame_duration;
                    last_frame_msg_sent = next_msg_deadline;
                    let now = Instant::now();
                    if now < next_msg_deadline {
                        std::thread::sleep(next_msg_deadline - now);
                    }
                    if !playback.video_capture.read(&mut img)? {
                        continue;
                    }
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
                    self.buffer_manager.push(image_buffer);
                    let pts = Timestamp::from_framenumber(playback.frame_number, playback.fps);
                    last_pts = pts;
                    self.msg_bus.send(Message::Image(ImageData {
                        pts,
                        shm_id,
                        width,
                        height,
                    }))?;
                    playback.frame_number += 1;
                    if playback.frame_count > 0 && playback.frame_number >= playback.frame_count {
                        self.play_state = State::EoS;
                        self.msg_bus.send(Message::Event(State::EoS))?;
                    }
                }
            }

            loop {
                match self.msg_bus.poll(-1)? {
                    Some(msg) => match msg {
                        Message::Command(cmd) => {
                            self.handle_command(&cmd)?;
                            break;
                        }
                        Message::Shutdown => {
                            self.stop()?;
                            return Ok(());
                        }
                        Message::Features(features_msg) => {
                            if features_msg.pts == last_pts {
                                break;
                            }
                        }
                        _ => panic!("Unexpected message {:?}", msg),
                    },
                    None => {}
                }
            }
        }
    }
}

impl Sampler {
    pub fn new(msg_bus: Client) -> Self {
        let buffer_manager = BufferHistory::new();
        Self {
            msg_bus,
            buffer_manager,
            play_state: State::Stop,
            playback: None,
        }
    }

    fn open(&mut self, path: &str) -> Result<()> {
        let mut video_capture = VideoCapture::from_file(path, 0)?;
        let frame_number = video_capture.get(cv::videoio::CAP_PROP_POS_FRAMES)? as u64;
        let frame_count = video_capture.get(cv::videoio::CAP_PROP_FRAME_COUNT)? as u64;
        let mut fps = video_capture.get(cv::videoio::CAP_PROP_FPS)?;
        if fps == 0.0 {
            fps = 30.0;
            video_capture.set(cv::videoio::CAP_PROP_FPS, 30.0)?;
        }
        let frame_duration = Duration::from_secs_f64(1.0 / fps);
        self.playback = Some(Video {
            video_capture,
            frame_number,
            frame_count,
            fps,
            frame_duration,
        });
        self.msg_bus
            .send(Message::Event(State::Open(path.to_string())))?;
        if frame_count > 0 {
            self.msg_bus.send(Message::Seekable(Seekable {
                start: Timestamp::from_framenumber(frame_number, fps),
                end: Timestamp::from_framenumber(frame_count, fps),
            }))?;
        }
        self.play()?;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.play_state = State::Pause;
        self.msg_bus.send(Message::Event(State::Pause))?;
        Ok(())
    }

    fn play(&mut self) -> Result<()> {
        self.play_state = State::Play;
        self.msg_bus.send(Message::Event(State::Play))?;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.play_state = State::Stop;
        self.msg_bus.send(Message::Event(State::Stop))?;
        Ok(())
    }

    fn seek(&mut self, target: &Timestamp) -> Result<()> {
        if let Some(playback) = &mut self.playback {
            let target_framenumber = target.to_framenumber(playback.fps);
            playback
                .video_capture
                .set(cv::videoio::CAP_PROP_POS_FRAMES, target_framenumber as f64)?;
            playback.frame_number = target_framenumber;
            self.msg_bus.send(Message::Event(State::Seek(*target)))?;
        }
        Ok(())
    }

    fn handle_command(&mut self, cmd: &libtracker::State) -> Result<()> {
        match cmd {
            State::Play => self.play(),
            State::Pause => self.pause(),
            State::Stop => self.stop(),
            State::Seek(timestamp) => self.seek(&timestamp),
            State::Open(path) => self.open(path),
            _ => Err(anyhow!("Unexpected command")),
        }
    }
}
