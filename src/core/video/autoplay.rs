//! Política de reproducción automática y su interacción con el pase de
//! diapositivas.

use std::time::Duration;

use super::playback::PlaybackState;

/// Qué hacer al abrir o navegar a un video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoplayDecision {
    Play,
    StayPaused,
}

impl AutoplayDecision {
    pub fn should_play(self) -> bool {
        self == AutoplayDecision::Play
    }
}

/// Decisión al abrir un video desde el diálogo, la CLI o el sidebar.
///
/// Con el pase activo el video siempre arranca: si no, el pase se quedaría
/// esperando indefinidamente a que terminase un video parado.
pub fn autoplay_on_open(
    setting_enabled: bool,
    has_video_track: bool,
    slideshow_active: bool,
) -> AutoplayDecision {
    if !has_video_track {
        return AutoplayDecision::StayPaused;
    }
    if slideshow_active || setting_enabled {
        AutoplayDecision::Play
    } else {
        AutoplayDecision::StayPaused
    }
}

/// Decisión al navegar de un archivo a otro con las flechas.
///
/// Si el usuario venía reproduciendo, el siguiente video sigue reproduciendo
/// aunque el autoplay esté desactivado: ya expresó su intención.
pub fn autoplay_on_navigate(
    setting_enabled: bool,
    was_playing: bool,
    slideshow_active: bool,
) -> AutoplayDecision {
    if slideshow_active || was_playing || setting_enabled {
        AutoplayDecision::Play
    } else {
        AutoplayDecision::StayPaused
    }
}

/// `true` si el pase de diapositivas debe avanzar al siguiente archivo.
///
/// Con `wait_for_video_end`, un video en reproducción retiene el pase hasta que
/// termina; si no, un clip de 3 minutos se saltaría a los 5 segundos. Las
/// imágenes (y los videos pausados por el usuario) siguen el intervalo normal.
pub fn slideshow_should_advance(
    video: Option<(PlaybackState, Duration)>,
    elapsed: Duration,
    interval: Duration,
    wait_for_video_end: bool,
) -> bool {
    match video {
        Some((PlaybackState::Ended, _)) => true,
        Some((PlaybackState::Playing, _)) if wait_for_video_end => false,
        _ => crate::core::slideshow::elapsed_reached(elapsed, interval),
    }
}

/// `true` si hay que detener el reproductor actual antes de abrir otro archivo.
///
/// Siempre que haya un reproductor vivo. Solaparlos haría sonar dos pistas de
/// audio a la vez durante el arranque del segundo.
pub fn should_stop_before_opening(has_active_player: bool) -> bool {
    has_active_player
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTERVAL: Duration = Duration::from_secs(5);

    #[test]
    fn autoplay_enabled_plays_on_open() {
        assert_eq!(autoplay_on_open(true, true, false), AutoplayDecision::Play);
    }

    #[test]
    fn autoplay_disabled_stays_paused_on_open() {
        assert_eq!(
            autoplay_on_open(false, true, false),
            AutoplayDecision::StayPaused
        );
    }

    #[test]
    fn file_without_video_track_never_autoplays() {
        assert_eq!(
            autoplay_on_open(true, false, true),
            AutoplayDecision::StayPaused
        );
    }

    #[test]
    fn slideshow_forces_autoplay_even_if_setting_is_off() {
        // Si no, el pase se quedaría bloqueado esperando un video parado.
        assert_eq!(autoplay_on_open(false, true, true), AutoplayDecision::Play);
    }

    #[test]
    fn autoplay_carries_playing_state_across_navigation() {
        assert_eq!(
            autoplay_on_navigate(false, true, false),
            AutoplayDecision::Play
        );
    }

    #[test]
    fn navigating_while_paused_with_autoplay_off_stays_paused() {
        assert_eq!(
            autoplay_on_navigate(false, false, false),
            AutoplayDecision::StayPaused
        );
    }

    #[test]
    fn navigating_with_autoplay_on_plays() {
        assert_eq!(
            autoplay_on_navigate(true, false, false),
            AutoplayDecision::Play
        );
    }

    #[test]
    fn navigating_away_stops_previous_video() {
        assert!(should_stop_before_opening(true));
        assert!(!should_stop_before_opening(false));
    }

    #[test]
    fn should_play_reflects_the_decision() {
        assert!(AutoplayDecision::Play.should_play());
        assert!(!AutoplayDecision::StayPaused.should_play());
    }

    #[test]
    fn slideshow_does_not_advance_while_video_plays() {
        let long = Duration::from_secs(180);
        assert!(!slideshow_should_advance(
            Some((PlaybackState::Playing, long)),
            Duration::from_secs(60),
            INTERVAL,
            true
        ));
    }

    #[test]
    fn slideshow_advances_immediately_when_video_ends() {
        assert!(slideshow_should_advance(
            Some((PlaybackState::Ended, Duration::ZERO)),
            Duration::ZERO,
            INTERVAL,
            true
        ));
    }

    #[test]
    fn slideshow_advances_on_interval_for_images() {
        // No-regresión del flujo 9: sin video, la política es la de siempre.
        assert!(slideshow_should_advance(
            None,
            Duration::from_secs(5),
            INTERVAL,
            true
        ));
        assert!(!slideshow_should_advance(
            None,
            Duration::from_secs(4),
            INTERVAL,
            true
        ));
    }

    #[test]
    fn slideshow_advances_when_video_is_paused_by_user() {
        // Un video pausado no debe bloquear el pase para siempre.
        assert!(slideshow_should_advance(
            Some((PlaybackState::Paused, Duration::from_secs(90))),
            Duration::from_secs(5),
            INTERVAL,
            true
        ));
    }

    #[test]
    fn slideshow_ignores_video_when_not_waiting_for_end() {
        assert!(slideshow_should_advance(
            Some((PlaybackState::Playing, Duration::from_secs(180))),
            Duration::from_secs(5),
            INTERVAL,
            false
        ));
    }
}
