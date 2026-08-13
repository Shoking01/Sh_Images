//! Formateo de tiempos de reproducción.

use std::time::Duration;

/// Marcador de duración desconocida, en el estilo de `Translations::no_size`.
pub const UNKNOWN_TIME: &str = "--:--";

/// Formatea una duración como `MM:SS`, o `H:MM:SS` a partir de una hora.
///
/// Trunca los subsegundos, no redondea: mostrar `00:01` cuando aún no ha pasado
/// el primer segundo adelantaría el contador respecto al video.
pub fn format_timestamp(d: Duration) -> String {
    let total = d.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Formatea `posición / duración` para la barra de controles.
///
/// La posición se acota a la duración: la deriva del reloj puede pasarse unos
/// milisegundos y mostrar `45:01 / 45:00` parecería un fallo.
pub fn format_position(position: Duration, duration: Option<Duration>) -> String {
    match duration {
        Some(total) => format!(
            "{} / {}",
            format_timestamp(position.min(total)),
            format_timestamp(total)
        ),
        None => format!("{} / {UNKNOWN_TIME}", format_timestamp(position)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn format_zero_is_double_zero() {
        assert_eq!(format_timestamp(Duration::ZERO), "00:00");
    }

    #[test]
    fn format_under_a_minute() {
        assert_eq!(format_timestamp(secs(5)), "00:05");
        assert_eq!(format_timestamp(secs(59)), "00:59");
    }

    #[test]
    fn format_exactly_one_minute() {
        assert_eq!(format_timestamp(secs(60)), "01:00");
    }

    #[test]
    fn format_fifty_nine_fifty_nine() {
        assert_eq!(format_timestamp(secs(3599)), "59:59");
    }

    #[test]
    fn format_one_hour_switches_to_h_mm_ss() {
        // Sin esto, un video de 2 h mostraría "120:00".
        assert_eq!(format_timestamp(secs(3600)), "1:00:00");
        assert_eq!(format_timestamp(secs(3661)), "1:01:01");
    }

    #[test]
    fn format_two_hours() {
        assert_eq!(format_timestamp(secs(7200)), "2:00:00");
        assert_eq!(format_timestamp(secs(7325)), "2:02:05");
    }

    #[test]
    fn format_truncates_subsecond_does_not_round() {
        assert_eq!(format_timestamp(Duration::from_millis(999)), "00:00");
        assert_eq!(format_timestamp(Duration::from_millis(1999)), "00:01");
        assert_eq!(format_timestamp(Duration::from_millis(59_999)), "00:59");
    }

    #[test]
    fn format_max_duration_does_not_panic() {
        let _ = format_timestamp(Duration::MAX);
    }

    #[test]
    fn format_position_shows_slash_separator() {
        assert_eq!(
            format_position(secs(754), Some(secs(2700))),
            "12:34 / 45:00"
        );
    }

    #[test]
    fn format_position_with_unknown_duration_shows_dashes() {
        assert_eq!(format_position(secs(10), None), "00:10 / --:--");
    }

    #[test]
    fn format_position_clamps_position_over_duration() {
        // Deriva del reloj: nunca mostrar una posición mayor que la duración.
        assert_eq!(format_position(secs(61), Some(secs(60))), "01:00 / 01:00");
    }

    #[test]
    fn format_position_mixes_hour_and_minute_forms() {
        assert_eq!(
            format_position(secs(30), Some(secs(7200))),
            "00:30 / 2:00:00"
        );
    }

    #[test]
    fn snapshot_video_time_formats() {
        let samples = [
            Duration::ZERO,
            Duration::from_millis(999),
            secs(5),
            secs(59),
            secs(60),
            secs(3599),
            secs(3600),
            secs(7325),
        ];
        let mut out = String::new();
        for d in samples {
            out.push_str(&format!("{:>10?} -> {}\n", d, format_timestamp(d)));
        }
        out.push('\n');
        out.push_str(&format_position(secs(754), Some(secs(2700))));
        out.push('\n');
        out.push_str(&format_position(secs(10), None));
        out.push('\n');
        insta::assert_snapshot!(out);
    }
}
