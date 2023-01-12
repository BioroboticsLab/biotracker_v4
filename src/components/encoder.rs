use anyhow::{anyhow, Result};
use cv::prelude::*;
use cv::videoio::VideoWriter;
use libtracker::{protocol::*, Client, CommandLineArguments, Component, SharedBuffer};
use std::sync::Arc;

pub struct VideoEncode {
    writer: VideoWriter,
    frame_number: u64,
}

pub struct VideoEncoder {
    msg_bus: Client,
    encode: Option<VideoEncode>,
    recording_state: RecordingState,
}

impl Component for VideoEncoder {
    /// Get images from the message bus and encode them into a video.
    fn run(&mut self) -> Result<()> {
        self.msg_bus.subscribe(&[
            MessageType::Image,
            MessageType::Shutdown,
            MessageType::ExperimentState,
        ])?;
        while let Some(message) = self.msg_bus.poll(-1)? {
            match message {
                Message::Image(img) => self.process_image(img)?,
                Message::ExperimentState(experiment) => {
                    if let Some(settings) = experiment.video_encoder_state {
                        self.initialize(settings)?;
                    }
                    self.recording_state =
                        RecordingState::from_i32(experiment.recording_state).unwrap();
                    if self.recording_state == RecordingState::Finished {
                        self.finish()?;
                        break;
                    }
                }
                Message::Shutdown => {
                    self.finish()?;
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl VideoEncoder {
    pub fn new(msg_bus: Client, _args: Arc<CommandLineArguments>) -> Self {
        Self {
            msg_bus,
            encode: None,
            recording_state: Default::default(),
        }
    }

    /// Initialize a VideoWriter and start encoding.
    pub fn initialize(&mut self, settings: VideoEncoderState) -> Result<()> {
        assert!(self.encode.is_none());
        let writer = VideoWriter::new(
            &settings.path,
            cv::videoio::VideoWriter::fourcc('m', 'p', '4', 'v')?,
            settings.fps,
            cv::core::Size::new(settings.width, settings.height),
            true, // is_color
        )?;
        if !writer.is_opened()? {
            return Err(anyhow!(
                "Failed to open video writer with settings: {:?}",
                settings
            ));
        }
        self.encode = Some(VideoEncode {
            writer,
            frame_number: 0,
        });
        Ok(())
    }

    pub fn finish(&mut self) -> Result<()> {
        _ = self.encode.take();
        Ok(())
    }

    pub fn process_image(&mut self, img: Image) -> Result<()> {
        if img.stream_id != "Annotated" {
            return Ok(());
        }
        if let Some(encode) = &mut self.encode {
            if self.recording_state != RecordingState::Recording {
                return Ok(());
            }
            let image_buffer = SharedBuffer::open(&img.shm_id)?;
            let cv_img = unsafe {
                let data = image_buffer.as_slice();
                let cv_img = Mat::new_nd_with_data(
                    &[img.width as i32, img.height as i32],
                    cv::core::CV_8UC4,
                    data.as_ptr() as *mut std::ffi::c_void,
                    None,
                )?;
                cv_img
            };
            let mut img_bgr = Mat::default();
            cv::imgproc::cvt_color(&cv_img, &mut img_bgr, cv::imgproc::COLOR_RGBA2BGR, 0)?;
            encode.writer.write(&img_bgr)?;
            encode.frame_number += 1;
        }
        Ok(())
    }
}
