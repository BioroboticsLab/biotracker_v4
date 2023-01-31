use std::io::Result;

fn main() -> Result<()> {
    tonic_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile(
            &[
                "protocol/message.proto",
                "protocol/tracking.proto",
                "protocol/video.proto",
            ],
            &["protocol/"],
        )?;
    Ok(())
}
