//! Control de volumen.
//!
//! El volumen se almacena como `u8` de 0 a 100, no como `f32`: elimina la
//! deriva en coma flotante al subir y bajar por pasos, y serializa un entero
//! estable en `settings.toml`. La conversión a ganancia ocurre sólo en la
//! frontera con el audio.

/// Incremento de cada pulsación de subir/bajar volumen.
pub const VOLUME_STEP: u8 = 5;

/// Volumen inicial si no hay nada persistido.
pub const DEFAULT_VOLUME: u8 = 80;

/// Volumen máximo. No hay boost: sin ganancia >1.0 no hay clipping.
pub const MAX_VOLUME: u8 = 100;

/// Sube un paso, con techo en `MAX_VOLUME`.
pub fn volume_up(v: u8) -> u8 {
    v.saturating_add(VOLUME_STEP).min(MAX_VOLUME)
}

/// Baja un paso, con suelo en cero.
pub fn volume_down(v: u8) -> u8 {
    v.saturating_sub(VOLUME_STEP)
}

/// Sanea un valor que puede venir de un TOML editado a mano.
pub fn clamp_volume(v: u8) -> u8 {
    v.min(MAX_VOLUME)
}

/// Alterna el silencio.
///
/// Devuelve `(muted, volumen_recordado)`. Al silenciar guarda el volumen actual
/// para poder restaurarlo; al restaurar desde cero vuelve a `DEFAULT_VOLUME`,
/// porque si no el usuario quedaría mudo para siempre.
pub fn toggle_mute(muted: bool, current: u8, remembered: u8) -> (bool, u8) {
    if muted {
        let restored = if remembered == 0 {
            DEFAULT_VOLUME
        } else {
            clamp_volume(remembered)
        };
        (false, restored)
    } else {
        (true, clamp_volume(current))
    }
}

/// Ganancia lineal aplicable al PCM, en `[0.0, 1.0]`.
///
/// Curva cuadrática: la percepción de sonoridad es aproximadamente logarítmica,
/// así que con ganancia lineal todo el rango útil del control se concentraría
/// en su parte alta. A media escala da ~−12 dB.
pub fn effective_gain(v: u8, muted: bool) -> f32 {
    if muted {
        return 0.0;
    }
    let normalized = f32::from(clamp_volume(v)) / f32::from(MAX_VOLUME);
    normalized * normalized
}

/// Sube el volumen desmutando si hacía falta.
///
/// Subir el volumen estando en silencio debe oírse; si no, el control parece
/// roto.
pub fn volume_up_unmuting(v: u8, muted: bool, remembered: u8) -> (u8, bool) {
    if muted {
        let base = if remembered == 0 {
            0
        } else {
            clamp_volume(remembered)
        };
        (volume_up(base), false)
    } else {
        (volume_up(v), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_volume_is_eighty_percent() {
        assert_eq!(DEFAULT_VOLUME, 80);
        assert_eq!(VOLUME_STEP, 5);
    }

    #[test]
    fn volume_up_increases_by_one_step() {
        assert_eq!(volume_up(50), 55);
    }

    #[test]
    fn volume_down_decreases_by_one_step() {
        assert_eq!(volume_down(50), 45);
    }

    #[test]
    fn volume_up_clamps_at_max() {
        assert_eq!(volume_up(100), 100);
        assert_eq!(volume_up(255), 100);
    }

    #[test]
    fn volume_down_clamps_at_min() {
        assert_eq!(volume_down(0), 0);
        assert_eq!(volume_down(3), 0);
    }

    #[test]
    fn volume_up_from_just_below_max_lands_exactly_on_max() {
        assert_eq!(volume_up(98), 100);
        assert_eq!(volume_up(95), 100);
    }

    #[test]
    fn volume_down_from_just_above_min_lands_exactly_on_min() {
        assert_eq!(volume_down(2), 0);
        assert_eq!(volume_down(5), 0);
    }

    #[test]
    fn twenty_ups_then_twenty_downs_returns_to_zero() {
        // El motivo de usar u8 y no f32: esto es exacto, sin epsilon.
        let mut v = 0u8;
        for _ in 0..20 {
            v = volume_up(v);
        }
        assert_eq!(v, 100);
        for _ in 0..20 {
            v = volume_down(v);
        }
        assert_eq!(v, 0);
    }

    #[test]
    fn clamp_volume_saturates_out_of_range_input() {
        assert_eq!(clamp_volume(250), 100);
        assert_eq!(clamp_volume(100), 100);
        assert_eq!(clamp_volume(0), 0);
    }

    #[test]
    fn mute_sets_gain_to_zero_without_losing_volume() {
        let (muted, remembered) = toggle_mute(false, 70, 0);
        assert!(muted);
        assert_eq!(remembered, 70);
        assert_eq!(effective_gain(70, muted), 0.0);
    }

    #[test]
    fn unmute_restores_previous_volume() {
        let (muted, remembered) = toggle_mute(false, 35, 0);
        let (muted, restored) = toggle_mute(muted, 35, remembered);
        assert!(!muted);
        assert_eq!(restored, 35);
    }

    #[test]
    fn unmute_from_zero_volume_restores_default() {
        // Si no, silenciar a volumen 0 dejaría al usuario mudo para siempre.
        let (muted, restored) = toggle_mute(true, 0, 0);
        assert!(!muted);
        assert_eq!(restored, DEFAULT_VOLUME);
    }

    #[test]
    fn volume_up_while_muted_unmutes() {
        let (v, muted) = volume_up_unmuting(0, true, 40);
        assert!(!muted);
        assert_eq!(v, 45);
    }

    #[test]
    fn volume_up_while_not_muted_keeps_unmuted() {
        let (v, muted) = volume_up_unmuting(40, false, 0);
        assert!(!muted);
        assert_eq!(v, 45);
    }

    #[test]
    fn effective_gain_is_perceptual_square_curve() {
        // A media escala, bastante por debajo de la mitad de la ganancia lineal.
        let half = effective_gain(50, false);
        assert!(
            (half - 0.25).abs() < 1e-6,
            "ganancia a media escala fue {half}"
        );
        assert!(half < 0.5, "la curva debe estar por debajo de la lineal");
    }

    #[test]
    fn effective_gain_endpoints_are_exact() {
        assert_eq!(effective_gain(0, false), 0.0);
        assert_eq!(effective_gain(100, false), 1.0);
    }

    #[test]
    fn effective_gain_is_monotonic_over_all_steps() {
        let mut prev = -1.0f32;
        for v in 0..=100u8 {
            let g = effective_gain(v, false);
            assert!(g > prev, "ganancia no monótona en v={v}: {g} <= {prev}");
            assert!((0.0..=1.0).contains(&g), "ganancia fuera de rango en v={v}");
            prev = g;
        }
    }

    #[test]
    fn effective_gain_is_zero_when_muted_regardless_of_volume() {
        for v in [0u8, 1, 50, 100, 255] {
            assert_eq!(effective_gain(v, true), 0.0, "v={v}");
        }
    }

    #[test]
    fn effective_gain_saturates_out_of_range_volume() {
        assert_eq!(effective_gain(200, false), 1.0);
    }
}
