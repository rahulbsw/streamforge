use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Statistics collection for MirrorMaker
#[derive(Debug, Default)]
pub struct Stats {
    processed: AtomicU64,
    filtered: AtomicU64,
    transformed: AtomicU64,
    completed: AtomicU64,
    errors: AtomicU64,
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn processed(&self) {
        self.processed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn filtered(&self) {
        self.filtered.fetch_add(1, Ordering::Relaxed);
    }

    pub fn transformed(&self) {
        self.transformed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn completed(&self) {
        self.completed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_processed(&self) -> u64 {
        self.processed.load(Ordering::Relaxed)
    }

    pub fn get_filtered(&self) -> u64 {
        self.filtered.load(Ordering::Relaxed)
    }

    pub fn get_transformed(&self) -> u64 {
        self.transformed.load(Ordering::Relaxed)
    }

    pub fn get_completed(&self) -> u64 {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn get_errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            processed: self.get_processed(),
            filtered: self.get_filtered(),
            transformed: self.get_transformed(),
            completed: self.get_completed(),
            errors: self.get_errors(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatsSnapshot {
    pub processed: u64,
    pub filtered: u64,
    pub transformed: u64,
    pub completed: u64,
    pub errors: u64,
}

impl StatsSnapshot {
    pub fn rate(&self, other: &StatsSnapshot, duration_secs: f64) -> StatsRate {
        let calc_rate = |current: u64, previous: u64| -> f64 {
            if duration_secs > 0.0 {
                (current.saturating_sub(previous)) as f64 / duration_secs
            } else {
                0.0
            }
        };

        StatsRate {
            processed_rate: calc_rate(self.processed, other.processed),
            filtered_rate: calc_rate(self.filtered, other.filtered),
            transformed_rate: calc_rate(self.transformed, other.transformed),
            completed_rate: calc_rate(self.completed, other.completed),
            error_rate: calc_rate(self.errors, other.errors),
        }
    }
}

#[derive(Debug)]
pub struct StatsRate {
    pub processed_rate: f64,
    pub filtered_rate: f64,
    pub transformed_rate: f64,
    pub completed_rate: f64,
    pub error_rate: f64,
}

/// Statistics reporter
pub struct StatsReporter {
    stats: Arc<Stats>,
    last_snapshot: StatsSnapshot,
    last_report_time: std::time::Instant,
}

impl StatsReporter {
    pub fn new(stats: Arc<Stats>) -> Self {
        Self {
            last_snapshot: stats.snapshot(),
            last_report_time: std::time::Instant::now(),
            stats,
        }
    }

    pub fn report(&mut self) {
        let now = std::time::Instant::now();
        let duration = now.duration_since(self.last_report_time);
        let duration_secs = duration.as_secs_f64();

        let current = self.stats.snapshot();
        let rate = current.rate(&self.last_snapshot, duration_secs);

        tracing::info!(
            "Stats: processed={} ({:.1}/s), filtered={} ({:.1}/s), transformed={} ({:.1}/s), completed={} ({:.1}/s), errors={} ({:.1}/s)",
            current.processed, rate.processed_rate,
            current.filtered, rate.filtered_rate,
            current.transformed, rate.transformed_rate,
            current.completed, rate.completed_rate,
            current.errors, rate.error_rate
        );

        self.last_snapshot = current;
        self.last_report_time = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats() {
        let stats = Stats::new();

        stats.processed();
        stats.processed();
        stats.filtered();
        stats.completed();

        assert_eq!(stats.get_processed(), 2);
        assert_eq!(stats.get_filtered(), 1);
        assert_eq!(stats.get_completed(), 1);
    }

    #[test]
    fn test_stats_rate() {
        let prev = StatsSnapshot {
            processed: 100,
            filtered: 10,
            transformed: 90,
            completed: 90,
            errors: 0,
        };

        let current = StatsSnapshot {
            processed: 200,
            filtered: 20,
            transformed: 180,
            completed: 180,
            errors: 0,
        };

        let rate = current.rate(&prev, 10.0);
        assert_eq!(rate.processed_rate, 10.0); // 100 messages / 10 seconds
        assert_eq!(rate.filtered_rate, 1.0);
    }
}
