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
        })
    }

    pub fn add_image(&mut self, image: Image) -> Result<()> {
        let image_buffer = SharedBuffer::open(&image.shm_id)?;
        let cv_image = unsafe {
            let data = image_buffer.as_slice();
            let cv_image = Mat::new_nd_with_data(
                &[image.width as i32, image.height as i32],
                cv::core::CV_8UC4,
                data.as_ptr() as *mut std::ffi::c_void,
                None,
            )?;
            cv_image
        };
        let cv_image = if image.width != self.config.width || image.height != self.config.height {
            let mut resized = Mat::default();
            cv::imgproc::resize(
                &cv_image,
                &mut resized,
                cv::core::Size::new(self.config.width as i32, self.config.height as i32),
                0.0,
                0.0,
                cv::imgproc::InterpolationFlags::INTER_LINEAR as i32,
            )?;
            resized
        } else {
            cv_image
        };
        let mut image_bgr = Mat::default();
        cv::imgproc::cvt_color(&cv_image, &mut image_bgr, cv::imgproc::COLOR_RGBA2BGR, 0)?;
        self.video_writer.write(&image_bgr)?;
        Ok(())
    }
}
