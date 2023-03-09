pub struct ScopedTimer {
    name: String,
    start: std::time::Instant,
}

impl ScopedTimer {
    #[allow(dead_code)]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            start: std::time::Instant::now(),
        }
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        println!("{}: {:.2?}", self.name, self.start.elapsed());
    }
}

pub fn framenumber_to_hhmmss(framenumber: u32, fps: f64) -> String {
    let duration = std::time::Duration::from_secs_f64(framenumber as f64 / fps);
    let seconds = duration.as_secs() % 60;
    let minutes = (duration.as_secs() / 60) % 60;
    let hours = (duration.as_secs() / 60) / 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

#[macro_export]
macro_rules! log_error {
    ( $x:expr ) => {
        match $x {
            Ok(_) => {}
            Err(e) => {
                log::error!("{}", e);
            }
        }
    };
}
