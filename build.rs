use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(
        &[
            "protocol/tracking.proto",
            "protocol/video.proto",
            "protocol/experiment.proto",
        ],
        &["protocol/"],
    )?;
    Ok(())
}
