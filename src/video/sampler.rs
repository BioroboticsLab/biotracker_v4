use anyhow::Result;
use derive_more::{Display, Error};
use gst::{prelude::*, BufferMap};

use std::sync::mpsc::Receiver;

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

pub struct VideoSample {
    sample: gst::Sample,
}

impl VideoSample {
    pub fn data(&self) -> Option<BufferMap<gst::buffer::Readable>> {
        if let Some(buffer_ref) = self.sample.buffer() {
            if let Ok(buffer_map) = buffer_ref.map_readable() {
                return Some(buffer_map);
            } else {
                eprintln!("Failed to map buffer");
            }
        }
        None
    }
}

pub struct Sampler {
    pub sample_rx: Receiver<VideoSample>,
    pipeline: gst::Pipeline,
    bus: gst::Bus,
}

impl Sampler {
    pub fn new() -> Result<Self> {
        gst::init()?;

        let pipeline = gst::Pipeline::new(Some("biotracker_video"));
        let src = gst::ElementFactory::make("videotestsrc", None)
            .map_err(|_| MissingElement("videotestsrc"))?;
        let sink =
            gst::ElementFactory::make("appsink", None).map_err(|_| MissingElement("appsink"))?;

        pipeline.add_many(&[&src, &sink])?;
        src.link(&sink)?;

        let appsink = sink
            .dynamic_cast::<gst_app::AppSink>()
            .expect("Sink element is expected to be an appsink!");

        // Tell the appsink what format we want. It will then be the audiotestsrc's job to
        // provide the format we request.
        // This can be set after linking the two objects, because format negotiation between
        // both elements will happen during pre-rolling of the pipeline.
        appsink.set_caps(Some(
            &gst::Caps::builder("video/x-raw")
                .field("width", 1280)
                .field("height", 720)
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
                    if let Err(e) = sample_tx.send(VideoSample { sample }) {
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
            sample_rx,
            pipeline,
            bus,
        })
    }

    pub fn play(&self) -> Result<()> {
        self.pipeline.set_state(gst::State::Playing)?;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.pipeline.set_state(gst::State::Null)?;
        Ok(())
    }

    pub fn poll_event(&self) {
        for msg in self.bus.iter() {
            use gst::MessageView;

            match msg.view() {
                MessageView::Error(err) => {
                    eprintln!("{:?}", err);
                }
                _ => {}
            }
        }
    }
}
