use std::io::Result;

fn main() -> Result<()> {
    tonic_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .field_attribute(
            ".biotracker.ComponentConfig.config_json",
            "#[serde(deserialize_with=\"from_map\", serialize_with=\"to_map\")]",
        )
        .field_attribute(
            ".biotracker.Pose.x_cm",
            "#[serde(deserialize_with=\"from_f32_or_null\")]",
        )
        .field_attribute(
            ".biotracker.Pose.y_cm",
            "#[serde(deserialize_with=\"from_f32_or_null\")]",
        )
        .field_attribute(
            ".biotracker.SkeletonNode.x",
            "#[serde(deserialize_with=\"from_f32_or_null\")]",
        )
        .field_attribute(
            ".biotracker.SkeletonNode.y",
            "#[serde(deserialize_with=\"from_f32_or_null\")]",
        )
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
