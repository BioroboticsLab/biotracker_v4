use super::protocol::Image;
use anyhow::Result;
use shared_memory::*;
use std::collections::VecDeque;

unsafe impl Send for SharedImage {}
unsafe impl Sync for SharedImage {}

impl SharedImage {
    pub fn id(&self) -> &str {
        self.shmem.get_os_id()
    }
}

pub struct DoubleBuffer {
    data: VecDeque<SharedImage>,
}

pub struct SharedImage {
    pub mat: cv::prelude::Mat,
    shmem: Shmem,
}

impl DoubleBuffer {
    pub fn new() -> Self {
        Self { data: [].into() }
    }

    pub fn get_mut(&mut self, width: u32, height: u32, channels: u32) -> Result<&mut SharedImage> {
        let mut shared_image = None;
        let len = (width * height * channels) as usize;
        if self.data.len() >= 2 {
            let image = self.data.pop_front().unwrap();
            assert!(image.shmem.is_owner());
            if image.shmem.len() == len {
                shared_image = Some(image);
            }
        }

        if shared_image.is_none() {
            let shmem = ShmemConf::new().size(len).create()?;
            let cv_type = channels_to_cvtype(channels)?;
            let mat = unsafe {
                cv::prelude::Mat::new_rows_cols_with_data(
                    height as i32,
                    width as i32,
                    cv_type,
                    shmem.as_ptr() as *mut _,
                    cv::core::Mat_AUTO_STEP,
                )?
            };
            shared_image = Some(SharedImage { mat, shmem });
        }
        self.data.push_back(shared_image.unwrap());
        Ok(self.data.back_mut().unwrap())
    }

    pub fn get(&mut self, image: &Image) -> Result<&SharedImage> {
        let shared_image = if self.data.len() >= 2 {
            let mut shared_image = self.data.pop_front().unwrap();
            if image.shm_id != shared_image.id() {
                shared_image = SharedImage::try_from(image)?;
            }
            shared_image
        } else {
            let shared_image = SharedImage::try_from(image)?;
            shared_image
        };
        self.data.push_back(shared_image);
        Ok(self.data.back().unwrap())
    }
}

impl TryFrom<&Image> for SharedImage {
    type Error = anyhow::Error;
    fn try_from(image: &Image) -> Result<Self> {
        let shmem = ShmemConf::new().os_id(&image.shm_id).open()?;
        let mat = unsafe {
            cv::prelude::Mat::new_size_with_data(
                cv::core::Size::new(image.width as i32, image.height as i32),
                cv::core::CV_8UC3,
                shmem.as_ptr() as *mut std::ffi::c_void,
                cv::core::Mat_AUTO_STEP,
            )?
        };
        Ok(Self { shmem, mat })
    }
}

fn channels_to_cvtype(channels: u32) -> Result<i32> {
    Ok(match channels {
        1 => cv::core::CV_8UC1,
        2 => cv::core::CV_8UC2,
        3 => cv::core::CV_8UC3,
        4 => cv::core::CV_8UC4,
        _ => Err(anyhow::anyhow!(
            "Unsupported number of channels: {}",
            channels
        ))?,
    })
}
