use libtracker::protocol::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PythonConfig {
    pub venv: String,
    pub cmd: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComponentConfig {
    pub id: String,
    pub typ: ComponentType,
    pub config_json: serde_json::Value,
    pub python_config: Option<PythonConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BiotrackerConfig {
    pub components: Vec<ComponentConfig>,
    pub arena: Arena,
}
