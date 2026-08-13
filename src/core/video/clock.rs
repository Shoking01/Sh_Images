//! Reloj de reproducción.
//!
//! Sigue la convención de `core::slideshow`: el instante actual entra como
//! argumento, nunca se llama a `Instant::now()` aquí dentro. Eso es lo que hace
//! el reloj testeable de forma determinista.

use std::time::{Duration, Instant};

/// Reloj monótono de reproducción, usado cuando no hay pista de audio.
///
/// Cuando sí la hay, el reloj maestro es el de audio (`AudioClockState`): el
/// dispositivo no corre exactamente a la frecuencia nominal y derivar el video
/// de `Instant` acumularía desincronía.
#[derive(Debug, Clone, Copy)]
pub struct PlayClock {
    /// Posición acumulada de todos los tramos ya reproducidos.
    base: Duration,
    /// Instante en que arrancó el tramo actual. `None` = pausado.
    started: Option<Instant>,
}

impl PlayClock {
    /// Reloj detenido en cero.
    pub fn new() -> Self {
        Self {
            base: Duration::ZERO,
            started: None,
        }
    }

    /// Reloj detenido en una posición concreta.
    pub fn at(position: Duration) -> Self {
        Self {
            base: position,
            started: None,
        }
    }

    /// Posición actual.
    pub fn now(&self, instant: Instant) -> Duration {
        match self.started {
            Some(start) => self.base + instant.saturating_duration_since(start),
            None => self.base,
        }
    }

    /// `true` si el reloj está avanzando.
    pub fn is_running(&self) -> bool {
        self.started.is_some()
    }

    /// Arranca o reanuda. Idempotente: llamarlo dos veces no adelanta el reloj.
    pub fn start(&mut self, instant: Instant) {
        if self.started.is_none() {
            self.started = Some(instant);
        }
    }

    /// Pausa, consolidando lo transcurrido en `base`.
    pub fn pause(&mut self, instant: Instant) {
        if let Some(start) = self.started.take() {
            self.base += instant.saturating_duration_since(start);
        }
    }

    /// Reposiciona sin cambiar si está corriendo o pausado.
    pub fn seek_to(&mut self, position: Duration, instant: Instant) {
        self.base = position;
        if self.started.is_some() {
            self.started = Some(instant);
        }
    }

    /// Vuelve a cero y detiene.
    pub fn reset(&mut self) {
        self.base = Duration::ZERO;
        self.started = None;
    }
}

impl Default for PlayClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Posición derivada del audio ya entregado al dispositivo.
///
/// `frames_played` lo escribe el callback de audio; `latency` es lo que el
/// dispositivo tiene en cola y todavía no ha sonado, así que se resta.
pub fn audio_position(
    base: Duration,
    frames_played: u64,
    sample_rate: u32,
    latency: Duration,
) -> Duration {
    if sample_rate == 0 {
        return base;
    }
    let played = Duration::from_secs_f64(frames_played as f64 / f64::from(sample_rate));
    base + played.saturating_sub(latency)
}

