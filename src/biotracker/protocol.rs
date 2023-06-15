use anyhow::Result;

tonic::include_proto!("biotracker");

pub use bio_tracker_command::Command;
pub use feature_detector_client::FeatureDetectorClient;
pub use matcher_client::MatcherClient;

impl BiotrackerConfig {
    pub fn load(path: &str) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let config = serde_json::from_reader(reader)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let file = std::fs::File::create(path)?;
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }
}

fn from_f32_or_null<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: f32 = match serde::Deserialize::deserialize(deserializer) {
        Ok(x) => x,
        Err(_) => std::f32::NAN,
    };
    Ok(s)
}

fn from_map<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: serde_json::Map<String, serde_json::Value> =
        serde::Deserialize::deserialize(deserializer)?;
    let config_json = serde_json::to_string(&s).unwrap();
    Ok(config_json)
}

fn to_map<S>(x: &str, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(x).unwrap();
    s.collect_map(map)
}
