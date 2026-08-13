//! Buffer acotado de frames decodificados, con reciclado de asignaciones.
//!
//! El video **no** pasa por `ImageCache`: su regla de admisión es
//! todo-o-nada contra un límite de 512 MiB y la UI renderiza desde la caché,
//! así que un video ni se cachearía ni se mostraría. Aquí sólo viven unos pocos
//! frames a la vez.
//!
//! Dos propiedades importantes:
//!
//! - **Contrapresión**: `push` bloquea cuando el buffer está lleno, así que la
//!   decodificación se autolimita a la velocidad de reproducción. Al pausar, el
//!   buffer se llena y el hilo decodificador se duerme solo en la `Condvar`.
//! - **Reciclado**: los `Vec` de píxeles vuelven al decodificador tras subirlos
//!   a la GPU. Sin esto, 1080p30 son 30 asignaciones de 8,29 MB por segundo
//!   (~60 000 fallos de página/s), que cuesta más que la propia copia.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// Frames en vuelo. Tres da ~100 ms de colchón a 30 fps y 23,7 MiB a 1080p.
pub const RING_CAPACITY: usize = 3;

/// Tope de resolución de salida. A 4K un frame RGBA son 31,6 MiB y tres
/// llenarían 95 MiB; Media Foundation puede escalar gratis en el mismo paso de
/// conversión de color.
pub const MAX_OUTPUT_WIDTH: u32 = 1920;
pub const MAX_OUTPUT_HEIGHT: u32 = 1080;

