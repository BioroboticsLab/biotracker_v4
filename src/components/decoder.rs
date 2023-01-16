use anyhow::Result;
use cv::prelude::*;
use cv::videoio::VideoCapture;
use libtracker::{protocol::*, DoubleBuffer, SharedBuffer};
use std::time::{Duration, Instant};

struct Playback {
    frame_number: u32,
    sampler: Box<dyn VideoSampler>,
}

pub struct VideoDecoder {
    pub info: VideoInfo,
    buffer_manager: DoubleBuffer,
    last_frame_msg_sent: Instant,
    last_timestamp: u64,
    playback: Playback,
}

trait VideoSampler {
    fn get_frame(&mut self, timestamp: u64) -> Result<(SharedBuffer, Image)>;
    fn seek(&mut self, _target_framenumber: u32) -> Result<()> {
        Err(anyhow::anyhow!("Seek not supported"))
    }
}

impl Playback {
    fn open_path(path: String) -> Result<(Playback, VideoInfo)> {
        let video_capture = VideoCapture::from_file(&path, 0)?;
        let frame_number = video_capture.get(cv::videoio::CAP_PROP_POS_FRAMES)? as u32;
        let frame_count = video_capture.get(cv::videoio::CAP_PROP_FRAME_COUNT)? as u32;
        let width = video_capture.get(cv::videoio::CAP_PROP_FRAME_WIDTH)? as u32;
        let height = video_capture.get(cv::videoio::CAP_PROP_FRAME_HEIGHT)? as u32;
        let fps = video_capture.get(cv::videoio::CAP_PROP_FPS)?;

        Ok((
            Playback {
                frame_number,
                sampler: Box::new(video_capture),
            },
            VideoInfo {
                path,
                frame_count,
                width,
                height,
                fps,
            },
        ))
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

impl VideoDecoder {
    pub fn new(path: String) -> Result<Self> {
        let buffer_manager = DoubleBuffer::new();
        let (playback, info) = Playback::open_path(path)?;
        Ok(Self {
            buffer_manager,
            last_frame_msg_sent: Instant::now(),
            last_timestamp: 0,
            playback,
            info,
        })
    }

    pub fn next_frame(&mut self, target_fps: f64) -> Result<Option<Image>> {
        let next_msg_deadline =
            self.last_frame_msg_sent + Duration::from_secs_f64(1.0 / target_fps);
        self.last_frame_msg_sent = next_msg_deadline;
        let now = Instant::now();
        if now < next_msg_deadline {
            std::thread::sleep(next_msg_deadline - now);
        }
        let timestamp = ((self.playback.frame_number as f64 / target_fps) * 1e9) as u64;
        let (image_buffer, image) = self.playback.sampler.get_frame(timestamp)?;
        self.buffer_manager.push(image_buffer);
        self.last_timestamp = image.timestamp;
        self.playback.frame_number += 1;
        return Ok(Some(image));
    }

    pub fn seek(&mut self, target_framenumber: u32) -> Result<()> {
        self.playback.sampler.seek(target_framenumber)?;
        self.playback.frame_number = target_framenumber;
        Ok(())
    }
}
