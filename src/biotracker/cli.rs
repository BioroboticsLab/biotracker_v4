use std::path::PathBuf;

use clap::Parser;

/// Distributed framework for animal tracking
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArguments {
    /// Open video file on startup
    #[arg(short, long)]
    pub video: Option<String>,
    /// Start experiment with <count> entities
    #[arg(long)]
    pub entity_count: Option<u64>,
    /// Skip frames if tracking is too slow
    #[arg(long)]
    pub realtime: Option<bool>,
    /// Path to configuration json
    #[arg(long)]
    pub config: PathBuf,
}
