use anyhow::Result;
use clap::Parser;

/// Distributed framework for animal tracking
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct CommandLineArguments {
    /// Open and play video file on startup
    #[arg(short, long)]
    pub video: Option<std::path::PathBuf>,
    /// Start experiment with <count> entities
    #[arg(long)]
    pub entity_count: Option<u64>,
    /// Skip frames if tracking is too slow
    #[arg(long)]
    pub realtime: Option<bool>,
    /// Path to configuration json
    #[arg(long)]
    pub config: std::path::PathBuf,
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
    pub track: Option<std::path::PathBuf>,
    /// Force loading of camera settings, this makes it possible to apply undistortion to videos.
    #[arg(long)]
    pub force_camera_config: Option<String>,
    /// Start of range of ports which are assigned to components.
    #[arg(long, default_value_t = 28000)]
    pub port_range_start: u16,
    /// Run biotracker in headless mode, without GUI
    #[arg(long)]
    pub headless: bool,
}

impl CommandLineArguments {
    pub fn canonicalize_paths(mut self) -> Result<Self> {
        canonicalize_path(&mut self.config)?;
        for arg in [&mut self.video, &mut self.track].iter_mut() {
            match arg {
                Some(path) => canonicalize_path(path)?,
                None => {}
            }
        }
        Ok(self)
    }
}

fn canonicalize_path(path: &mut std::path::PathBuf) -> Result<()> {
    match path.canonicalize() {
        Ok(canonicalized) => {
            *path = canonicalized;
            return Ok(());
        }
        Err(e) => return Err(anyhow::anyhow!("{}: {}", e, path.display())),
    };
}
