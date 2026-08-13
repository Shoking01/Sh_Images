//! Decisión de presentación de frames y política de descarte.
//!
//! El equivalente de `LoadedImage::frame_index_at` para video, pero con
//! búsqueda binaria: un GIF tiene como mucho 120 frames, un video de 2 h a
//! 30 fps tiene 216 000 y esto se consulta 30-60 veces por segundo.

use std::time::Duration;

/// Qué hacer con el frame que está a la cabeza del buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDecision {
    /// Su momento ya llegó: subirlo a la textura.
    Present,
    /// Todavía es pronto: esperar esto antes de repintar.
    Wait(Duration),
    /// Llegó demasiado tarde: descartarlo sin subirlo a la GPU.
    Drop,
}

/// Retraso a partir del cual un frame se descarta en vez de presentarse.
///
/// Presentar un frame muy tardío se ve peor que saltárselo, y además retrasa
/// todos los siguientes.
pub const LATE_THRESHOLD: Duration = Duration::from_millis(80);

/// Espera mínima entre repintados.
///
/// `request_repaint_after(ZERO)` quemaría un núcleo. Mismo criterio que
/// `image_loader::MIN_FRAME_DELAY`.
pub const MIN_FRAME_INTERVAL: Duration = Duration::from_millis(4);

/// Espera máxima entre comprobaciones, como red de seguridad si el reloj de
/// audio se detiene (dispositivo desconectado).
pub const MAX_FRAME_WAIT: Duration = Duration::from_millis(50);

/// Descartes seguidos tras los cuales conviene resincronizar en vez de seguir
/// tirando frames.
pub const MAX_CONSECUTIVE_DROPS: u32 = 10;

/// Decide qué hacer con un frame de marca temporal `pts` dado el reloj actual.
pub fn frame_decision(pts: Duration, clock: Duration, late_threshold: Duration) -> FrameDecision {
    if pts > clock {
        return FrameDecision::Wait(pts.saturating_sub(clock));
    }
    // pts <= clock: le toca. Sólo se descarta si va muy retrasado.
    if clock.saturating_sub(pts) > late_threshold {
        return FrameDecision::Drop;
    }
    FrameDecision::Present
}

/// Índice del último frame cuya marca temporal no supera el reloj.
///
/// `None` si el reloj va por delante del primer PTS o la lista está vacía.
/// Búsqueda binaria: es O(log n), no O(n).
pub fn frame_index_at(pts_list: &[Duration], clock: Duration) -> Option<usize> {
    if pts_list.is_empty() {
        return None;
    }
    // partition_point devuelve cuántos elementos cumplen `<= clock`.
    let count = pts_list.partition_point(|pts| *pts <= clock);
    if count == 0 {
        None
    } else {
        Some(count - 1)
    }
}

/// Cuánto esperar antes del próximo repintado.
///
/// Acotado por abajo a `MIN_FRAME_INTERVAL` y por arriba a `MAX_FRAME_WAIT`.
pub fn time_to_next_frame(
    pts_next: Option<Duration>,
    clock: Duration,
    fps_hint: Option<f32>,
) -> Duration {
    let raw = match pts_next {
        Some(pts) => pts.saturating_sub(clock),
        None => match fps_hint {
            Some(fps) if fps > 0.0 => Duration::from_secs_f32(1.0 / fps),
            _ => MAX_FRAME_WAIT,
        },
    };
    raw.clamp(MIN_FRAME_INTERVAL, MAX_FRAME_WAIT)
}

/// `true` si conviene resincronizar el reloj tras demasiados descartes.
pub fn should_resync(consecutive_drops: u32) -> bool {
    consecutive_drops >= MAX_CONSECUTIVE_DROPS
}

