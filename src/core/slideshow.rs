//! Lógica pura del slideshow: intervalos y límites.
//!
//! `core/` no depende de la UI; `app.rs` orquesta el avance con estas funciones.

use std::time::Duration;

/// Intervalo mínimo del slideshow.
pub const MIN_INTERVAL: Duration = Duration::from_secs(1);
/// Intervalo máximo del slideshow.
pub const MAX_INTERVAL: Duration = Duration::from_secs(60);
/// Intervalo por defecto.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

/// Intervalo por defecto del slideshow (5 s).
pub fn default_interval() -> Duration {
    DEFAULT_INTERVAL
}

/// Acelera el slideshow: divide el intervalo por dos, sin bajar de 1 s.
pub fn faster(interval: Duration) -> Duration {
    (interval / 2).max(MIN_INTERVAL)
}

/// Ralentiza el slideshow: duplica el intervalo, sin superar 60 s.
pub fn slower(interval: Duration) -> Duration {
    (interval * 2).min(MAX_INTERVAL)
}

/// `true` si `elapsed` ya superó `interval` (toca avanzar).
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
