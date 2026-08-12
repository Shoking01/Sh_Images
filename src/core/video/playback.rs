//! Máquina de estados de reproducción.
//!
//! Funciones puras y totales: toda combinación (estado, evento) tiene una
//! transición definida. El backend no decide nada; sólo emite eventos.

use std::time::Duration;

/// Estado de reproducción de un video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlaybackState {
    /// Cargado pero sin arrancar, o detenido tras cerrar el archivo.
    Stopped,
    Playing,
    Paused,
    /// Llegó al final. Conserva la posición en `duration`.
    Ended,
}

/// Evento que puede cambiar el estado de reproducción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackEvent {
    Play,
    Pause,
    TogglePlayPause,
    Stop,
    Seek,
    ReachedEnd,
    Loaded { autoplay: bool },
}

impl PlaybackState {
    /// `true` si el reloj de reproducción debe avanzar.
    pub fn is_running(self) -> bool {
        self == PlaybackState::Playing
    }
}

/// Transición de estado. Total: definida para las 4×7 combinaciones.
pub fn next_state(state: PlaybackState, event: PlaybackEvent) -> PlaybackState {
    use PlaybackEvent as E;
    use PlaybackState as S;
    match (state, event) {
        (_, E::Stop) => S::Stopped,
        (_, E::Loaded { autoplay: true }) => S::Playing,
        (_, E::Loaded { autoplay: false }) => S::Stopped,

        (_, E::Play) => S::Playing,

        // Pausar algo que no está sonando es un no-op. Si `Ended` degradase a
        // `Paused`, se perdería el reinicio desde cero y el siguiente play no
        // haría nada visible: el video ya está al final.
        (S::Stopped, E::Pause) => S::Stopped,
        (S::Ended, E::Pause) => S::Ended,
        (_, E::Pause) => S::Paused,

        (S::Stopped, E::TogglePlayPause) => S::Playing,
        (S::Playing, E::TogglePlayPause) => S::Paused,
        (S::Paused, E::TogglePlayPause) => S::Playing,
        (S::Ended, E::TogglePlayPause) => S::Playing,

        // Buscar desde el final deja el video listo pero en pausa: no debe
        // re-arrancar el audio solo por retroceder 5 s.
        (S::Ended, E::Seek) => S::Paused,
        (S::Stopped, E::Seek) => S::Stopped,
        (S::Playing, E::Seek) => S::Playing,
        (S::Paused, E::Seek) => S::Paused,

        (S::Playing, E::ReachedEnd) => S::Ended,
        (S::Ended, E::ReachedEnd) => S::Ended,
        (S::Stopped, E::ReachedEnd) => S::Stopped,
        (S::Paused, E::ReachedEnd) => S::Paused,
    }
}

/// `true` si la transición implica reiniciar la posición a cero.
///
/// Sólo ocurre al arrancar desde `Ended`: sin esto, pulsar play al final no
/// haría nada visible.
pub fn should_restart_from_zero(state: PlaybackState, event: PlaybackEvent) -> bool {
    matches!(
        (state, event),
        (
            PlaybackState::Ended,
            PlaybackEvent::Play | PlaybackEvent::TogglePlayPause
        )
    )
}

/// Avanza la posición y detecta el final.
///
/// Devuelve `(nueva_posición, llegó_al_final)`. Con duración desconocida nunca
/// declara el final: sólo el backend sabe cuándo se acabaron las muestras.
pub fn advance(
    position: Duration,
    delta: Duration,
    duration: Option<Duration>,
) -> (Duration, bool) {
    let next = position.saturating_add(delta);
    match duration {
        Some(total) if next >= total => (total, true),
        _ => (next, false),
    }
}

#[cfg(test)]
mod tests {
    use super::PlaybackEvent as E;
    use super::PlaybackState as S;
    use super::*;

    const STATES: [S; 4] = [S::Stopped, S::Playing, S::Paused, S::Ended];
    const EVENTS: [E; 7] = [
        E::Play,
        E::Pause,
        E::TogglePlayPause,
        E::Stop,
        E::Seek,
        E::ReachedEnd,
        E::Loaded { autoplay: true },
    ];

    #[test]
    fn stopped_play_transitions_to_playing() {
        assert_eq!(next_state(S::Stopped, E::Play), S::Playing);
    }

    #[test]
    fn playing_pause_transitions_to_paused() {
        assert_eq!(next_state(S::Playing, E::Pause), S::Paused);
    }

    #[test]
    fn paused_play_resumes_playing() {
        assert_eq!(next_state(S::Paused, E::Play), S::Playing);
    }

    #[test]
    fn toggle_alternates_playing_and_paused() {
        let s = next_state(S::Playing, E::TogglePlayPause);
        assert_eq!(s, S::Paused);
        assert_eq!(next_state(s, E::TogglePlayPause), S::Playing);
    }

    #[test]
    fn toggle_from_stopped_starts_playing() {
        assert_eq!(next_state(S::Stopped, E::TogglePlayPause), S::Playing);
    }

    #[test]
    fn toggle_from_ended_restarts_playing() {
        assert_eq!(next_state(S::Ended, E::TogglePlayPause), S::Playing);
        assert!(should_restart_from_zero(S::Ended, E::TogglePlayPause));
    }

