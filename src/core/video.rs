use super::{
    message_bus::Client, BufferManager, CommandLineArguments, ImageData, Message, Seekable, State,
    Timestamp,
};
use anyhow::Result;
use derive_more::{Display, Error};
use gst::element_error;
use gst::prelude::*;
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Debug, Display, Error)]
#[display(fmt = "Missing element {}", _0)]
struct MissingElement(#[error(not(source))] &'static str);

#[derive(Debug, Display, Error)]
#[display(fmt = "Received error from {}: {} (debug: {:?})", src, error, debug)]
struct ErrorMessage {
    src: String,
    error: String,
    debug: Option<String>,
    source: glib::Error,
}

struct SampleMessage {
    sample: gst::Sample,
    info: gst_video::VideoInfo,
    pts: Timestamp,
}

pub struct Sampler {
    msg_bus: Client,
    buffer_manager: BufferManager,
    sample_tx: Sender<SampleMessage>,
    sample_rx: Receiver<SampleMessage>,
    appsink: Option<gst_app::AppSink>,
    pipeline: Option<gst::Pipeline>,
    gst_bus: Option<gst::Bus>,
    seekable_queried: bool,
}

impl Sampler {
    pub fn new(args: &CommandLineArguments) -> Result<Self> {
        gst::init()?;
        let msg_bus = Client::new()?;
        let buffer_manager = BufferManager::new();
        let (sample_tx, sample_rx) = channel();
        let mut sampler = Sampler {
            msg_bus,
            buffer_manager,
            sample_tx,
            sample_rx,
            appsink: None,
            pipeline: None,
            gst_bus: None,
            seekable_queried: false,
        };
        if let Some(video) = &args.video {
            let _ = sampler.open(&video).map_err(|e| eprintln!("{e}"));
        }

        Ok(sampler)
    }

    pub fn open(&mut self, uri: &str) -> Result<()> {
        self.stop()?;
        let pipeline = gst::parse_launch(
            format!(
                "filesrc location={uri} ! decodebin ! videoconvert ! appsink name=biotracker_sink"
            )
            .as_str(),
        )?
        .downcast::<gst::Pipeline>()
        .expect("Expected a gst::Pipeline");

        let appsink = pipeline
            .by_name("biotracker_sink")
            .expect("Sink element not found")
            .downcast::<gst_app::AppSink>()
            .expect("Sink element is expected to be an appsink!");

        // Tell the appsink what format we want.
        // This can be set after linking the two objects, because format negotiation between
        // both elements will happen during pre-rolling of the pipeline.
        appsink.set_caps(Some(
            &gst::Caps::builder("video/x-raw")
                .field("format", gst_video::VideoFormat::Rgba.to_str())
                .build(),
        ));

        let sample_tx = self.sample_tx.clone();
        // Getting data out of the appsink is done by setting callbacks on it.
        // The appsink will then call those handlers, as soon as data is available.
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                // Add a handler to the "new-sample" signal.
                .new_sample(move |appsink| {
                    // FIXME
                    // Pull the sample in question out of the appsink's buffer.
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or_else(|| {
                        element_error!(
                            appsink,
                            gst::ResourceError::Failed,
                            ("Failed to get buffer from appsink")
                        );

                        gst::FlowError::Error
                    })?;
                    let pts = buffer
                        .pts()
                        .ok_or_else(|| {
                            element_error!(
                                appsink,
                                gst::ResourceError::Failed,
                                ("Failed to get pts from sample")
                            );

                            gst::FlowError::Error
                        })?
                        .nseconds();
                    let info = sample
                        .caps()
                        .and_then(|caps| gst_video::VideoInfo::from_caps(caps).ok())
                        .ok_or_else(|| {
                            element_error!(
                                appsink,
                                gst::ResourceError::Failed,
                                ("Failed to get video info from sample")
                            );

                            gst::FlowError::Error
                        })?;
                    sample_tx
                        .send(SampleMessage {
                            sample,
                            info,
                            pts: Timestamp(pts),
                        })
                        .map_err(|e| {
                            eprintln!("{e}");
                            gst::FlowError::Error
                        })?;
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        let bus = pipeline
            .bus()
            .expect("Pipeline without bus. Shouldn't happen!");

        self.pipeline = Some(pipeline);
        self.appsink = Some(appsink);
        self.gst_bus = Some(bus);
        self.seekable_queried = false;
        self.play()?;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        if let Some(pipeline) = &self.pipeline {
            pipeline.set_state(gst::State::Paused)?;
        }
        Ok(())
    }

    pub fn play(&self) -> Result<()> {
        if let Some(pipeline) = &self.pipeline {
            pipeline.set_state(gst::State::Playing)?;
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        if let Some(pipeline) = &self.pipeline {
            pipeline.set_state(gst::State::Null)?;
        }
        Ok(())
    }

    pub fn seek(&self, target: &Timestamp) -> Result<()> {
        if let Some(appsink) = &self.appsink {
            let seek_event = gst::event::Seek::new(
                1.0,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                gst::ClockTime::from_nseconds(target.0),
                gst::SeekType::End,
                gst::ClockTime::ZERO,
            );
            appsink.send_event(seek_event);
        }
        Ok(())
    }

    pub fn handle_command(&mut self, msg: &Message) -> Result<()> {
        match msg {
            Message::Command(State::Play) => self.play(),
            Message::Command(State::Pause) => self.pause(),
            Message::Command(State::Stop) => self.stop(),
            Message::Command(State::Seek(timestamp)) => self.seek(&timestamp),
            Message::Command(State::Open(path)) => self.open(path),
            Message::Shutdown => self.stop(),
            _ => panic!("Unexpected command"),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.msg_bus.subscribe("Command")?;
        self.msg_bus.subscribe("Shutdown")?;
        loop {
            if let Ok(sample_msg) = self.sample_rx.try_recv() {
                let buffer_ref = sample_msg.sample.buffer().unwrap();
                let buffer_map = buffer_ref.map_readable()?;
                let data_slice = buffer_map.as_slice();
                let image_buffer = self.buffer_manager.allocate(data_slice.len())?;
                unsafe {
                    image_buffer.as_slice_mut().clone_from_slice(data_slice);
                }
                self.msg_bus
                    .send(Message::Image(ImageData {
                        pts: sample_msg.pts,
                        shm_id: image_buffer.id().to_owned(),
                        width: sample_msg.info.width(),
                        height: sample_msg.info.height(),
                    }))
                    .unwrap();
            }

            while let Ok(Some(msg)) = self.msg_bus.poll(0) {
                self.handle_command(&msg)?;
            }

            if let Some(gst_bus) = &self.gst_bus {
                for msg in gst_bus.iter() {
                    use gst::MessageView;

                    match msg.view() {
                        MessageView::Warning(warn) => {
                            eprintln!("{:?}", warn);
                        }
                        MessageView::Error(err) => {
                            eprintln!("{:?}", err);
                        }
                        MessageView::StateChanged(state_changed) => {
                            if state_changed.current() == gst::State::Paused
                                && !self.seekable_queried
                            {
                                let mut seeking = gst::query::Seeking::new(gst::Format::Time);
                                if self
                                    .pipeline
                                    .as_ref()
                                    .expect("gstreamer bus without pipeline")
                                    .query(&mut seeking)
                                {
                                    self.seekable_queried = true;
                                    let (seekable, start, end) = seeking.result();
                                    if seekable && start.value() >= 0 && end.value() >= 0 {
                                        self.msg_bus.send(Message::Seekable(Seekable {
                                            start: Timestamp(start.value() as u64),
                                            end: Timestamp(end.value() as u64),
                                        }))?;
                                    }
                                }
                            } else {
                                let event_msg = match state_changed.current() {
                                    gst::State::Paused => Some(Message::Event(State::Pause)),
                                    gst::State::Playing => Some(Message::Event(State::Play)),
                                    _ => None,
                                };
                                if let Some(msg) = event_msg {
                                    self.msg_bus.send(msg)?;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
