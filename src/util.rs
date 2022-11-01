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
