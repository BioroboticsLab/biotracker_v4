use std::time::Instant;

pub struct DurationMetric {
    last_update: Instant,
}

impl Default for DurationMetric {
    fn default() -> Self {
        Self {
            last_update: Instant::now(),
        }
    }
}

impl DurationMetric {
    pub fn update(&mut self) -> f32 {
        let now = Instant::now();
        let elapsed = now - self.last_update;
        self.last_update = now;
        elapsed.as_secs_f32()
    }
}
