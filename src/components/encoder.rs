use anyhow::{anyhow, Result};
use cv::prelude::*;
use cv::videoio::VideoWriter;
use libtracker::{message_bus::Client, protocol::*, CommandLineArguments, Component, SharedBuffer};
use std::sync::Arc;

pub struct VideoEncode {
    writer: VideoWriter,
    frame_number: u64,
    state: VideoEncoderState,
}

pub struct VideoEncoder {
    msg_bus: Client,
    encode: Option<VideoEncode>,
}

impl Component for VideoEncoder {
    fn new(msg_bus: Client, _args: Arc<CommandLineArguments>) -> Self {
        Self {
            msg_bus,
            encode: None,
        }
    }

    /// Get images from the message bus and encode them into a video.
    fn run(&mut self) -> Result<()> {
        self.msg_bus.subscribe(&[
            MessageType::Image,
            MessageType::Shutdown,
            MessageType::VideoEncoderCommand,
        ])?;
        while let Some(message) = self.msg_bus.poll(-1)? {
            match message {
                Message::Image(img) => self.process_image(img)?,
                Message::VideoEncoderCommand(cmd) => {
                    if let Some(settings) = cmd.settings {
                        self.initialize(settings)?;
                        self.send_state_update()?;
                    }
                    if let Some(state) = cmd.state {
                        match VideoState::from_i32(state).unwrap() {
                            VideoState::Playing | VideoState::Paused => {
                                if let Some(encode) = &mut self.encode {
                                    encode.state.state = state;
                                    self.send_state_update()?;
                                }
                            }
                            VideoState::Eos | VideoState::Stopped => self.finish()?,
                        }
                    }
                }
                Message::Shutdown => {
                    self.finish()?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl VideoEncoder {
    pub fn send_state_update(&self) -> Result<()> {
        if let Some(encode) = &self.encode {
            self.msg_bus
                .send(Message::VideoEncoderState(encode.state.clone()))?;
        }
        Ok(())
    }

    /// Initialize a VideoWriter and start encoding.
    pub fn initialize(&mut self, settings: VideoEncoderState) -> Result<()> {
        assert!(self.encode.is_none());
        let writer = VideoWriter::new(
            &settings.path,
            cv::videoio::VideoWriter::fourcc('m', 'p', '4', 'v')?,
            settings.fps,
            cv::core::Size::new(2048, 2048),
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
            state: settings,
        });
        Ok(())
    }

    pub fn finish(&mut self) -> Result<()> {
        if let Some(encode) = &mut self.encode {
            encode.state.state = VideoState::Eos.into();
            self.send_state_update()?;
        }
        _ = self.encode.take();
        Ok(())
    }

    pub fn process_image(&mut self, img: Image) -> Result<()> {
        if img.stream_id != "Annotated" {
            return Ok(());
        }
        if let Some(encode) = &mut self.encode {
            if VideoState::from_i32(encode.state.state).unwrap() != VideoState::Playing {
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
