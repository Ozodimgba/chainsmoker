use log::info;

pub struct ReceiveStats {
    pub count: u64,
    last_log: std::time::Instant,
    last_count: u64,
}

impl ReceiveStats {
    pub fn new() -> Self {
        Self {
            count: 0,
            last_log: std::time::Instant::now(),
            last_count: 0,
        }
    }

    pub fn increment(&mut self) {
        self.count += 1;
    }

    pub fn maybe_log(&mut self) {
        if self.last_log.elapsed().as_secs() >= 10 {
            let recent = self.count - self.last_count;
            info!("Shred Stats: {} total, {} in last 10s", self.count, recent);
            self.last_count = self.count;
            self.last_log = std::time::Instant::now();
        }
    }
}