use anyhow::{Context, Result};

use super::{ComponentConfig, PythonConfig};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

pub struct PythonProcess {
    log_task: tokio::task::JoinHandle<Result<()>>,
    id: String,
    child: Child,
}

impl PythonProcess {
    pub fn start(config: &ComponentConfig, python_config: &PythonConfig) -> Result<Self> {
        let commandline = format!(
            "export BIOTRACKER_COMPONENT_ADDRESS='{}'; . '{}/bin/activate'; exec python3 {} 2>&1",
            config.address, python_config.venv, python_config.cmd
        );
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(commandline)
            .stdout(Stdio::piped())
            .spawn()?;
        let mut stdout_reader =
            BufReader::new(child.stdout.take().context("stdout not available")?).lines();
        let id = config.id.clone();
        let log_task = tokio::spawn(async move {
            while let Some(line) = stdout_reader.next_line().await? {
                log::warn!(target: &id, "{}", line);
            }
            Ok(())
        });

        Ok(Self {
            log_task,
            id: config.id.clone(),
            child,
        })
    }

    pub async fn stop(&mut self) {
        match self.kill().await {
            Ok(_) => {}
            Err(e) => {
                log::warn!("Failed to kill process {}: {}", self.id, e);
            }
        }
        self.log_task.abort();
    }

    async fn kill(&mut self) -> Result<()> {
        if let Ok(None) = self.child.try_wait() {
            self.child.kill().await?;
        }
        Ok(())
    }
}
