use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    fn unix_seconds(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn unix_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[cfg(test)]
pub struct FixedClock(pub u64);

#[cfg(test)]
impl Clock for FixedClock {
    fn unix_seconds(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_is_deterministic() {
        assert_eq!(FixedClock(42).unix_seconds(), 42);
    }
}
