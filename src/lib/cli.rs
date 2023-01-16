use std::path::PathBuf;

use clap::Parser;

/// Distributed framework for animal tracking
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
    #[arg(long)]
    pub save_video: Option<String>,
    #[arg(long)]
    pub config: PathBuf,
}