    #[test]
    fn playing_reached_end_transitions_to_ended() {
        assert_eq!(next_state(S::Playing, E::ReachedEnd), S::Ended);
    }

    #[test]
    fn ended_play_restarts_from_zero() {
        assert_eq!(next_state(S::Ended, E::Play), S::Playing);
        assert!(should_restart_from_zero(S::Ended, E::Play));
    }

    #[test]
    fn should_restart_from_zero_only_from_ended() {
        for s in STATES {
            for e in EVENTS {
                let expected = s == S::Ended && matches!(e, E::Play | E::TogglePlayPause);
                assert_eq!(
                    should_restart_from_zero(s, e),
                    expected,
                    "({s:?}, {e:?}) no debería reiniciar la posición"
                );
            }
        }
    }

    #[test]
    fn stop_from_any_state_returns_stopped() {
        for s in STATES {
            assert_eq!(next_state(s, E::Stop), S::Stopped, "desde {s:?}");
        }
    }

    #[test]
    fn seek_from_ended_returns_paused() {
        assert_eq!(next_state(S::Ended, E::Seek), S::Paused);
        assert!(!should_restart_from_zero(S::Ended, E::Seek));
    }

    #[test]
    fn seek_preserves_playing_and_paused() {
        assert_eq!(next_state(S::Playing, E::Seek), S::Playing);
        assert_eq!(next_state(S::Paused, E::Seek), S::Paused);
        assert_eq!(next_state(S::Stopped, E::Seek), S::Stopped);
    }

    #[test]
    fn reached_end_is_idempotent() {
        let once = next_state(S::Playing, E::ReachedEnd);
        assert_eq!(next_state(once, E::ReachedEnd), once);
    }

    #[test]
    fn pause_from_stopped_stays_stopped() {
        assert_eq!(next_state(S::Stopped, E::Pause), S::Stopped);
    }

    #[test]
    fn pause_from_ended_stays_ended() {
        // Degradar a Paused perdería el reinicio desde cero: el usuario
        // pulsaría play al final del video y no pasaría nada.
        assert_eq!(next_state(S::Ended, E::Pause), S::Ended);
        assert!(should_restart_from_zero(
            next_state(S::Ended, E::Pause),
            E::Play
        ));
    }

    #[test]
    fn loaded_with_autoplay_true_starts_playing() {
        for s in STATES {
            assert_eq!(next_state(s, E::Loaded { autoplay: true }), S::Playing);
        }
    }

    #[test]
    fn loaded_with_autoplay_false_stays_stopped() {
        for s in STATES {
            assert_eq!(next_state(s, E::Loaded { autoplay: false }), S::Stopped);
        }
    }

    #[test]
    fn is_running_only_while_playing() {
        assert!(S::Playing.is_running());
        assert!(!S::Paused.is_running());
        assert!(!S::Stopped.is_running());
        assert!(!S::Ended.is_running());
    }

    #[test]
    fn every_state_event_pair_is_total() {
        // No asserta un valor concreto: verifica que ninguna combinación entra
        // en un brazo inexistente (haría panic por match no exhaustivo).
        let mut seen = 0;
        for s in STATES {
            for e in EVENTS {
                let _ = next_state(s, e);
                seen += 1;
            }
        }
        assert_eq!(seen, 28);
    }

    #[test]
    fn advance_accumulates_position() {
        let (pos, ended) = advance(
            Duration::from_secs(3),
            Duration::from_millis(500),
            Some(Duration::from_secs(10)),
        );
        assert_eq!(pos, Duration::from_millis(3500));
        assert!(!ended);
    }

    #[test]
    fn advance_clamps_at_duration_and_reports_end() {
        let (pos, ended) = advance(
            Duration::from_secs(9),
            Duration::from_secs(5),
            Some(Duration::from_secs(10)),
        );
        assert_eq!(pos, Duration::from_secs(10));
        assert!(ended);
    }

    #[test]
    fn advance_with_unknown_duration_never_reports_end() {
        let (pos, ended) = advance(Duration::from_secs(9), Duration::from_secs(5), None);
        assert_eq!(pos, Duration::from_secs(14));
        assert!(!ended);
    }

    #[test]
    fn advance_with_zero_duration_reports_end_immediately() {
        let (pos, ended) = advance(Duration::ZERO, Duration::ZERO, Some(Duration::ZERO));
        assert_eq!(pos, Duration::ZERO);
        assert!(ended);
    }

    #[test]
    fn advance_saturates_without_overflow() {
        let (pos, _) = advance(Duration::MAX, Duration::from_secs(1), None);
        assert_eq!(pos, Duration::MAX);
    }

    #[test]
    fn snapshot_playback_transition_table() {
        let mut out = String::new();
        for s in STATES {
            for e in EVENTS {
                let restart = if should_restart_from_zero(s, e) {
                    " (pos=0)"
                } else {
                    ""
                };
                out.push_str(&format!(
                    "{s:?} + {e:?} -> {:?}{restart}\n",
                    next_state(s, e)
                ));
            }
        }
        insta::assert_snapshot!(out);
    }
}
