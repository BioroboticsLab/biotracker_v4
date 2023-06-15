use anyhow::{Context, Result};

use super::{ComponentConfig, PythonConfig};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub struct PythonProcess {
    log_task: tokio::task::JoinHandle<Result<()>>,
    wait_task: tokio::task::JoinHandle<Result<()>>,
    id: String,
}

impl PythonProcess {
    pub fn start(
        config: &ComponentConfig,
        python_config: &PythonConfig,
        address: String,
    ) -> Result<Self> {
        let commandline = format!(
            "export BIOTRACKER_COMPONENT_ADDRESS='{}'; . '{}/bin/activate'; exec python3 {} 2>&1",
            address, python_config.venv, python_config.cmd
        );
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(commandline)
            .stdout(Stdio::piped())
            .kill_on_drop(true)
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
        let id = config.id.clone();
        let wait_task = tokio::spawn(async move {
            let status = child.wait().await?;
            log::error!(target: &id, "Python process exited with status {}", status);
            Ok(())
        });

        Ok(Self {
            log_task,
            wait_task,
            id: config.id.clone(),
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
        self.wait_task.abort();
        Ok(())
    }
}
