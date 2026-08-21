//! Aritmética de búsqueda (seek) ±5 s.
//!
//! Todo con `saturating_*`: `Duration - Duration` hace panic en underflow y
//! `clippy -D warnings` no lo detecta.

use std::time::Duration;

/// Salto de los controles de avance y retroceso.
pub const SEEK_STEP: Duration = Duration::from_secs(5);

/// Avanza `SEEK_STEP`, sin pasarse de la duración conocida.
pub fn seek_forward(position: Duration, duration: Option<Duration>) -> Duration {
    clamp_to_duration(position.saturating_add(SEEK_STEP), duration)
}

/// Retrocede `SEEK_STEP`, con suelo en cero.
pub fn seek_backward(position: Duration) -> Duration {
    position.saturating_sub(SEEK_STEP)
}

/// Salto relativo en segundos, positivo o negativo.
pub fn seek_relative(position: Duration, delta_secs: i64, duration: Option<Duration>) -> Duration {
    let target = if delta_secs >= 0 {
        position.saturating_add(Duration::from_secs(delta_secs.unsigned_abs()))
    } else {
        // unsigned_abs evita el overflow de `-i64::MIN`.
        position.saturating_sub(Duration::from_secs(delta_secs.unsigned_abs()))
    };
    clamp_to_duration(target, duration)
}

/// Salto absoluto (barra de progreso), acotado a `[0, duration]`.
pub fn seek_absolute(target: Duration, duration: Option<Duration>) -> Duration {
    clamp_to_duration(target, duration)
}

/// `true` si la posición resultante es el final exacto del video.
///
/// Con duración desconocida siempre es `false`: sólo el backend sabe cuándo se
/// acabaron las muestras.
pub fn seek_reaches_end(new_position: Duration, duration: Option<Duration>) -> bool {
    match duration {
        Some(total) => new_position >= total,
        None => false,
    }
}

/// Convierte una fracción de la barra de progreso `[0.0, 1.0]` en una posición.
pub fn position_from_fraction(fraction: f32, duration: Option<Duration>) -> Duration {
    let Some(total) = duration else {
        return Duration::ZERO;
    };
    let f = fraction.clamp(0.0, 1.0);
    Duration::from_secs_f64(total.as_secs_f64() * f64::from(f))
}

/// Progreso `[0.0, 1.0]` para dibujar la barra. Cero si la duración es
/// desconocida o nula.
pub fn progress_fraction(position: Duration, duration: Option<Duration>) -> f32 {
    match duration {
        Some(total) if total > Duration::ZERO => {
            (position.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0) as f32
        }
        _ => 0.0,
    }
}

fn clamp_to_duration(value: Duration, duration: Option<Duration>) -> Duration {
    match duration {
        Some(total) => value.min(total),
        None => value,
    }
}

