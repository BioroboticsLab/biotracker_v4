use anyhow::Result;
use cv::prelude::*;
use cv::videoio::VideoWriter;
use libtracker::{protocol::*, Client, Component, SharedBuffer};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VideoEncoderConfig {
    pub video_path: String,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub image_stream_id: String,
}

pub struct VideoEncoder {
    msg_bus: Client,
    recording_state: RecordingState,
    video_writer: Option<VideoWriter>,
}

impl Component for VideoEncoder {
    fn run(&mut self) -> Result<()> {
        self.msg_bus
            .subscribe(&[Topic::ExperimentState, Topic::Shutdown])?;
        loop {
            while let Some(message) = self.msg_bus.poll(-1)? {
                match message {
                    Message::ComponentMessage(msg) => match msg.content {
                        Some(component_message::Content::ConfigJson(config)) => {
                            self.initialize(serde_json::from_str(&config)?)?;
                        }
                        _ => {}
                    },
                    Message::ExperimentState(state) => {
                        self.recording_state =
                            RecordingState::from_i32(state.recording_state).unwrap();
                        if self.recording_state == RecordingState::Finished {
                            self.video_writer = None;
                        }
                    }
                    Message::Image(image) => {
                        eprintln!("Received image");
                        if self.recording_state == RecordingState::Recording {
                            if let Some(writer) = &mut self.video_writer {
                                eprintln!("Recording image");
                                VideoEncoder::record_frame(image, writer)?;
                            }
                        }
                    }
                    Message::Shutdown(_) => {
                        self.video_writer = None;
                        return Ok(());
                    }
                    _ => panic!("Encoder: Unexpected message: {:?}", message),
                }
            }
        }
    }
}

impl VideoEncoder {
    pub fn new(msg_bus: Client) -> Self {
        msg_bus
            .register_component(libtracker::protocol::Component {
                id: "VideoEncoder".to_owned(),
                typ: ComponentType::Recorder as i32,
            })
            .unwrap();
        Self {
            msg_bus,
            video_writer: None,
            recording_state: RecordingState::Initial,
        }
    }

    fn record_frame(image: Image, writer: &mut VideoWriter) -> Result<()> {
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
        let mut image_bgr = Mat::default();
        cv::imgproc::cvt_color(&cv_image, &mut image_bgr, cv::imgproc::COLOR_RGBA2BGR, 0)?;
        writer.write(&image_bgr)?;
        Ok(())
    }

    fn initialize(&mut self, config: VideoEncoderConfig) -> Result<()> {
        let writer = VideoWriter::new(
            &config.video_path,
            cv::videoio::VideoWriter::fourcc('m', 'p', '4', 'v')?,
            config.fps,
            cv::core::Size::new(config.width as i32, config.height as i32),
            true, // is_color
        )?;
        if !writer.is_opened()? {
            eprintln!("Failed to open video writer with settings: {:?}", config);
            return Ok(());
        }
        self.video_writer = Some(writer);
        self.msg_bus.subscribe_image(&config.image_stream_id)?;
        Ok(())
    }
}