/// Desfase entre el video y el reloj maestro, en milisegundos con signo.
///
/// Positivo = el video va por delante del audio.
pub fn drift_ms(video_pts: Duration, master_clock: Duration) -> i64 {
    let v = video_pts.as_millis() as i64;
    let m = master_clock.as_millis() as i64;
    v.saturating_sub(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_instant() -> Instant {
        Instant::now()
    }

    #[test]
    fn new_clock_starts_at_zero_and_paused() {
        let c = PlayClock::new();
        assert_eq!(c.now(base_instant()), Duration::ZERO);
        assert!(!c.is_running());
    }

    #[test]
    fn at_starts_paused_at_position() {
        let c = PlayClock::at(Duration::from_secs(30));
        assert_eq!(c.now(base_instant()), Duration::from_secs(30));
        assert!(!c.is_running());
    }

    #[test]
    fn running_clock_advances_with_the_instant() {
        let t0 = base_instant();
        let mut c = PlayClock::new();
        c.start(t0);
        assert!(c.is_running());
        assert_eq!(c.now(t0 + Duration::from_secs(3)), Duration::from_secs(3));
    }

    #[test]
    fn paused_clock_freezes_the_position() {
        let t0 = base_instant();
        let mut c = PlayClock::new();
        c.start(t0);
        c.pause(t0 + Duration::from_secs(2));
        // El tiempo sigue corriendo fuera, pero el reloj no.
        assert_eq!(c.now(t0 + Duration::from_secs(9)), Duration::from_secs(2));
        assert!(!c.is_running());
    }

    #[test]
    fn resume_continues_from_where_it_paused() {
        let t0 = base_instant();
        let mut c = PlayClock::new();
        c.start(t0);
        c.pause(t0 + Duration::from_secs(2));
        c.start(t0 + Duration::from_secs(10));
        assert_eq!(c.now(t0 + Duration::from_secs(13)), Duration::from_secs(5));
    }

    #[test]
    fn start_is_idempotent() {
        let t0 = base_instant();
        let mut c = PlayClock::new();
        c.start(t0);
        c.start(t0 + Duration::from_secs(5)); // no debe reiniciar el tramo
        assert_eq!(c.now(t0 + Duration::from_secs(6)), Duration::from_secs(6));
    }

    #[test]
    fn pause_twice_does_not_double_count() {
        let t0 = base_instant();
        let mut c = PlayClock::new();
        c.start(t0);
        c.pause(t0 + Duration::from_secs(2));
        c.pause(t0 + Duration::from_secs(8));
        assert_eq!(c.now(t0 + Duration::from_secs(20)), Duration::from_secs(2));
    }

    #[test]
    fn seek_while_paused_keeps_it_paused() {
        let t0 = base_instant();
        let mut c = PlayClock::at(Duration::from_secs(10));
        c.seek_to(Duration::from_secs(40), t0);
        assert!(!c.is_running());
        assert_eq!(c.now(t0 + Duration::from_secs(5)), Duration::from_secs(40));
    }

    #[test]
    fn seek_while_playing_keeps_it_playing_from_the_target() {
        let t0 = base_instant();
        let mut c = PlayClock::new();
        c.start(t0);
        c.seek_to(Duration::from_secs(40), t0 + Duration::from_secs(3));
        assert!(c.is_running());
        assert_eq!(c.now(t0 + Duration::from_secs(5)), Duration::from_secs(42));
    }

    #[test]
    fn reset_returns_to_zero_and_pauses() {
        let t0 = base_instant();
        let mut c = PlayClock::at(Duration::from_secs(30));
        c.start(t0);
        c.reset();
        assert!(!c.is_running());
        assert_eq!(c.now(t0 + Duration::from_secs(9)), Duration::ZERO);
    }

    #[test]
    fn now_with_an_earlier_instant_does_not_panic() {
        // saturating_duration_since evita el panic si llega un instante viejo.
        let t0 = base_instant() + Duration::from_secs(10);
        let mut c = PlayClock::new();
        c.start(t0);
        assert_eq!(c.now(t0 - Duration::from_secs(5)), Duration::ZERO);
    }

    #[test]
    fn default_matches_new() {
        let t = base_instant();
        assert_eq!(PlayClock::default().now(t), PlayClock::new().now(t));
    }

    #[test]
    fn audio_position_subtracts_device_latency() {
        // 48 000 frames a 48 kHz = 1 s reproducido, menos 100 ms en cola.
        let pos = audio_position(Duration::ZERO, 48_000, 48_000, Duration::from_millis(100));
        assert_eq!(pos, Duration::from_millis(900));
    }

    #[test]
    fn audio_position_adds_the_seek_base() {
        let pos = audio_position(Duration::from_secs(30), 48_000, 48_000, Duration::ZERO);
        assert_eq!(pos, Duration::from_secs(31));
    }

    #[test]
    fn audio_position_with_latency_over_played_clamps_to_base() {
        let pos = audio_position(
            Duration::from_secs(5),
            100,
            48_000,
            Duration::from_millis(500),
        );
        assert_eq!(pos, Duration::from_secs(5));
    }

    #[test]
    fn audio_position_with_zero_sample_rate_returns_base() {
        // Dispositivo no inicializado: no dividir por cero.
        let base = Duration::from_secs(7);
        assert_eq!(audio_position(base, 1000, 0, Duration::ZERO), base);
    }

    #[test]
    fn drift_is_signed_and_saturating() {
        assert_eq!(
            drift_ms(Duration::from_millis(1040), Duration::from_millis(1000)),
            40
        );
        assert_eq!(
            drift_ms(Duration::from_millis(960), Duration::from_millis(1000)),
            -40
        );
        assert_eq!(drift_ms(Duration::ZERO, Duration::ZERO), 0);
        let _ = drift_ms(Duration::MAX, Duration::ZERO);
    }
}
