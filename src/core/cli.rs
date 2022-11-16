use clap::Parser;

/// Modular framework for animal tracking
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArguments {
    /// Open video file on startup
    #[arg(short, long)]
    pub video: Option<String>,
}
