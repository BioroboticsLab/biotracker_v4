use super::{protocol::*, SharedBuffer};
use anyhow::Result;
use cv::prelude::*;
use cv::videoio::VideoWriter;

unsafe impl Send for VideoEncoder {}
unsafe impl Sync for VideoEncoder {}

pub struct VideoEncoder {
    video_writer: VideoWriter,
    config: VideoEncoderConfig,
}

impl VideoEncoder {
    pub fn new(config: VideoEncoderConfig) -> Result<Self> {
        let video_writer = VideoWriter::new(
            &config.video_path,
            cv::videoio::VideoWriter::fourcc('m', 'p', '4', 'v')?,
            config.fps,
            cv::core::Size::new(config.width as i32, config.height as i32),
            false, // is_color
        )?;
        if !video_writer.is_opened()? {
            return Err(anyhow::anyhow!(
                "Failed to open video writer with settings: {:?}",
                config
            ));
        }
        Ok(Self {
            video_writer,
            config,
        })
    }

    pub fn add_image(&mut self, image: Image) -> Result<()> {
        let buffer = SharedBuffer::open(&image.shm_id)?;
        unsafe {
            let data_ptr = buffer.as_ptr();
            let mat = Mat::new_size_with_data(
                cv::core::Size::new(image.width as i32, image.height as i32),
                cv::core::CV_8UC3,
                data_ptr as *mut std::ffi::c_void,
                cv::core::Mat_AUTO_STEP,
            )?;
            if image.width == self.config.width || image.height == self.config.height {
                self.video_writer.write(&mat)?;
            } else {
                let mut resized_mat = Mat::default();
                cv::imgproc::resize(
                    &mat,
                    &mut resized_mat,
                    cv::core::Size::new(self.config.width as i32, self.config.height as i32),
                    0.0,
                    0.0,
                    cv::imgproc::INTER_LINEAR,
                )?;
                self.video_writer.write(&resized_mat)?;
            }
        }
        Ok(())
    }
}
