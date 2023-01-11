use anyhow::{anyhow, Result};
use libtracker::Component;

pub struct PythonRunner {
    venv: String,
    cmd: String,
}

impl Component for PythonRunner {
    fn run(&mut self) -> Result<()> {
        let shell_cmd = format!("source {}/bin/activate && python3 {}", self.venv, self.cmd);
        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(shell_cmd.clone())
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
                    shell_cmd,
                    output.status,
                    stdout,
                    stderr
                ))
            }
        }
    }
}

impl PythonRunner {
    pub fn new(venv: String, cmd: String) -> Self {
        Self { venv, cmd }
    }
}
