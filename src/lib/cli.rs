use std::path::PathBuf;

use clap::Parser;

/// Modular framework for animal tracking
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArguments {
    /// Open video file on startup
    #[arg(short, long)]
    pub video: Option<String>,
    #[arg(short, long)]
    pub inspect_bus: Option<String>,
    #[arg(long)]
    pub entity_count: Option<u64>,
    #[arg(long, requires = "tracker_cmd_path")]
    pub tracker_venv: Option<PathBuf>,
    #[arg(long, requires = "tracker_venv_path")]
    pub tracker_cmd: Option<PathBuf>,
    #[arg(long)]
    pub save_video: Option<PathBuf>,
}
