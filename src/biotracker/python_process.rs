use anyhow::{Context, Result};

use super::{ComponentConfig, PythonConfig};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub struct PythonProcess {
    stdout_log_task: tokio::task::JoinHandle<Result<()>>,
    stderr_log_task: tokio::task::JoinHandle<Result<()>>,
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
            "export BIOTRACKER_COMPONENT_ADDRESS='{}'; . '{}/bin/activate'; exec python3 {}",
            address, python_config.venv, python_config.cmd
        );
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(commandline)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let mut stdout_reader =
            BufReader::new(child.stderr.take().context("stdout not available")?).lines();
        let mut stderr_reader =
            BufReader::new(child.stdout.take().context("stdout not available")?).lines();
        let stdout_id = config.id.clone();
        let stdout_log_task = tokio::spawn(async move {
            while let Some(line) = stdout_reader.next_line().await? {
                log::warn!("{}: {}", stdout_id, line);
            }
            Ok(())
        });
        let stderr_id = config.id.clone();
        let stderr_log_task = tokio::spawn(async move {
            while let Some(line) = stderr_reader.next_line().await? {
                log::error!("{}: {}", stderr_id, line);
            }
            Ok(())
        });
        let id = config.id.clone();
        let wait_task = tokio::spawn(async move {
            let status = child.wait().await?;
            log::error!("{}: Python process exited with status {}.", id, status);
            Ok(())
        });

        Ok(Self {
            stdout_log_task,
            stderr_log_task,
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
        self.stdout_log_task.abort();
        self.stderr_log_task.abort();
    }

    async fn kill(&mut self) -> Result<()> {
        self.wait_task.abort();
        Ok(())
    }
}