/// `true` si una muestra con `pts` debe descartarse por pertenecer al tramo
/// anterior a un salto pendiente.
///
/// El decodificador aterriza en el fotograma clave anterior al objetivo, así
/// que las primeras muestras tras un `SeekTo` pueden llevar PTS anterior al
/// target. Reproducirlas sería oír/ver contenido ya pasado. El filtro se
/// desactiva en cuanto llega la primera muestra con `pts >= target`.
pub fn discard_before_seek_target(pts: Duration, seek_target: Option<Duration>) -> bool {
    match seek_target {
        Some(target) => pts < target,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOTAL: Option<Duration> = Some(Duration::from_secs(60));

    #[test]
    fn seek_step_is_five_seconds() {
        assert_eq!(SEEK_STEP, Duration::from_secs(5));
    }

    #[test]
    fn seek_forward_adds_five_seconds() {
        assert_eq!(
            seek_forward(Duration::from_secs(10), TOTAL),
            Duration::from_secs(15)
        );
    }

    #[test]
    fn seek_backward_subtracts_five_seconds() {
        assert_eq!(
            seek_backward(Duration::from_secs(10)),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn seek_backward_at_zero_stays_zero() {
        // Sin saturating_sub esto haría panic.
        assert_eq!(seek_backward(Duration::ZERO), Duration::ZERO);
    }

    #[test]
    fn seek_backward_below_five_seconds_clamps_to_zero() {
        assert_eq!(seek_backward(Duration::from_secs(2)), Duration::ZERO);
        assert_eq!(seek_backward(Duration::from_millis(1)), Duration::ZERO);
    }

    #[test]
    fn seek_forward_clamps_at_duration() {
        assert_eq!(
            seek_forward(Duration::from_secs(58), TOTAL),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn seek_forward_in_last_second_lands_exactly_on_duration() {
        let pos = seek_forward(Duration::from_millis(59_500), TOTAL);
        assert_eq!(pos, Duration::from_secs(60));
        assert!(seek_reaches_end(pos, TOTAL));
    }

    #[test]
    fn seek_forward_at_duration_is_noop() {
        let end = Duration::from_secs(60);
        assert_eq!(seek_forward(end, TOTAL), end);
        assert_eq!(seek_forward(seek_forward(end, TOTAL), TOTAL), end);
    }

    #[test]
    fn seek_reaches_end_true_only_at_duration() {
        assert!(!seek_reaches_end(Duration::from_secs(59), TOTAL));
        assert!(seek_reaches_end(Duration::from_secs(60), TOTAL));
        assert!(seek_reaches_end(Duration::from_secs(61), TOTAL));
    }

    #[test]
    fn seek_with_unknown_duration_advances_unbounded() {
        let pos = seek_forward(Duration::from_secs(1_000_000), None);
        assert_eq!(pos, Duration::from_secs(1_000_005));
        assert!(!seek_reaches_end(pos, None));
    }

    #[test]
    fn seek_with_zero_duration_always_returns_zero() {
        let zero = Some(Duration::ZERO);
        assert_eq!(seek_forward(Duration::ZERO, zero), Duration::ZERO);
        assert_eq!(seek_absolute(Duration::from_secs(9), zero), Duration::ZERO);
        assert_eq!(progress_fraction(Duration::ZERO, zero), 0.0);
        assert_eq!(position_from_fraction(0.5, zero), Duration::ZERO);
    }

    #[test]
    fn seek_absolute_clamps_both_ends() {
        assert_eq!(
            seek_absolute(Duration::from_secs(999), TOTAL),
            Duration::from_secs(60)
        );
        assert_eq!(seek_absolute(Duration::ZERO, TOTAL), Duration::ZERO);
    }

    #[test]
    fn seek_relative_negative_saturates_without_overflow() {
        // -i64::MIN desbordaría con .abs(); unsigned_abs no.
        assert_eq!(
            seek_relative(Duration::from_secs(10), i64::MIN, TOTAL),
            Duration::ZERO
        );
        assert_eq!(
            seek_relative(Duration::from_secs(10), i64::MAX, TOTAL),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn seek_relative_matches_forward_and_backward() {
        let pos = Duration::from_secs(20);
        assert_eq!(seek_relative(pos, 5, TOTAL), seek_forward(pos, TOTAL));
        assert_eq!(seek_relative(pos, -5, TOTAL), seek_backward(pos));
    }

    #[test]
    fn seek_is_idempotent_at_bounds() {
        let end = Duration::from_secs(60);
        assert_eq!(seek_forward(seek_forward(end, TOTAL), TOTAL), end);
        assert_eq!(seek_backward(seek_backward(Duration::ZERO)), Duration::ZERO);
    }

    #[test]
    fn seek_roundtrip_returns_to_origin_in_the_middle() {
        let pos = Duration::from_secs(30);
        assert_eq!(seek_backward(seek_forward(pos, TOTAL)), pos);
    }

    #[test]
    fn seek_roundtrip_near_zero_is_lossy() {
        // Comportamiento intencionado: 2s -> -5 -> +5 = 5s, no 2s. El clamp en
        // cero pierde información. Se fija aquí para que nadie lo "arregle".
        let pos = Duration::from_secs(2);
        assert_eq!(
            seek_forward(seek_backward(pos), TOTAL),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn progress_fraction_spans_zero_to_one() {
        assert_eq!(progress_fraction(Duration::ZERO, TOTAL), 0.0);
        assert_eq!(progress_fraction(Duration::from_secs(30), TOTAL), 0.5);
        assert_eq!(progress_fraction(Duration::from_secs(60), TOTAL), 1.0);
        // Deriva del reloj más allá del final: se acota, no se pasa de 1.0.
        assert_eq!(progress_fraction(Duration::from_secs(99), TOTAL), 1.0);
        assert_eq!(progress_fraction(Duration::from_secs(30), None), 0.0);
    }

    #[test]
    fn position_from_fraction_clamps_out_of_range_input() {
        assert_eq!(position_from_fraction(0.5, TOTAL), Duration::from_secs(30));
        assert_eq!(position_from_fraction(-1.0, TOTAL), Duration::ZERO);
        assert_eq!(position_from_fraction(9.0, TOTAL), Duration::from_secs(60));
        assert_eq!(position_from_fraction(0.5, None), Duration::ZERO);
    }

    #[test]
    fn discard_before_seek_target_keeps_samples_when_no_seek_pending() {
        assert!(!discard_before_seek_target(Duration::from_secs(10), None));
    }

    #[test]
    fn discard_before_seek_target_drops_samples_before_the_target() {
        let pending = Some(Duration::from_secs(30));
        assert!(discard_before_seek_target(Duration::from_secs(29), pending));
        assert!(discard_before_seek_target(
            Duration::from_millis(1),
            pending
        ));
    }

    #[test]
    fn discard_before_seek_target_keeps_samples_from_the_target_on() {
        let pending = Some(Duration::from_secs(30));
        assert!(!discard_before_seek_target(
            Duration::from_secs(30),
            pending
        ));
        assert!(!discard_before_seek_target(
            Duration::from_secs(31),
            pending
        ));
        assert!(!discard_before_seek_target(
            Duration::from_secs(60),
            pending
        ));
    }

    #[test]
    fn discard_before_seek_target_with_zero_target_keeps_everything() {
        // Un seek a cero es un reinicio: ninguna muestra es anterior.
        assert!(!discard_before_seek_target(
            Duration::ZERO,
            Some(Duration::ZERO)
        ));
        assert!(!discard_before_seek_target(
            Duration::from_secs(1),
            Some(Duration::ZERO)
        ));
    }
}
