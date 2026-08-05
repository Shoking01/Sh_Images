use std::time::Duration;
pub const MIN_INTERVAL: Duration = Duration::from_secs(1);
pub const MAX_INTERVAL: Duration = Duration::from_secs(60);
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);
pub fn default_interval() -> Duration {
    DEFAULT_INTERVAL
}
pub fn faster(interval: Duration) -> Duration {
    (interval / 2).max(MIN_INTERVAL)
}
pub fn slower(interval: Duration) -> Duration {
    (interval * 2).min(MAX_INTERVAL)
}
pub fn elapsed_reached(elapsed: Duration, interval: Duration) -> bool {
    elapsed >= interval
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_interval_is_five_seconds() {
        assert_eq!(default_interval(), Duration::from_secs(5));
    }

    #[test]
    fn faster_halves_interval() {
        assert_eq!(faster(Duration::from_secs(5)), Duration::from_millis(2500));
    }

    #[test]
    fn faster_clamps_at_one_second() {
        assert_eq!(faster(Duration::from_secs(1)), Duration::from_secs(1));
        assert_eq!(faster(Duration::from_secs(2)), Duration::from_secs(1));
    }

    #[test]
    fn faster_subsecond_interval_clamps_up_to_minimum() {
        assert_eq!(faster(Duration::from_millis(500)), Duration::from_secs(1));
        assert_eq!(faster(Duration::ZERO), Duration::from_secs(1));
    }

    #[test]
    fn slower_doubles_interval() {
        assert_eq!(slower(Duration::from_secs(5)), Duration::from_secs(10));
    }

    #[test]
    fn slower_clamps_at_sixty_seconds() {
        assert_eq!(slower(Duration::from_secs(60)), Duration::from_secs(60));
        assert_eq!(slower(Duration::from_secs(30)), Duration::from_secs(60));
    }

    #[test]
    fn elapsed_reached_compares_correctly() {
        assert!(elapsed_reached(
            Duration::from_secs(5),
            Duration::from_secs(5)
        ));
        assert!(!elapsed_reached(
            Duration::from_secs(4),
            Duration::from_secs(5)
        ));
    }
}
