//! Salida de audio y rampa de volumen.
//!
//! El búfer y la rampa son puros y se testean en las tres plataformas; sólo la
//! apertura del dispositivo depende de `cpal`.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

/// Duración de la rampa al cambiar el volumen o tras un salto.
///
/// Sin ella, cada pulsación de subir/bajar produce un chasquido audible y cada
/// seek un golpe seco.
pub const RAMP_SECONDS: f32 = 0.015;

/// Segundos de audio que se mantienen en cola como colchón.
///
/// Subido de 0.5 a 1.5 s: con 0.5 s, un hipo del consumidor de frames de
/// video (repintado de la UI) que superase ese margen vaciaba el buffer
/// antes de que el decodificador se pusiera al día, y sonaba como un
/// petardeo. 1.5 s da mucho más margen sin notarse en la latencia de
/// arranque ni en los saltos (el buffer se vacía con `AudioBuffer::flush`).
pub const AUDIO_BUFFER_SECONDS: f32 = 1.5;

/// Coeficiente del filtro de un polo que suaviza el cambio de ganancia.
pub fn ramp_coefficient(sample_rate: u32) -> f32 {
    if sample_rate == 0 {
        return 1.0;
    }
    1.0 - (-1.0 / (RAMP_SECONDS * sample_rate as f32)).exp()
}

/// Aplica la rampa a un bloque de muestras y devuelve la ganancia final.
///
/// Se llama desde el callback de audio, así que no asigna ni bloquea.
pub fn apply_gain_ramp(buffer: &mut [f32], mut gain: f32, target: f32, k: f32) -> f32 {
    for sample in buffer.iter_mut() {
        gain += (target - gain) * k;
        *sample *= gain;
    }
    gain
}

/// Muestras que caben en el colchón para una frecuencia y número de canales.
pub fn buffer_capacity(sample_rate: u32, channels: u32) -> usize {
    ((sample_rate as f32 * AUDIO_BUFFER_SECONDS) as usize).saturating_mul(channels.max(1) as usize)
}

/// Cola de PCM entre el decodificador y el callback del dispositivo.
///
/// El callback usa `try_lock` y, si no lo consigue, emite silencio: bloquear en
/// el hilo de audio provocaría un corte garantizado.
#[derive(Debug)]
pub struct AudioBuffer {
    samples: Mutex<VecDeque<f32>>,
    capacity: usize,
    /// Ganancia objetivo, en bits de `f32`. Atómico porque lo lee el callback.
    target_gain: AtomicU32,
    /// Muestras (por canal) ya entregadas al dispositivo: es el reloj maestro.
    frames_played: AtomicU64,
    /// Se incrementa en cada salto; el callback hace fundido y descarta.
    epoch: AtomicU64,
    starved: AtomicBool,
}

