use anyhow::{anyhow, Result};
use cv::prelude::*;
use cv::videoio::VideoWriter;
use libtracker::{message_bus::Client, Component, Message, SharedBuffer};

#[derive(Debug)]
pub struct VideoEncoderSettings {
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub path: String,
}

pub struct VideoEncode {
    writer: VideoWriter,
    frame_number: u64,
    _settings: VideoEncoderSettings,
}

pub struct VideoEncoder {
    msg_bus: Client,
    encode: Option<VideoEncode>,
}

impl Component for VideoEncoder {
    /// Get images from the message bus and encode them into a video.
    fn run(&mut self) -> Result<()> {
        self.msg_bus.subscribe("Image")?;
        self.msg_bus.subscribe("Shutdown")?;
        loop {
            match self.msg_bus.poll(-1)?.unwrap() {
                Message::Image(img) => {
                    if let Some(encode) = &mut self.encode {
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
                        cv::imgproc::cvt_color(
                            &cv_img,
                            &mut img_bgr,
                            cv::imgproc::COLOR_RGBA2BGR,
                            0,
                        )?;
                        encode.writer.write(&img_bgr)?;
                        encode.frame_number += 1;
                        if encode.frame_number == 10 {
                            self.finish()?;
                            break;
                        }
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
    pub fn new(msg_bus: Client) -> Self {
        Self {
            msg_bus,
            encode: None,
        }
    }

    /// Initialize a VideoWriter and start encoding.
    pub fn start(&mut self, settings: VideoEncoderSettings) -> Result<()> {
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
            _settings: settings,
        });
        Ok(())
    }

    pub fn finish(&mut self) -> Result<()> {
        assert!(self.encode.is_some());
        let _ = self.encode.take();
        Ok(())
    }
}
