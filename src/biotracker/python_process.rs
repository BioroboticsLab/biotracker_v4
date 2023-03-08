use anyhow::{Context, Result};

use super::{ComponentConfig, PythonConfig};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};

struct PythonProcess {
    pub stdout_reader: Option<Lines<BufReader<ChildStdout>>>,
    id: String,
    child: Child,
}

impl PythonProcess {
    pub fn new(config: &ComponentConfig, python_config: &PythonConfig) -> Result<Self> {
        let commandline = format!(
            "export BIOTRACKER_COMPONENT_ADDRESS='{}'; . '{}/bin/activate'; exec python3 {} 2>&1",
            config.address, python_config.venv, python_config.cmd
        );
        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(commandline)
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout_reader =
            Some(BufReader::new(child.stdout.take().context("stdout not available")?).lines());

        Ok(Self {
            stdout_reader,
            id: config.id.clone(),
            child,
        })
    }

    pub async fn kill(&mut self) -> Result<()> {
        if let Ok(None) = self.child.try_wait() {
            self.child.kill().await?;
        }
        Ok(())
    }
}

pub struct ProcessManager {
    tasks: Vec<tokio::task::JoinHandle<()>>,
    processes: Vec<PythonProcess>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            tasks: vec![],
            processes: vec![],
        }
    }

    pub async fn start_python_process(
        &mut self,
        config: &ComponentConfig,
        python_config: &PythonConfig,
    ) -> Result<()> {
        let process = PythonProcess::new(config, python_config)?;
        self.processes.push(process);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        let futures = self
            .processes
            .iter_mut()
            .map(|process| process.kill())
            .collect::<Vec<_>>();
        futures::future::join_all(futures).await;
        self.processes.clear();
        Ok(())
    }

    pub fn run(&mut self) {
        let stdout_readers = self
            .processes
            .iter_mut()
            .map(|process| (process.stdout_reader.take().unwrap(), process.id.clone()))
            .collect::<Vec<_>>();
        let stdout_futures = stdout_readers
            .into_iter()
            .map(|(mut reader, id)| async move {
                while let Some(line) = reader.next_line().await.unwrap() {
                    log::warn!(target: &id, "{}", line);
                }
            })
            .collect::<Vec<_>>();
        self.tasks.push(tokio::spawn(async move {
            futures::future::join_all(stdout_futures).await;
        }));
    }
}
