use clap::Parser;
//use std::sync::Arc;

/// Modular framework for animal tracking
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArguments {
    /// Open video file on startup
    #[arg(short, long)]
    pub video: Option<String>,
    #[arg(short, long)]
    pub inspect_bus: Option<String>,
    #[arg(short, long)]
    pub entity_count: Option<u64>,
}
