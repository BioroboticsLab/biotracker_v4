use std::io::Result;

fn main() -> Result<()> {
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    config.compile_protos(
        &[
            "protocol/experiment.proto",
            "protocol/message.proto",
            "protocol/tracking.proto",
            "protocol/video.proto",
        ],
        &["protocol/"],
    )?;
    Ok(())
}
