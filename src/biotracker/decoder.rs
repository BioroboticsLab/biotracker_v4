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
    fn open(uri: String, fps: f64) -> Result<(Playback, VideoInfo)> {
        if uri.starts_with("pylon:///") {
            Playback::open_basler(uri, fps)
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
    fn open_basler(_camera_id: String, _: f64) -> Result<(Playback, VideoInfo)> {
        panic!("Pylon feature disabled");
    }

    #[cfg(feature = "pylon")]
    fn open_basler(camera_id: String, fps: f64) -> Result<(Playback, VideoInfo)> {
        let pylon = Box::pin(pylon_cxx::Pylon::new());
        // Safety:
        // - pylon is pinned
        // - pylon_camera.pylon is never modified
        // - pylon_camera.pylon outlives pylon_camera.camera
        let tlfactory = unsafe {
            let pylon_unchecked_ref = (&*pylon as *const pylon_cxx::Pylon).as_ref().unwrap();
            pylon_cxx::TlFactory::instance(pylon_unchecked_ref)
        };

        let device_id = camera_id.strip_prefix("pylon:///").unwrap();
        let devices = tlfactory.enumerate_devices()?;
        let device_info = devices
            .iter()
            .find(|d| d.model_name().unwrap() == device_id)
            .unwrap();
        let camera = tlfactory.create_device(device_info)?;
        camera.open()?;
        camera
            .node_map()
            .enum_node("PixelFormat")?
            .set_value("Mono8")?;
        camera.start_grabbing(&pylon_cxx::GrabOptions::default())?;
        let frame_number = 0;
        let frame_count = 0;
        let width = camera.node_map().integer_node("Width")?.value()? as u32;
        let height = camera.node_map().integer_node("Height")?.value()? as u32;
        camera
            .node_map()
            .float_node("AcquisitionFrameRate")?
            .set_value(fps)?;
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
    fn get_image(&mut self, mat: &mut Mat) -> Result<()> {
        for i in 0..100 {
            self.camera.retrieve_result(
                1000,
                &mut self.grab_result,
                pylon_cxx::TimeoutHandling::Return,
            )?;

            if !self.grab_result.grab_succeeded()? {
                continue;
            }

            if i > 0 {
                log::warn!("Warning: grabbing image took {} retries", i);
            }

            let pylon_buffer = self.grab_result.buffer()?;
            let width = self.grab_result.width()?;
            let height = self.grab_result.height()?;
            unsafe {
                let data_ptr = pylon_buffer.as_ptr();
                let src_mat = Mat::new_size_with_data(
                    cv::core::Size::new(width as i32, height as i32),
                    cv::core::CV_8UC1,
                    data_ptr as *mut std::ffi::c_void,
                    cv::core::Mat_AUTO_STEP,
                )?;
                assert!(mat.size()? == src_mat.size()?);
                cv::imgproc::cvt_color(&src_mat, mat, cv::imgproc::COLOR_GRAY2BGR, 0)?;
            }
            return Ok(());
        }
        Err(anyhow::anyhow!("Failed to retrieve frame"))
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
    pub fn new(path: String, fps: f64) -> Result<Self> {
        let (playback, info) = Playback::open(path, fps)?;
        Ok(Self {
            info,
            playback,
            buffer_manager: DoubleBuffer::new(),
        })
    }

    pub fn get_image(&mut self) -> Result<Image> {
        let frame_number = self.playback.frame_number;
        let shared_image = self
            .buffer_manager
            .get_mut(self.info.width, self.info.height, 3)?;
        self.playback.sampler.get_image(&mut shared_image.mat)?;
        let image = Image {
            stream_id: "Tracking".to_owned(),
            frame_number,
            shm_id: shared_image.id().to_owned(),
            width: self.info.width,
            height: self.info.height,
            channels: 3,
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