/// Actualiza el contador de descartes seguidos.
pub fn update_drop_counter(consecutive_drops: u32, decision: FrameDecision) -> u32 {
    match decision {
        FrameDecision::Drop => consecutive_drops.saturating_add(1),
        FrameDecision::Present => 0,
        FrameDecision::Wait(_) => consecutive_drops,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn frame_on_time_is_presented() {
        assert_eq!(
            frame_decision(ms(100), ms(105), LATE_THRESHOLD),
            FrameDecision::Present
        );
    }

    #[test]
    fn frame_exactly_at_clock_is_presented() {
        assert_eq!(
            frame_decision(ms(100), ms(100), LATE_THRESHOLD),
            FrameDecision::Present
        );
    }

    #[test]
    fn early_frame_returns_wait_with_remaining() {
        assert_eq!(
            frame_decision(ms(150), ms(100), LATE_THRESHOLD),
            FrameDecision::Wait(ms(50))
        );
    }

    #[test]
    fn late_frame_beyond_threshold_is_dropped() {
        assert_eq!(
            frame_decision(ms(100), ms(200), LATE_THRESHOLD),
            FrameDecision::Drop
        );
    }

    #[test]
    fn frame_exactly_at_late_threshold_is_presented() {
        // Umbral inclusivo: exactamente 80 ms tarde todavía se presenta.
        assert_eq!(
            frame_decision(ms(100), ms(180), LATE_THRESHOLD),
            FrameDecision::Present
        );
        assert_eq!(
            frame_decision(ms(100), ms(181), LATE_THRESHOLD),
            FrameDecision::Drop
        );
    }

    #[test]
    fn frame_index_at_selects_last_pts_not_after_clock() {
        let pts = [ms(0), ms(33), ms(66), ms(100)];
        assert_eq!(frame_index_at(&pts, ms(0)), Some(0));
        assert_eq!(frame_index_at(&pts, ms(50)), Some(1));
        assert_eq!(frame_index_at(&pts, ms(66)), Some(2));
        assert_eq!(frame_index_at(&pts, ms(99)), Some(2));
    }

    #[test]
    fn frame_index_at_before_first_pts_returns_none() {
        let pts = [ms(10), ms(20)];
        assert_eq!(frame_index_at(&pts, ms(5)), None);
    }

    #[test]
    fn frame_index_at_after_last_pts_returns_last() {
        let pts = [ms(0), ms(33), ms(66)];
        assert_eq!(frame_index_at(&pts, ms(10_000)), Some(2));
    }

    #[test]
    fn frame_index_at_empty_list_returns_none() {
        assert_eq!(frame_index_at(&[], ms(0)), None);
    }

    #[test]
    fn frame_index_at_non_monotonic_pts_does_not_panic() {
        // B-frames o mux defectuoso: partition_point da un resultado sin
        // sentido pero no puede hacer panic ni salirse del slice.
        let pts = [ms(0), ms(50), ms(20), ms(80)];
        let idx = frame_index_at(&pts, ms(40));
        if let Some(i) = idx {
            assert!(i < pts.len());
        }
    }

    #[test]
    fn frame_index_at_is_binary_search_on_ten_thousand_pts() {
        let pts: Vec<Duration> = (0..10_000).map(|i| ms(i * 33)).collect();
        assert_eq!(frame_index_at(&pts, ms(0)), Some(0));
        assert_eq!(frame_index_at(&pts, ms(33 * 5_000)), Some(5_000));
        assert_eq!(frame_index_at(&pts, ms(33 * 9_999)), Some(9_999));
        assert_eq!(frame_index_at(&pts, ms(u64::from(u32::MAX))), Some(9_999));
    }

    #[test]
    fn time_to_next_frame_never_returns_zero() {
        // request_repaint_after(ZERO) quemaría un núcleo.
        assert_eq!(
            time_to_next_frame(Some(ms(100)), ms(100), None),
            MIN_FRAME_INTERVAL
        );
        assert_eq!(
            time_to_next_frame(Some(ms(50)), ms(100), None),
            MIN_FRAME_INTERVAL
        );
    }

    #[test]
    fn time_to_next_frame_is_capped_for_a_stalled_clock() {
        assert_eq!(
            time_to_next_frame(Some(ms(10_000)), ms(0), None),
            MAX_FRAME_WAIT
        );
    }

    #[test]
    fn time_to_next_frame_returns_remaining_within_bounds() {
        assert_eq!(time_to_next_frame(Some(ms(120)), ms(100), None), ms(20));
    }

    #[test]
    fn time_to_next_frame_without_next_pts_uses_fps_hint() {
        // 30 fps -> ~33 ms, dentro de los límites.
        let wait = time_to_next_frame(None, ms(0), Some(30.0));
        assert!(
            wait >= ms(33) && wait <= ms(34),
            "esperaba ~33 ms, fue {wait:?}"
        );
    }

    #[test]
    fn time_to_next_frame_with_invalid_fps_hint_falls_back() {
        assert_eq!(time_to_next_frame(None, ms(0), Some(0.0)), MAX_FRAME_WAIT);
        assert_eq!(time_to_next_frame(None, ms(0), Some(-5.0)), MAX_FRAME_WAIT);
        assert_eq!(time_to_next_frame(None, ms(0), None), MAX_FRAME_WAIT);
        // 240 fps daría 4,1 ms: por debajo del mínimo, se acota.
        assert_eq!(
            time_to_next_frame(None, ms(0), Some(1000.0)),
            MIN_FRAME_INTERVAL
        );
    }

    #[test]
    fn should_resync_after_max_consecutive_drops() {
        assert!(!should_resync(0));
        assert!(!should_resync(MAX_CONSECUTIVE_DROPS - 1));
        assert!(should_resync(MAX_CONSECUTIVE_DROPS));
    }

    #[test]
    fn drop_counter_resets_on_presented_frame() {
        assert_eq!(update_drop_counter(0, FrameDecision::Drop), 1);
        assert_eq!(update_drop_counter(5, FrameDecision::Drop), 6);
        assert_eq!(update_drop_counter(5, FrameDecision::Present), 0);
        // Esperar no es ni acierto ni fallo: no toca el contador.
        assert_eq!(update_drop_counter(5, FrameDecision::Wait(ms(1))), 5);
        assert_eq!(update_drop_counter(u32::MAX, FrameDecision::Drop), u32::MAX);
    }
}