/// Un frame decodificado, listo para subir a la textura.
///
/// Los píxeles vienen ya en el orden de `egui::Color32` (RGBA sin premultiplicar
/// con alfa 255), para que la UI sólo tenga que moverlos, no convertirlos.
#[derive(Debug)]
pub struct VideoFrame {
    pub pts: Duration,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl VideoFrame {
    /// Bytes que ocupa el buffer de píxeles.
    pub fn bytes(&self) -> usize {
        self.pixels.len()
    }

    /// `true` si el buffer tiene exactamente el tamaño que declaran las
    /// dimensiones. Un frame truncado subido a la GPU daría basura.
    pub fn is_consistent(&self) -> bool {
        self.pixels.len() == expected_bytes(self.width, self.height)
    }
}

/// Bytes que ocupa un frame RGBA de estas dimensiones.
pub fn expected_bytes(width: u32, height: u32) -> usize {
    (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4)
}

/// Resolución de salida a pedirle al decodificador.
///
/// Escala **uniformemente** hasta que quepa en el viewport y en el tope duro.
/// Acotar cada eje por separado deformaría el contenido —un 2560×1072 acabaría
/// como 1920×1072, de 2.39:1 a 1.79:1— y además Media Foundation rechaza ese
/// media type con `MF_E_INVALIDMEDIATYPE`.
///
/// Las dimensiones se redondean a par: los formatos con submuestreo de croma
/// (todo lo que produce un decodificador H.264) no admiten lados impares.
pub fn output_size(native: (u32, u32), viewport: (u32, u32)) -> (u32, u32) {
    let (nw, nh) = (native.0.max(1), native.1.max(1));
    let (vw, vh) = (viewport.0.max(1), viewport.1.max(1));

    let limit_w = vw.min(MAX_OUTPUT_WIDTH);
    let limit_h = vh.min(MAX_OUTPUT_HEIGHT);

    // Nunca se amplía: `scale` se acota a 1.0.
    let scale = (f64::from(limit_w) / f64::from(nw))
        .min(f64::from(limit_h) / f64::from(nh))
        .min(1.0);

    let w = ((f64::from(nw) * scale).round() as u32).max(1);
    let h = ((f64::from(nh) * scale).round() as u32).max(1);
    (round_to_even(w), round_to_even(h))
}

/// Redondea a la baja hasta un valor par, con suelo en 2.
fn round_to_even(v: u32) -> u32 {
    if v <= 2 {
        return 2;
    }
    v & !1
}

/// `true` si conviene renegociar la resolución de salida tras un cambio de
/// tamaño de ventana.
///
/// Con histéresis: renegociar el media type por cada píxel de un arrastre de
/// ventana reiniciaría el pipeline decenas de veces por segundo.
pub fn should_renegotiate(current: (u32, u32), desired: (u32, u32)) -> bool {
    if current == desired {
        return false;
    }
    let ratio_w = ratio_change(current.0, desired.0);
    let ratio_h = ratio_change(current.1, desired.1);
    ratio_w > 0.25 || ratio_h > 0.25
}

fn ratio_change(current: u32, desired: u32) -> f32 {
    if current == 0 {
        return f32::INFINITY;
    }
    let diff = current.abs_diff(desired) as f32;
    diff / current as f32
}

/// Buffer compartido entre el hilo decodificador y el de UI.
#[derive(Debug)]
pub struct FrameRing {
    inner: Mutex<RingInner>,
    /// Notifica tanto al productor (hay hueco) como al consumidor (hay frame).
    cv: Condvar,
    closed: AtomicBool,
    /// Se incrementa en cada seek; los frames de una época vieja se descartan.
    epoch: AtomicU64,
}

#[derive(Debug, Default)]
struct RingInner {
    frames: VecDeque<(u64, VideoFrame)>,
    /// Asignaciones devueltas por la UI, listas para reutilizar.
    free: Vec<Vec<u8>>,
    capacity: usize,
}

impl FrameRing {
    pub fn new() -> Self {
        Self::with_capacity(RING_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(RingInner {
                frames: VecDeque::new(),
                free: Vec::new(),
                capacity: capacity.max(1),
            }),
            cv: Condvar::new(),
            closed: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RingInner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Época actual. El decodificador la lee antes de empezar un frame.
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    /// Invalida lo pendiente tras un seek y devuelve la época nueva.
    ///
    /// Los buffers de los frames descartados van a la lista de reciclado.
    pub fn bump_epoch(&self) -> u64 {
        let next = self.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        {
            let mut inner = self.lock();
            while let Some((_, frame)) = inner.frames.pop_front() {
                inner.free.push(frame.pixels);
            }
        }
        self.cv.notify_all();
        next
    }

    /// Cierra el buffer y despierta a todos: el decodificador debe terminar.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.cv.notify_all();
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// Encola un frame, bloqueando mientras el buffer esté lleno.
    ///
    /// Devuelve `false` si el buffer se cerró o el frame quedó obsoleto por un
    /// seek: en ambos casos su asignación se recicla.
    pub fn push(&self, epoch: u64, frame: VideoFrame) -> bool {
        let mut inner = self.lock();
        loop {
            if self.closed.load(Ordering::Acquire) || epoch != self.epoch() {
                inner.free.push(frame.pixels);
                return false;
            }
            if inner.frames.len() < inner.capacity {
                inner.frames.push_back((epoch, frame));
                drop(inner);
                self.cv.notify_all();
                return true;
            }
            inner = self.cv.wait(inner).unwrap_or_else(|p| p.into_inner());
        }
    }

    /// Saca el frame más antiguo sin bloquear. Para el hilo de UI.
    pub fn try_pop(&self) -> Option<VideoFrame> {
        let popped = {
            let mut inner = self.lock();
            let current = self.epoch();
            // Descarta de una vez lo que quedó de épocas anteriores.
            while let Some((e, _)) = inner.frames.front() {
                if *e == current {
                    break;
                }
                if let Some((_, stale)) = inner.frames.pop_front() {
                    inner.free.push(stale.pixels);
                }
            }
            inner.frames.pop_front().map(|(_, f)| f)
        };
        if popped.is_some() {
            self.cv.notify_all();
        }
        popped
    }

    /// Marca temporal del frame a la cabeza, sin sacarlo.
    pub fn peek_pts(&self) -> Option<Duration> {
        let inner = self.lock();
        let current = self.epoch();
        inner
            .frames
            .front()
            .filter(|(e, _)| *e == current)
            .map(|(_, f)| f.pts)
    }

    /// Devuelve una asignación tras subir el frame a la GPU.
    ///
    /// Buffers de tamaño distinto al actual no se guardan: no sirven tras un
    /// cambio de resolución y sólo ocuparían memoria.
    pub fn recycle(&self, mut pixels: Vec<u8>, expected: usize) {
        if pixels.capacity() < expected {
            return;
        }
        pixels.clear();
        let mut inner = self.lock();
        if inner.free.len() < inner.capacity + 1 {
            inner.free.push(pixels);
        }
    }

    /// Toma una asignación reciclada con capacidad suficiente, o crea una.
    pub fn take_buffer(&self, needed: usize) -> Vec<u8> {
        let mut inner = self.lock();
        if let Some(pos) = inner.free.iter().position(|b| b.capacity() >= needed) {
            let mut buf = inner.free.swap_remove(pos);
            buf.clear();
            return buf;
        }
        drop(inner);
        Vec::with_capacity(needed)
    }

    pub fn len(&self) -> usize {
        self.lock().frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_full(&self) -> bool {
        let inner = self.lock();
        inner.frames.len() >= inner.capacity
    }

    /// Bytes retenidos ahora mismo, incluida la lista de reciclado.
    pub fn bytes_in_use(&self) -> usize {
        let inner = self.lock();
        let queued: usize = inner.frames.iter().map(|(_, f)| f.pixels.len()).sum();
        let recycled: usize = inner.free.iter().map(Vec::capacity).sum();
        queued + recycled
    }

    /// Techo de memoria del buffer para unas dimensiones dadas.
    ///
    /// `capacity + 1` porque puede haber un frame en la lista de reciclado
    /// mientras el resto está en cola.
    pub fn capacity_bytes(&self, width: u32, height: u32) -> usize {
        let inner = self.lock();
        expected_bytes(width, height).saturating_mul(inner.capacity + 1)
    }
}

impl Default for FrameRing {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn frame(pts_ms: u64, w: u32, h: u32) -> VideoFrame {
        VideoFrame {
            pts: Duration::from_millis(pts_ms),
            width: w,
            height: h,
            pixels: vec![0u8; expected_bytes(w, h)],
        }
    }

    #[test]
    fn expected_bytes_is_four_per_pixel() {
        assert_eq!(expected_bytes(2, 3), 24);
        assert_eq!(expected_bytes(1920, 1080), 8_294_400);
        assert_eq!(expected_bytes(3840, 2160), 33_177_600);
    }

    #[test]
    fn expected_bytes_saturates_instead_of_overflowing() {
        let _ = expected_bytes(u32::MAX, u32::MAX);
    }

    #[test]
    fn output_size_caps_at_1080p() {
        assert_eq!(output_size((3840, 2160), (3840, 2160)), (1920, 1080));
    }

    #[test]
    fn output_size_respects_a_smaller_viewport() {
        assert_eq!(output_size((1920, 1080), (800, 600)), (800, 450));
    }

    #[test]
    fn output_size_never_upscales_beyond_native() {
        assert_eq!(output_size((640, 480), (1920, 1080)), (640, 480));
    }

    #[test]
    fn output_size_preserves_the_aspect_ratio() {
        // Regresión de un clip real de 2560×1072: acotar cada eje por separado
        // daba 1920×1072 (de 2.39:1 a 1.79:1) y Media Foundation lo rechazaba
        // con MF_E_INVALIDMEDIATYPE.
        let (w, h) = output_size((2560, 1072), (1920, 1080));
        assert_eq!((w, h), (1920, 804));
        let nativo = 2560.0 / 1072.0;
        let salida = f64::from(w) / f64::from(h);
        assert!(
            (nativo - salida).abs() < 0.01,
            "aspecto {salida} frente al nativo {nativo}"
        );
    }

    #[test]
    fn output_size_preserves_the_aspect_ratio_for_tall_video() {
        // Vertical de móvil: manda el límite de altura, no el de anchura.
        let (w, h) = output_size((1080, 1920), (1920, 1080));
        assert_eq!((w, h), (608, 1080));
        assert!(h <= MAX_OUTPUT_HEIGHT && w <= MAX_OUTPUT_WIDTH);
        let nativo = 1080.0 / 1920.0;
        let salida = f64::from(w) / f64::from(h);
        assert!((nativo - salida).abs() < 0.01, "aspecto {salida}");
    }

    #[test]
    fn output_size_is_always_even() {
        // Los formatos con submuestreo de croma no admiten lados impares.
        for native in [(1001, 999), (777, 333), (2560, 1073), (35, 17)] {
            let (w, h) = output_size(native, (1920, 1080));
            assert_eq!(w % 2, 0, "ancho impar {w} para {native:?}");
            assert_eq!(h % 2, 0, "alto impar {h} para {native:?}");
        }
    }

    #[test]
    fn output_size_is_never_zero() {
        // Un media type de tamaño cero haría fallar la negociación.
        assert_eq!(output_size((1920, 1080), (0, 0)), (2, 2));
        assert_eq!(output_size((0, 0), (800, 600)), (2, 2));
    }

    #[test]
    fn renegotiation_needs_a_significant_change() {
        assert!(!should_renegotiate((1920, 1080), (1920, 1080)));
        // Arrastrar la ventana unos píxeles no debe reiniciar el pipeline.
        assert!(!should_renegotiate((1920, 1080), (1900, 1070)));
        assert!(should_renegotiate((1920, 1080), (800, 600)));
    }

    #[test]
    fn renegotiation_from_zero_always_triggers() {
        assert!(should_renegotiate((0, 0), (640, 480)));
    }

    #[test]
    fn frame_consistency_detects_a_truncated_buffer() {
        let good = frame(0, 4, 4);
        assert!(good.is_consistent());
        assert_eq!(good.bytes(), 64);
        let bad = VideoFrame {
            pts: Duration::ZERO,
            width: 4,
            height: 4,
            pixels: vec![0u8; 10],
        };
        assert!(!bad.is_consistent());
    }

    #[test]
    fn push_then_pop_returns_the_frame() {
        let ring = FrameRing::new();
        assert!(ring.is_empty());
        assert!(ring.push(ring.epoch(), frame(100, 2, 2)));
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.peek_pts(), Some(Duration::from_millis(100)));
        let got = ring.try_pop().expect("debería haber un frame");
        assert_eq!(got.pts, Duration::from_millis(100));
        assert!(ring.is_empty());
    }

    #[test]
    fn pop_is_fifo() {
        let ring = FrameRing::new();
        let e = ring.epoch();
        for i in 0..3 {
            assert!(ring.push(e, frame(i * 33, 2, 2)));
        }
        for i in 0..3 {
            let f = ring.try_pop().expect("frame");
            assert_eq!(f.pts, Duration::from_millis(i * 33));
        }
    }

    #[test]
    fn try_pop_on_empty_returns_none() {
        let ring = FrameRing::new();
        assert!(ring.try_pop().is_none());
        assert!(ring.peek_pts().is_none());
    }

    #[test]
    fn ring_reports_full_at_capacity() {
        let ring = FrameRing::with_capacity(2);
        let e = ring.epoch();
        assert!(ring.push(e, frame(0, 2, 2)));
        assert!(!ring.is_full());
        assert!(ring.push(e, frame(33, 2, 2)));
        assert!(ring.is_full());
    }

    #[test]
    fn zero_capacity_is_clamped_to_one() {
        let ring = FrameRing::with_capacity(0);
        assert!(ring.push(ring.epoch(), frame(0, 2, 2)));
        assert!(ring.is_full());
    }

    #[test]
    fn push_blocks_until_the_consumer_drains() {
        // Contrapresión: es lo que hace que pausar duerma al decodificador.
        let ring = Arc::new(FrameRing::with_capacity(1));
        let e = ring.epoch();
        assert!(ring.push(e, frame(0, 2, 2)));

        let producer = {
            let ring = Arc::clone(&ring);
            std::thread::spawn(move || ring.push(e, frame(33, 2, 2)))
        };
        // El productor está bloqueado; hacer hueco lo libera.
        let first = ring.try_pop().expect("primer frame");
        assert_eq!(first.pts, Duration::ZERO);
        assert!(producer
            .join()
            .expect("el productor no debe entrar en pánico"));
        assert_eq!(ring.len(), 1);
    }

    #[test]
    fn close_unblocks_a_waiting_producer() {
        let ring = Arc::new(FrameRing::with_capacity(1));
        let e = ring.epoch();
        assert!(ring.push(e, frame(0, 2, 2)));
        let producer = {
            let ring = Arc::clone(&ring);
            std::thread::spawn(move || ring.push(e, frame(33, 2, 2)))
        };
        ring.close();
        assert!(
            !producer
                .join()
                .expect("el productor no debe entrar en pánico"),
            "push debe devolver false tras cerrar"
        );
        assert!(ring.is_closed());
    }

    #[test]
    fn bump_epoch_discards_queued_frames() {
        let ring = FrameRing::new();
        let e = ring.epoch();
        ring.push(e, frame(0, 2, 2));
        ring.push(e, frame(33, 2, 2));
        assert_eq!(ring.len(), 2);
        let new_epoch = ring.bump_epoch();
        assert_ne!(new_epoch, e);
        assert!(ring.is_empty(), "un seek debe vaciar lo pendiente");
        assert!(ring.try_pop().is_none());
    }

    #[test]
    fn push_with_a_stale_epoch_is_rejected() {
        let ring = FrameRing::new();
        let stale = ring.epoch();
        ring.bump_epoch();
        assert!(
            !ring.push(stale, frame(0, 2, 2)),
            "un frame decodificado antes del seek no debe presentarse"
        );
        assert!(ring.is_empty());
    }

    #[test]
    fn discarded_frames_return_their_buffers_to_the_free_list() {
        let ring = FrameRing::with_capacity(2);
        let e = ring.epoch();
        ring.push(e, frame(0, 16, 16));
        ring.bump_epoch();
        // El buffer descartado se reutiliza en vez de asignar de nuevo.
        let buf = ring.take_buffer(expected_bytes(16, 16));
        assert!(buf.capacity() >= expected_bytes(16, 16));
    }

    #[test]
    fn recycled_buffers_are_reused_without_allocating() {
        let ring = FrameRing::new();
        let needed = expected_bytes(64, 64);
        let original = Vec::<u8>::with_capacity(needed);
        let ptr = original.as_ptr();
        ring.recycle(original, needed);
        let reused = ring.take_buffer(needed);
        assert_eq!(reused.as_ptr(), ptr, "debería ser la misma asignación");
        assert!(reused.is_empty(), "el buffer reciclado llega vacío");
    }

    #[test]
    fn undersized_buffers_are_not_recycled() {
        let ring = FrameRing::new();
        ring.recycle(Vec::with_capacity(10), 1000);
        // No hay nada reutilizable: take_buffer asigna de cero.
        let buf = ring.take_buffer(1000);
        assert!(buf.capacity() >= 1000);
    }

    #[test]
    fn free_list_does_not_grow_without_bound() {
        let ring = FrameRing::with_capacity(3);
        let needed = expected_bytes(8, 8);
        for _ in 0..50 {
            ring.recycle(Vec::with_capacity(needed), needed);
        }
        assert!(
            ring.bytes_in_use() <= needed * 5,
            "la lista de reciclado debe estar acotada, fueron {} bytes",
            ring.bytes_in_use()
        );
    }

    #[test]
    fn capacity_bytes_stays_under_the_budget_at_1080p() {
        // Presupuesto: ≤64 MiB de RAM adicional reproduciendo 1080p.
        let ring = FrameRing::new();
        let bytes = ring.capacity_bytes(1920, 1080);
        assert!(
            bytes <= 64 * 1024 * 1024,
            "el buffer de 1080p ocuparía {bytes} bytes"
        );
    }

    #[test]
    fn bytes_in_use_tracks_queued_frames() {
        let ring = FrameRing::new();
        assert_eq!(ring.bytes_in_use(), 0);
        ring.push(ring.epoch(), frame(0, 16, 16));
        assert_eq!(ring.bytes_in_use(), expected_bytes(16, 16));
    }
}
