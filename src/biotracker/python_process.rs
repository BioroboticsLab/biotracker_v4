use anyhow::Result;

use super::{ComponentConfig, PythonConfig};

pub struct PythonProcess {
    child: std::process::Child,
}

impl PythonProcess {
    pub fn new(config: &ComponentConfig, python_config: &PythonConfig) -> Result<Self> {
        let commandline = format!(
            "export BIOTRACKER_COMPONENT_ADDRESS='{}'; . {}/bin/activate && python3 {}",
            config.address, python_config.venv, python_config.cmd
        );
        let child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(commandline)
            .spawn()?;

        Ok(Self { child })
    }
}

impl Drop for PythonProcess {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            self.child.kill().unwrap();
        }
    }
}
