use super::protocol::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PythonConfig {
    pub venv: String,
    pub cmd: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ComponentConfig {
    pub id: String,
    pub services: Vec<String>,
    pub address: String,
    pub config_json: serde_json::Value,
    pub python_config: Option<PythonConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BiotrackerConfig {
    pub components: Vec<ComponentConfig>,
    pub arena: Arena,
}

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
