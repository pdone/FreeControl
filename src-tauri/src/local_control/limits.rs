use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct RateLimiter(Mutex<VecDeque<Instant>>);

impl RateLimiter {
    pub fn allow(&self, limit: usize, window: Duration) -> bool {
        let now = Instant::now();
        let mut hits = self.0.lock().unwrap();
        while hits
            .front()
            .is_some_and(|hit| now.duration_since(*hit) >= window)
        {
            hits.pop_front();
        }
        if hits.len() >= limit {
            return false;
        }
        hits.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_limit_inside_window() {
        let limiter = RateLimiter::default();
        assert!(limiter.allow(2, Duration::from_secs(60)));
        assert!(limiter.allow(2, Duration::from_secs(60)));
        assert!(!limiter.allow(2, Duration::from_secs(60)));
    }
}