impl AudioBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity: capacity.max(1),
            target_gain: AtomicU32::new(1.0f32.to_bits()),
            frames_played: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            starved: AtomicBool::new(false),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<f32>> {
        self.samples.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn set_target_gain(&self, gain: f32) {
        self.target_gain
            .store(gain.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn target_gain(&self) -> f32 {
        f32::from_bits(self.target_gain.load(Ordering::Relaxed))
    }

    pub fn frames_played(&self) -> u64 {
        self.frames_played.load(Ordering::Relaxed)
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    /// Vacía la cola tras un salto y reinicia el reloj.
    pub fn flush(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        self.lock().clear();
        self.frames_played.store(0, Ordering::Relaxed);
    }

    /// `true` si el callback se quedó sin datos desde la última consulta.
    pub fn take_starved(&self) -> bool {
        self.starved.swap(false, Ordering::Relaxed)
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_full(&self) -> bool {
        self.lock().len() >= self.capacity
    }

    /// Encola muestras del decodificador. Devuelve cuántas entraron.
    pub fn push(&self, samples: &[f32]) -> usize {
        let mut q = self.lock();
        let room = self.capacity.saturating_sub(q.len());
        let n = room.min(samples.len());
        q.extend(&samples[..n]); // el resto se TIRA
        n
    }

    /// Rellena el bloque del dispositivo. Devuelve la ganancia resultante.
    ///
    /// Lo que falte se rellena con silencio en vez de repetir muestras viejas.
    ///
    /// La producción siempre es estéreo intercalado (L,R); el dispositivo puede
    /// pedir otra cantidad de canales, así que se mapea sin cambiar el formato:
    /// mono promedia L/R, más de dos canales usa los primeros dos y silencia
    /// el resto. El reloj maestro sólo avanza por contenido real: contar el
    /// silencio de relleno adelantaba al video en cada underrun.
    pub fn fill(&self, out: &mut [f32], gain: f32, k: f32, channels: usize) -> f32 {
        let Ok(mut q) = self.samples.try_lock() else {
            // Nunca bloquear aquí: mejor un bloque de silencio que un corte.
            out.fill(0.0);
            return gain;
        };
        let ch = channels.max(1);
        let mut consumed = 0usize; // muestras estéreo reales sacadas de la cola
        match ch {
            2 => {
                // Camino rápido: el layout del dispositivo coincide.
                let n = q.len().min(out.len());
                for slot in out.iter_mut().take(n) {
                    *slot = q.pop_front().unwrap_or(0.0);
                }
                consumed = n;
                if n < out.len() {
                    out[n..].fill(0.0);
                    self.starved.store(true, Ordering::Relaxed);
                }
            }
            1 => {
                for slot in out.iter_mut() {
                    if q.len() >= 2 {
                        let l = q.pop_front().unwrap_or(0.0);
                        let r = q.pop_front().unwrap_or(0.0);
                        *slot = (l + r) * 0.5;
                        consumed += 2;
                    } else {
                        *slot = 0.0;
                        self.starved.store(true, Ordering::Relaxed);
                    }
                }
            }
            _ => {
                for frame in out.chunks_exact_mut(ch) {
                    if q.len() >= 2 {
                        let l = q.pop_front().unwrap_or(0.0);
                        let r = q.pop_front().unwrap_or(0.0);
                        frame[0] = l;
                        frame[1] = r;
                        for extra in frame[2..].iter_mut() {
                            *extra = 0.0;
                        }
                        consumed += 2;
                    } else {
                        frame.fill(0.0);
                        self.starved.store(true, Ordering::Relaxed);
                    }
                }
            }
        }
        drop(q);

        let target = self.target_gain();
        let result = apply_gain_ramp(out, gain, target, k);
        // Producción estéreo: 2 muestras consumidas = 1 frame de reloj.
        self.frames_played
            .fetch_add((consumed / 2) as u64, Ordering::Relaxed);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_coefficient_is_between_zero_and_one() {
        for sr in [8_000u32, 44_100, 48_000, 192_000] {
            let k = ramp_coefficient(sr);
            assert!(k > 0.0 && k < 1.0, "k={k} para sr={sr}");
        }
    }

    #[test]
    fn ramp_coefficient_with_zero_sample_rate_is_instant() {
        // Sin dividir por cero: aplica el cambio de golpe.
        assert_eq!(ramp_coefficient(0), 1.0);
    }

    #[test]
    fn higher_sample_rate_gives_a_smaller_step() {
        assert!(ramp_coefficient(96_000) < ramp_coefficient(48_000));
    }

    #[test]
    fn gain_ramp_converges_towards_the_target() {
        let k = ramp_coefficient(48_000);
        let mut buf = vec![1.0f32; 4800]; // 100 ms, muy por encima de los 15 ms
        let final_gain = apply_gain_ramp(&mut buf, 0.0, 1.0, k);
        assert!(
            (final_gain - 1.0).abs() < 0.01,
            "la ganancia debería converger, fue {final_gain}"
        );
    }

    #[test]
    fn gain_ramp_is_gradual_not_instant() {
        // Es lo que evita el chasquido: la primera muestra no salta al destino.
        let k = ramp_coefficient(48_000);
        let mut buf = vec![1.0f32; 8];
        apply_gain_ramp(&mut buf, 0.0, 1.0, k);
        assert!(
            buf[0] < 0.01,
            "salto brusco en la primera muestra: {}",
            buf[0]
        );
        assert!(buf[7] > buf[0], "la rampa debe ser monótona");
    }

    #[test]
    fn gain_ramp_to_zero_fades_out() {
        let k = ramp_coefficient(48_000);
        let mut buf = vec![1.0f32; 4800];
        let g = apply_gain_ramp(&mut buf, 1.0, 0.0, k);
        assert!(g < 0.01, "debería haberse desvanecido, fue {g}");
    }

    #[test]
    fn gain_ramp_on_empty_buffer_keeps_the_gain() {
        let mut buf: [f32; 0] = [];
        assert_eq!(apply_gain_ramp(&mut buf, 0.5, 1.0, 0.1), 0.5);
    }

    #[test]
    fn buffer_capacity_scales_with_rate_and_channels() {
        // 48 000 Hz × 1.5 s = 72 000 frames por canal.
        assert_eq!(buffer_capacity(48_000, 2), 144_000);
        assert_eq!(buffer_capacity(48_000, 1), 72_000);
        // channels = 0 se trata como 1 en vez de dar capacidad cero.
        assert_eq!(buffer_capacity(48_000, 0), 72_000);
    }

    #[test]
    fn push_and_fill_roundtrip_the_samples() {
        let b = AudioBuffer::new(1000);
        b.set_target_gain(1.0);
        assert_eq!(b.push(&[0.5, 0.5, 0.25, 0.25]), 4);
        assert_eq!(b.len(), 4);
        let mut out = [0.0f32; 4];
        b.fill(&mut out, 1.0, 1.0, 2);
        assert!(out[0] > 0.0, "debería haber salido audio");
        assert!(b.is_empty());
        assert_eq!(b.frames_played(), 2, "4 muestras estéreo = 2 frames");
    }

    #[test]
    fn push_respects_capacity() {
        let b = AudioBuffer::new(4);
        assert_eq!(b.push(&[0.1; 10]), 4, "sólo deben entrar 4");
        assert!(b.is_full());
        assert_eq!(b.push(&[0.1; 10]), 0, "lleno: no entra nada más");
    }

    #[test]
    fn fill_pads_with_silence_and_flags_starvation() {
        let b = AudioBuffer::new(100);
        b.push(&[1.0, 1.0]);
        let mut out = [9.0f32; 8];
        b.fill(&mut out, 1.0, 1.0, 2);
        assert_eq!(
            &out[2..],
            &[0.0; 6],
            "el resto debe ser silencio, no basura"
        );
        assert!(
            b.take_starved(),
            "debería haberse marcado la falta de datos"
        );
        assert!(!b.take_starved(), "el aviso se consume una sola vez");
    }

    #[test]
    fn starvation_padding_does_not_advance_the_master_clock() {
        // El silencio de relleno no es contenido: contarlo adelantaba al video
        // en cada underrun y armaba la espiral de drops + petardeo.
        let b = AudioBuffer::new(100);
        b.push(&[1.0, 1.0]); // 1 frame estéreo real
        let mut out = [9.0f32; 100]; // 50 frames pedidos, hay para 1
        b.fill(&mut out, 1.0, 1.0, 2);
        assert_eq!(
            b.frames_played(),
            1,
            "sólo el frame con contenido real avanza el reloj"
        );
    }

    #[test]
    fn mono_device_downmixes_stereo_pairs() {
        let b = AudioBuffer::new(100);
        b.set_target_gain(1.0);
        b.push(&[0.6, 0.2, 0.4, -0.2]);
        let mut out = [0.0f32; 2];
        b.fill(&mut out, 1.0, 1.0, 1);
        assert!(
            (out[0] - 0.4).abs() < 1e-6,
            "L y R promediados, fue {}",
            out[0]
        );
        assert!((out[1] - 0.1).abs() < 1e-6, "fue {}", out[1]);
        assert_eq!(b.frames_played(), 2, "dos pares consumidos = dos frames");
    }

    #[test]
    fn surround_device_maps_stereo_to_first_two_channels() {
        let b = AudioBuffer::new(1000);
        b.set_target_gain(1.0);
        b.push(&[0.5, 0.25, 0.75, 0.125]); // dos frames estéreo
        let mut out = [9.0f32; 12]; // 2 frames de 6 canales
        b.fill(&mut out, 1.0, 1.0, 6);
        assert_eq!(&out[0..6], &[0.5, 0.25, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(&out[6..12], &[0.75, 0.125, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(b.frames_played(), 2);
    }

    #[test]
    fn fill_on_empty_buffer_outputs_silence() {
        let b = AudioBuffer::new(100);
        let mut out = [9.0f32; 4];
        b.fill(&mut out, 1.0, 1.0, 2);
        assert_eq!(out, [0.0; 4]);
    }

    #[test]
    fn zero_gain_silences_the_output() {
        let b = AudioBuffer::new(100);
        b.set_target_gain(0.0);
        b.push(&[1.0; 4800]);
        let mut out = [0.0f32; 4800];
        b.fill(&mut out, 0.0, ramp_coefficient(48_000), 2);
        assert!(
            out.iter().all(|s| s.abs() < 0.01),
            "debería estar en silencio"
        );
    }

    #[test]
    fn target_gain_is_clamped() {
        let b = AudioBuffer::new(10);
        b.set_target_gain(5.0);
        assert_eq!(b.target_gain(), 1.0, "sin boost: evita el recorte");
        b.set_target_gain(-1.0);
        assert_eq!(b.target_gain(), 0.0);
    }

    #[test]
    fn flush_clears_the_queue_and_bumps_the_epoch() {
        let b = AudioBuffer::new(100);
        b.push(&[1.0; 50]);
        let before = b.epoch();
        b.flush();
        assert!(b.is_empty(), "un salto debe vaciar el audio pendiente");
        assert_eq!(b.frames_played(), 0, "el reloj se reinicia");
        assert_ne!(b.epoch(), before);
    }

    #[test]
    fn frames_played_accumulates_across_calls() {
        let b = AudioBuffer::new(1000);
        b.push(&[0.1; 100]);
        let mut out = [0.0f32; 50];
        b.fill(&mut out, 1.0, 1.0, 2);
        b.fill(&mut out, 1.0, 1.0, 2);
        assert_eq!(b.frames_played(), 50, "100 muestras estéreo = 50 frames");
    }
}
