use anyhow::Result;
use derive_more::{Display, Error};
use gst::prelude::*;

use std::sync::mpsc::Receiver;

use crate::core::{self, Timestamp};

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

pub struct Sampler {
    pub sample_rx: Receiver<gst::Sample>,
    appsink: gst_app::AppSink,
    pipeline: gst::Pipeline,
    bus: gst::Bus,
    seekable_queried: bool,
}

pub enum SamplerEvent {
    Seekable(core::VideoSeekable),
    Event(core::VideoState),
}

impl Sampler {
    pub fn new(uri: &str) -> Result<Self> {
        gst::init()?;

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

        let (sample_tx, sample_rx) = std::sync::mpsc::channel();
        // Getting data out of the appsink is done by setting callbacks on it.
        // The appsink will then call those handlers, as soon as data is available.
        appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                // Add a handler to the "new-sample" signal.
                .new_sample(move |appsink| {
                    // Pull the sample in question out of the appsink's buffer.
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    if let Err(e) = sample_tx.send(sample) {
                        eprintln!("send error: {e}");
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        let bus = pipeline
            .bus()
            .expect("Pipeline without bus. Shouldn't happen!");

        Ok(Sampler {
            appsink,
            sample_rx,
            pipeline,
            bus,
            seekable_queried: false,
        })
    }

    pub fn pause(&self) -> Result<bool> {
        self.pipeline.set_state(gst::State::Paused)?;
        Ok(true)
    }

    pub fn play(&self) -> Result<bool> {
        self.pipeline.set_state(gst::State::Playing)?;
        Ok(true)
    }

    pub fn stop(&self) -> Result<bool> {
        self.pipeline.set_state(gst::State::Null)?;
        Ok(true)
    }

    pub fn seek(&self, target: &core::Timestamp) -> Result<bool> {
        let seek_event = gst::event::Seek::new(
            1.0,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            gst::ClockTime::from_nseconds(target.0),
            gst::SeekType::End,
            gst::ClockTime::ZERO,
        );
        self.appsink.send_event(seek_event);
        Ok(true)
    }

    pub fn handle_command(&mut self, msg: &core::Message) -> Result<bool> {
        match msg {
            core::Message::Command(core::VideoState::Play) => self.play(),
            core::Message::Command(core::VideoState::Pause) => self.pause(),
            core::Message::Command(core::VideoState::Seek(timestamp)) => self.seek(&timestamp),
            _ => Ok(false),
        }
    }

    pub fn poll_event(&mut self) -> Option<SamplerEvent> {
        for msg in self.bus.iter() {
            use gst::MessageView;

            match msg.view() {
                MessageView::Warning(warn) => {
                    eprintln!("{:?}", warn);
                }
                MessageView::Error(err) => {
                    eprintln!("{:?}", err);
                }
                MessageView::StateChanged(state_changed) => {
                    if state_changed.current() == gst::State::Paused && !self.seekable_queried {
                        let mut seeking = gst::query::Seeking::new(gst::Format::Time);
                        if self.pipeline.query(&mut seeking) {
                            self.seekable_queried = true;
                            let (seekable, start, end) = seeking.result();
                            if seekable && start.value() >= 0 && end.value() >= 0 {
                                return Some(SamplerEvent::Seekable(core::VideoSeekable {
                                    start: Timestamp(start.value() as u64),
                                    end: Timestamp(end.value() as u64),
                                }));
                            }
                        }
                    } else {
                        return match state_changed.current() {
                            gst::State::Paused => {
                                Some(SamplerEvent::Event(core::VideoState::Pause))
                            }
                            gst::State::Playing => {
                                Some(SamplerEvent::Event(core::VideoState::Play))
                            }
                            _ => None,
                        };
                    }
                }
                _ => {}
            }
        }
        return None;
    }
}
