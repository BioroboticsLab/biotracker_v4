use super::protocol::*;
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
