use super::{protocol::*, DoubleBuffer};
use anyhow::Result;
use cv::prelude::*;
use cv::videoio::VideoWriter;

unsafe impl Send for VideoEncoder {}
unsafe impl Sync for VideoEncoder {}

pub struct VideoEncoder {
    video_writer: VideoWriter,
    config: VideoEncoderConfig,
    image_buffers: DoubleBuffer,
}

impl VideoEncoder {
    pub fn new(config: VideoEncoderConfig) -> Result<Self> {
        let video_writer = VideoWriter::new(
            &config.video_path,
            cv::videoio::VideoWriter::fourcc('m', 'p', '4', 'v')?,
            config.fps,
            cv::core::Size::new(config.width as i32, config.height as i32),
            true, // is_color
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
            image_buffers: DoubleBuffer::new(),
        })
    }

    pub fn add_frame(&mut self, image: Image) -> Result<()> {
        let mat = &self.image_buffers.get(&image)?.mat;
        if mat.cols() == self.config.width as i32 || mat.rows() == self.config.height as i32 {
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
        Ok(())
    }
}
