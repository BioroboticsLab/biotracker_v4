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
    pub config: String,
    /// Port for biotracker core
    #[arg(long, default_value_t = 27342)]
    pub port: u32,
    /// Seek to frame
    #[arg(long)]
    pub seek: Option<u32>,
    /// Number of OpenCV worker threads
    #[arg(long, default_value_t = 4)]
    pub cv_worker_threads: u32,
    /// Path to robofish track file
    #[arg(long)]
    pub track: Option<String>,
    /// Force loading of camera settings, this makes it possible to apply undistortion to videos.
    #[arg(long)]
    pub force_camera_config: Option<String>,
    /// Start of range of ports which are assigned to components.
    #[arg(long, default_value_t = 28000)]
    pub port_range_start: u16,
}
