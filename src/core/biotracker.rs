use anyhow::Result;

use super::{
    message_bus::Client, BufferManager, Message, Sampler, SamplerEvent, Timestamp, VideoSample,
    VideoState,
};

pub struct BioTracker {
    buffer_manager: BufferManager,
    video_sampler: Option<Sampler>,
    msg_bus: Client,
}

impl BioTracker {
    pub fn new() -> Result<Self> {
        Ok(Self {
            buffer_manager: BufferManager::new(),
            video_sampler: None,
            msg_bus: Client::new()?,
        })
    }

    pub fn open_media(&mut self, path: &str) {
        let sampler = Sampler::new(path).expect("Failed to create video sampler");
        sampler.play().unwrap();
        if let Some(old_sampler) = self.video_sampler.take() {
            old_sampler.stop().unwrap();
        }
        self.video_sampler = Some(sampler);
    }

    pub fn run(&mut self) {
        self.msg_bus.subscribe("Command").unwrap();
        self.msg_bus.subscribe("Shutdown").unwrap();
        loop {
            if let Some(sampler) = &mut self.video_sampler {
                if let Ok(sample) = sampler.sample_rx.try_recv() {
                    // FIXME: unwraps
                    let caps = sample.caps().ok_or(gst::FlowError::Error).unwrap();
                    let gst_info = gst_video::VideoInfo::from_caps(&caps).unwrap();
                    let (width, height) = (gst_info.width(), gst_info.height());
                    let buffer_ref = sample.buffer().unwrap();
                    let buffer_map = buffer_ref.map_readable().unwrap();
                    let data_slice = buffer_map.as_slice();
                    let pts = match buffer_ref.pts() {
                        Some(pts) => Some(Timestamp(pts.nseconds())),
                        None => None,
                    };
                    let image_buffer = self.buffer_manager.allocate(data_slice.len()).unwrap();
                    unsafe {
                        image_buffer.as_slice_mut().clone_from_slice(data_slice);
                    }

                    self.msg_bus
                        .send(Message::Sample(VideoSample {
                            id: image_buffer.id().to_owned(),
                            width,
                            height,
                            pts,
                        }))
                        .unwrap();
                }
            }

            if let Some(sampler) = &mut self.video_sampler {
                match sampler.poll_event() {
                    Some(SamplerEvent::Seekable(seekable)) => {
                        self.msg_bus.send(Message::Seekable(seekable)).unwrap();
                    }
                    Some(SamplerEvent::Event(video_state)) => {
                        self.msg_bus.send(Message::Event(video_state)).unwrap();
                    }
                    None => {}
                }
            }

            if let Ok(Some(msg)) = self.msg_bus.poll(0) {
                //eprintln!("Core: {:?}", msg);
                let mut handled = false;
                if let Some(sampler) = &mut self.video_sampler {
                    handled |= sampler.handle_command(&msg).unwrap();
                }

                if !handled {
                    match msg {
                        Message::Command(VideoState::Open(path)) => {
                            self.open_media(&path);
                        }
                        Message::Command(VideoState::Stop) => {
                            if let Some(sampler) = &mut self.video_sampler {
                                sampler.stop().unwrap();
                            }
                        }
                        Message::Shutdown => {
                            if let Some(sampler) = &self.video_sampler {
                                sampler.stop().unwrap();
                            }
                            break;
                        }
                        _ => panic!("Unexpected message"),
                    }
                }
            }
        }
    }
}
