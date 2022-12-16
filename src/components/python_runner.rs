use anyhow::{anyhow, Result};
use libtracker::Component;
use std::path::PathBuf;

pub struct PythonRunner {
    venv_path: PathBuf,
    component_path: PathBuf,
}

impl Component for PythonRunner {
    fn run(&mut self) -> Result<()> {
        let cmd = format!(
            "source {}/bin/activate && python3 {}",
            self.venv_path.to_str().expect("Invalid python venv path"),
            self.component_path
                .to_str()
                .expect("Invalid python component path")
        );
        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(cmd.clone())
            .output()?;

        match output.status.success() {
            true => Ok(()),
            false => {
                let stdout = String::from_utf8(output.stdout)
                    .unwrap_or("Failed to decode stdout as utf8".to_string());
                let stderr = String::from_utf8(output.stderr)
                    .unwrap_or("Failed to decode stderr as utf8".to_string());
                Err(anyhow!(
                    "Commandline: `{}` failed with {}.\nstdout:\n{}\nstderr:\n{}\n",
                    cmd,
                    output.status,
                    stdout,
                    stderr
                ))
            }
        }
    }
}

impl PythonRunner {
    pub fn new(venv_path: PathBuf, component_path: PathBuf) -> Self {
        Self {
            venv_path,
            component_path,
        }
    }
}
