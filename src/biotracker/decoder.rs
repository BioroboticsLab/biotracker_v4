use super::{
    protocol::{Image, VideoInfo},
    DoubleBuffer,
};
use anyhow::Result;
use cv::prelude::*;
use cv::videoio::VideoCapture;

struct Playback {
    frame_number: u32,
    sampler: Box<dyn VideoSampler>,
}

unsafe impl Send for VideoDecoder {}
unsafe impl Sync for VideoDecoder {}
pub struct VideoDecoder {
    pub info: VideoInfo,
    playback: Playback,
    buffer_manager: DoubleBuffer,
}

#[cfg(feature = "pylon")]
struct PylonCamera<'a> {
    camera: pylon_cxx::InstantCamera<'a>,
    grab_result: pylon_cxx::GrabResult,
    // Safety: Camera holds an unchecked reference to _pylon_raii, keep this as the last element,
    // so that it gets dropped last.
    _pylon_raii: std::pin::Pin<Box<pylon_cxx::Pylon>>,
}

trait VideoSampler {
    fn get_image(&mut self, mat: &mut Mat) -> Result<()>;
    fn seek(&mut self, _target_framenumber: u32) -> Result<()> {
        Err(anyhow::anyhow!("Seek not supported"))
    }
}

impl Playback {
    fn open(uri: String) -> Result<(Playback, VideoInfo)> {
        if uri == "pylon" {
            Playback::open_basler(uri)
        } else {
            Playback::open_cv(uri)
        }
    }

    fn open_cv(video_path: String) -> Result<(Playback, VideoInfo)> {
        let video_capture = VideoCapture::from_file(&video_path, 0)?;
        let frame_number = video_capture.get(cv::videoio::CAP_PROP_POS_FRAMES)? as u32;
        let frame_count = video_capture.get(cv::videoio::CAP_PROP_FRAME_COUNT)? as u32;
        let width = video_capture.get(cv::videoio::CAP_PROP_FRAME_WIDTH)? as u32;
        let height = video_capture.get(cv::videoio::CAP_PROP_FRAME_HEIGHT)? as u32;
        let fps = video_capture.get(cv::videoio::CAP_PROP_FPS)?;
        if width == 0 || height == 0 {
            return Err(anyhow::anyhow!("Invalid video size"));
        }

        Ok((
            Playback {
                frame_number,
                sampler: Box::new(video_capture),
            },
            VideoInfo {
                path: video_path,
                frame_count,
                width,
                height,
                fps,
            },
        ))
    }
    #[cfg(not(feature = "pylon"))]
    fn open_basler(_camera_id: String) -> Result<(Playback, VideoInfo)> {
        panic!("Pylon feature disabled");
    }

    #[cfg(feature = "pylon")]
    fn open_basler(camera_id: String) -> Result<(Playback, VideoInfo)> {
        let pylon = Box::pin(pylon_cxx::Pylon::new());
        // Safety:
        // - pylon is pinned
        // - pylon_camera.pylon is never modified
        // - pylon_camera.pylon outlives pylon_camera.camera
        let camera = unsafe {
            let pylon_unchecked_ref = (&*pylon as *const pylon_cxx::Pylon).as_ref().unwrap();
            pylon_cxx::TlFactory::instance(pylon_unchecked_ref).create_first_device()?
        };
        camera.open()?;
        //camera.enum_node("PixelFormat")?.set_value("RGB8")?;
        camera.start_grabbing(&pylon_cxx::GrabOptions::default())?;
        let frame_number = 0;
        let frame_count = 0;
        // FIXME: get from node_map
        let width = 2048;
        let height = 2048;
        let fps = 25.0;
        let pylon_camera = Box::new(PylonCamera {
            _pylon_raii: pylon,
            camera,
            grab_result: pylon_cxx::GrabResult::new()?,
        });
        Ok((
            Playback {
                frame_number,
                sampler: pylon_camera,
            },
            VideoInfo {
                path: camera_id,
                frame_count,
                width,
                height,
                fps,
            },
        ))
    }
}

#[cfg(feature = "pylon")]
impl VideoSampler for PylonCamera<'_> {
    fn get_image(&mut self, frame_number: u32) -> Result<(SharedBuffer, Image)> {
        self.camera.retrieve_result(
            2000,
            &mut self.grab_result,
            pylon_cxx::TimeoutHandling::ThrowException,
        )?;

        if self.grab_result.grab_succeeded()? {
            let pylon_buffer = self.grab_result.buffer()?;
            let width = self.grab_result.width()?;
            let height = self.grab_result.height()?;
            let mut shared_buffer = SharedBuffer::new(pylon_buffer.len())?;
            unsafe {
                shared_buffer.as_slice_mut().clone_from_slice(pylon_buffer);
            }
            let shm_id = shared_buffer.id().to_owned();
            let image = Image {
                stream_id: "Tracking".to_owned(),
                frame_number,
                shm_id,
                width,
                height,
            };
            Ok((shared_buffer, image))
        } else {
            Err(anyhow::anyhow!("PylonCamera: Failed to grab image"))
        }
    }
}

impl VideoSampler for VideoCapture {
    fn get_image(&mut self, mat: &mut Mat) -> Result<()> {
        self.read(mat)?;
        Ok(())
    }

    fn seek(&mut self, target_framenumber: u32) -> Result<()> {
        self.set(cv::videoio::CAP_PROP_POS_FRAMES, target_framenumber as f64)?;
        Ok(())
    }
}

impl VideoDecoder {
    pub fn new(path: String) -> Result<Self> {
        let (playback, info) = Playback::open(path)?;
        Ok(Self {
            info,
            playback,
            buffer_manager: DoubleBuffer::new(),
        })
    }

    pub fn get_image(&mut self) -> Result<Image> {
        let frame_number = self.playback.frame_number;
        let buffer_len = (self.info.width * self.info.height * 3) as usize;
        let buffer = self.buffer_manager.get(buffer_len);
        unsafe {
            let data_ptr = buffer.as_ptr();
            let mut mat = Mat::new_size_with_data(
                cv::core::Size::new(self.info.width as i32, self.info.height as i32),
                cv::core::CV_8UC3,
                data_ptr as *mut std::ffi::c_void,
                cv::core::Mat_AUTO_STEP,
            )?;
            self.playback.sampler.get_image(&mut mat)?;
        }
        let image = Image {
            stream_id: "Tracking".to_owned(),
            frame_number,
            shm_id: buffer.id().to_owned(),
            width: self.info.width,
            height: self.info.height,
        };
        self.playback.frame_number += 1;
        Ok(image)
    }

    pub fn seek(&mut self, target_framenumber: u32) -> Result<()> {
        self.playback.sampler.seek(target_framenumber)?;
        self.playback.frame_number = target_framenumber;
        Ok(())
    }
}
