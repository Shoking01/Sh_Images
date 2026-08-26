//! Reproductor: une decodificador, audio y reloj en un hilo dedicado.
//!
//! El hilo de UI nunca toca Media Foundation ni COM (ver `mf`): sólo envía
//! órdenes por un canal y lee frames ya decodificados de un buffer acotado.
//!
//! Contrapresión y consumo en reposo: el hilo se bloquea en `Condvar` cuando el
//! buffer está lleno y en `recv()` cuando está en pausa, así que un video
//! pausado consume 0 % de CPU sin lógica adicional.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[cfg(windows)]
use super::audio::ramp_coefficient;
use super::audio::{buffer_capacity, AudioBuffer};
use super::backend::{self, Decoder, Sample};
use super::clock::{audio_position, PlayClock};
use super::playback::{next_state, should_restart_from_zero, PlaybackEvent, PlaybackState};
use super::ring::{FrameRing, VideoFrame};
use super::timeline::{self, FrameDecision};
use super::{seek, volume, VideoInfo};

/// Órdenes del hilo de UI al decodificador.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PlayerCommand {
    Play,
    Pause,
    SeekTo(Duration),
    Shutdown,
}

/// Estado que el hilo decodificador publica para la UI.
#[derive(Debug, Default)]
struct Shared {
    info: VideoInfo,
    /// Posición del último salto, base del reloj de audio.
    seek_base: Duration,
    audio_sample_rate: u32,
    reached_end: bool,
    error: Option<String>,
    ready: bool,
}

/// Instantánea para pintar los controles.
#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    pub state: PlaybackState,
    pub position: Duration,
    pub info: VideoInfo,
    pub volume: u8,
    pub muted: bool,
    pub error: Option<String>,
}

impl PlayerSnapshot {
    pub fn duration(&self) -> Option<Duration> {
        self.info.duration
    }
}

/// Reproductor de un archivo de video.
///
/// Al soltarlo se para el hilo y se hace `join`: nunca quedan dos pistas de
/// audio sonando ni hilos huérfanos con recursos COM.
pub struct VideoPlayer {
    path: PathBuf,
    ring: Arc<FrameRing>,
    audio: Arc<AudioBuffer>,
    shared: Arc<Mutex<Shared>>,
    cmd_tx: mpsc::Sender<PlayerCommand>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,

    // Estado que vive en el hilo de UI.
    state: PlaybackState,
    clock: PlayClock,
    volume: u8,
    muted: bool,
    remembered_volume: u8,
    consecutive_drops: u32,
    /// Salto acumulado pendiente de enviar, para no encolar un `SetCurrentPosition`
    /// por cada pulsación al mantener la tecla.
    pending_seek: Option<Duration>,
    last_seek_request: Option<Instant>,
}

/// Espera antes de materializar un salto, para agrupar pulsaciones seguidas.
const SEEK_DEBOUNCE: Duration = Duration::from_millis(120);

impl VideoPlayer {
    /// Abre un video y arranca el hilo decodificador.
    ///
    /// La apertura real ocurre en el hilo: esta función no bloquea, y los
    /// errores aparecen luego en `snapshot().error`.
    pub fn open(path: &Path, viewport: (u32, u32), volume: u8, autoplay: bool) -> Self {
        // La frecuencia del dispositivo se consulta ANTES de dimensionar el
        // buffer: con el rate real, el colchón de 1.5 s dura lo que dice durar
        // también en dispositivos a 96 kHz (antes se dimensionaba siempre a
        // 48 kHz y el margen real era la mitad o menos).
        //
        // El backend WASAPI de cpal inicializa COM en STA sobre el hilo que lo
        // llama, así que la consulta corre en un hilo descartable para no
        // dejar el hilo de UI en el apartamento equivocado. Cuesta unos pocos
        // ms una sola vez por apertura.
        let device_rate = std::thread::spawn(audio_sample_rate_probe)
            .join()
            .unwrap_or(0);
        let buffer_rate = if device_rate == 0 {
            48_000
        } else {
            device_rate
        };

        let ring = Arc::new(FrameRing::new());
        let audio = Arc::new(AudioBuffer::new(buffer_capacity(buffer_rate, 2)));
        let shared = Arc::new(Mutex::new(Shared::default()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (cmd_tx, cmd_rx) = mpsc::channel();

        let state = if autoplay {
            PlaybackState::Playing
        } else {
            PlaybackState::Stopped
        };

        let thread = {
            let path = path.to_path_buf();
            let ring = Arc::clone(&ring);
            let audio = Arc::clone(&audio);
            let shared = Arc::clone(&shared);
            let shutdown = Arc::clone(&shutdown);
            std::thread::Builder::new()
                .name("sh_images-video".to_string())
                .spawn(move || {
                    decoder_thread(
                        path,
                        viewport,
                        device_rate,
                        autoplay,
                        ring,
                        audio,
                        shared,
                        shutdown,
                        cmd_rx,
                    );
                })
                .ok()
        };

        let mut player = Self {
            path: path.to_path_buf(),
            ring,
            audio,
            shared,
            cmd_tx,
            shutdown,
            thread,
            state,
            clock: PlayClock::new(),
            volume: volume::clamp_volume(volume),
            muted: false,
            remembered_volume: volume::clamp_volume(volume),
            consecutive_drops: 0,
            pending_seek: None,
            last_seek_request: None,
        };
        player.apply_gain();
        if autoplay {
            player.clock.start(Instant::now());
        }
        player
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Diagnóstico: muestras estéreo encoladas en el buffer de audio.
    pub fn audio_len(&self) -> usize {
        self.audio.len()
    }

    /// Diagnóstico: frames de video en el ring.
    pub fn ring_len(&self) -> usize {
        self.ring.len()
    }

    /// Diagnóstico: rate del dispositivo de salida (0 = desconocido).
    pub fn audio_sample_rate(&self) -> u32 {
        self.shared().audio_sample_rate
    }

    /// Diagnóstico: reloj maestro crudo (frames consumidos por el dispositivo).
    pub fn audio_frames_played(&self) -> u64 {
        self.audio.frames_played()
    }

    /// Diagnóstico: base del reloj de audio (último salto).
    pub fn audio_seek_base(&self) -> Duration {
        self.shared().seek_base
    }

    fn shared(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `true` cuando el decodificador ya publicó los metadatos.
    pub fn is_ready(&self) -> bool {
        self.shared().ready
    }

    pub fn info(&self) -> VideoInfo {
        self.shared().info
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Posición actual, derivada del audio cuando lo hay.
    ///
    /// El dispositivo de audio no corre exactamente a su frecuencia nominal:
    /// derivar el video de `Instant` acumularía desincronía labial.
    pub fn position(&self, now: Instant) -> Duration {
        let s = self.shared();
        if s.info.has_audio && s.audio_sample_rate > 0 {
            audio_position(
                s.seek_base,
                self.audio.frames_played(),
                s.audio_sample_rate,
                Duration::ZERO,
            )
        } else {
            self.clock.now(now)
        }
    }

    pub fn snapshot(&self, now: Instant) -> PlayerSnapshot {
        let position = self.position(now);
        let s = self.shared();
        PlayerSnapshot {
            state: self.state,
            position: match s.info.duration {
                Some(total) => position.min(total),
                None => position,
            },
            info: s.info,
            volume: self.volume,
            muted: self.muted,
            error: s.error.clone(),
        }
    }

    fn send(&self, cmd: PlayerCommand) {
        if self.cmd_tx.send(cmd).is_err() {
            tracing::debug!("el hilo de video ya terminó; orden descartada");
        }
    }

    /// Aplica un evento de la máquina de estados.
    pub fn dispatch(&mut self, event: PlaybackEvent, now: Instant) {
        let restart = should_restart_from_zero(self.state, event);
        let next = next_state(self.state, event);
        if restart {
            self.seek_to(Duration::ZERO, now);
        }
        if next == self.state {
            return;
        }
        self.state = next;
        match next {
            PlaybackState::Playing => {
                self.clock.start(now);
                self.send(PlayerCommand::Play);
            }
            PlaybackState::Paused | PlaybackState::Ended => {
                self.clock.pause(now);
                self.send(PlayerCommand::Pause);
            }
            PlaybackState::Stopped => {
                self.clock.pause(now);
                self.send(PlayerCommand::Pause);
            }
        }
    }

    pub fn toggle_play_pause(&mut self, now: Instant) {
        self.dispatch(PlaybackEvent::TogglePlayPause, now);
    }

    /// Salto relativo, agrupando pulsaciones seguidas.
    pub fn seek_relative(&mut self, delta_secs: i64, now: Instant) {
        let base = self.pending_seek.unwrap_or_else(|| self.position(now));
        let duration = self.info().duration;
        let target = seek::seek_relative(base, delta_secs, duration);
        self.pending_seek = Some(target);
        self.last_seek_request = Some(now);
        // El contador de la UI salta ya; el decodificador va detrás.
        self.clock.seek_to(target, now);
        if seek::seek_reaches_end(target, duration) {
            self.dispatch(PlaybackEvent::ReachedEnd, now);
        }
    }

    pub fn seek_forward(&mut self, now: Instant) {
        self.seek_relative(seek::SEEK_STEP.as_secs() as i64, now);
    }

    pub fn seek_backward(&mut self, now: Instant) {
        self.seek_relative(-(seek::SEEK_STEP.as_secs() as i64), now);
    }

    /// Salto absoluto, para la barra de progreso.
    pub fn seek_to(&mut self, target: Duration, now: Instant) {
        let target = seek::seek_absolute(target, self.info().duration);
        self.pending_seek = Some(target);
        self.last_seek_request = Some(now);
        self.clock.seek_to(target, now);
    }

    /// Envía el salto pendiente cuando pasó el tiempo de agrupación.
    fn flush_pending_seek(&mut self, now: Instant) {
        let (Some(target), Some(requested)) = (self.pending_seek, self.last_seek_request) else {
            return;
        };
        if now.saturating_duration_since(requested) < SEEK_DEBOUNCE {
            return;
        }
        self.pending_seek = None;
        self.last_seek_request = None;
        self.ring.bump_epoch();
        {
            let mut s = self.shared();
            s.reached_end = false;
        }
        self.send(PlayerCommand::SeekTo(target));
        self.dispatch(PlaybackEvent::Seek, now);
    }

    pub fn volume(&self) -> u8 {
        self.volume
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn volume_up(&mut self) {
        let (v, muted) =
            volume::volume_up_unmuting(self.volume, self.muted, self.remembered_volume);
        self.volume = v;
        self.muted = muted;
        self.apply_gain();
    }

    pub fn volume_down(&mut self) {
        self.volume = volume::volume_down(self.volume);
        self.apply_gain();
    }

    pub fn set_volume(&mut self, v: u8) {
        self.volume = volume::clamp_volume(v);
        self.apply_gain();
    }

    pub fn toggle_mute(&mut self) {
        let (muted, v) = volume::toggle_mute(self.muted, self.volume, self.remembered_volume);
        if muted {
            self.remembered_volume = v;
        } else {
            self.volume = v;
        }
        self.muted = muted;
        self.apply_gain();
    }

    fn apply_gain(&self) {
        self.audio
            .set_target_gain(volume::effective_gain(self.volume, self.muted));
    }

    /// Avanza un frame si toca. Devuelve el que hay que subir a la textura.
    ///
    /// El descarte de frames atrasados ocurre aquí, antes de tocar la GPU.
    pub fn tick(&mut self, now: Instant) -> Option<VideoFrame> {
        self.flush_pending_seek(now);

        if self.shared().reached_end && self.state == PlaybackState::Playing {
            self.dispatch(PlaybackEvent::ReachedEnd, now);
        }

        let clock = self.position(now);
        let frame_interval = self
            .info()
            .fps
            .filter(|f| *f > 0.0)
            .map(|f| Duration::from_secs_f32(1.0 / f))
            .unwrap_or(Duration::from_millis(33))
            .max(timeline::MIN_FRAME_INTERVAL);
        let early_tolerance = frame_interval / 2;
        let mut presentable = None;
        while let Some(pts) = self.ring.peek_pts() {
            let decision = timeline::frame_decision(pts, clock, timeline::LATE_THRESHOLD);
            self.consecutive_drops =
                timeline::update_drop_counter(self.consecutive_drops, decision);
            match decision {
                FrameDecision::Wait(remaining) => {
                    if remaining <= early_tolerance {
                        presentable = self.ring.try_pop();
                    }
                    break;
                }
                FrameDecision::Present => {
                    presentable = self.ring.try_pop();
                    break;
                }
                FrameDecision::Drop => {
                    // Nunca dejar el buffer vacío: sin frame nuevo la pantalla
                    // se quedaría en negro.
                    if self.ring.len() <= 1 {
                        presentable = self.ring.try_pop();
                        break;
                    }
                    if let Some(stale) = self.ring.try_pop() {
                        self.recycle(stale);
                    }
                }
            }
        }
        presentable
    }

    /// Devuelve la asignación de un frame ya subido, para que el decodificador
    /// la reutilice en vez de pedir 8 MB nuevos treinta veces por segundo.
    pub fn recycle(&self, frame: VideoFrame) {
        let expected = frame.pixels.len();
        self.ring.recycle(frame.pixels, expected);
    }

    /// Cuánto esperar antes del próximo repintado. `None` si no hay que
    /// repintar (en pausa: cero repintados, cero CPU).
    pub fn repaint_after(&self, now: Instant) -> Option<Duration> {
        if self.state != PlaybackState::Playing {
            return None;
        }
        let clock = self.position(now);
        Some(timeline::time_to_next_frame(
            self.ring.peek_pts(),
            clock,
            self.info().fps,
        ))
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        // Parar antes de soltar: si no, dos videos seguidos solaparían audio.
        self.shutdown.store(true, Ordering::Release);
        let _ = self.cmd_tx.send(PlayerCommand::Shutdown);
        // Despertar al decodificador si está bloqueado en push y cortar la cola:
        // ambas cosas son baratas y síncronas.
        self.ring.close();
        // El join pesado (teardown del stream WASAPI + cierre del decodificador
        // FFmpeg) corre en un hilo descartable: hacerlo aquí congelaba la UI en
        // cada cambio rápido de video. Los Arcs de ring/audio/shared sobreviven
        // en el hilo decodificador hasta que termina; este hilo sólo espera.
        if let Some(handle) = self.thread.take() {
            std::thread::Builder::new()
                .name("sh_images-video-close".to_string())
                .spawn(move || {
                    if handle.join().is_err() {
                        tracing::warn!("el hilo de video terminó en pánico");
                    }
                })
                .ok();
        }
    }
}

/// Toma el mutex de estado compartido tolerando envenenamiento, igual que el
/// resto del proyecto.
fn lock(s: &Arc<Mutex<Shared>>) -> std::sync::MutexGuard<'_, Shared> {
    s.lock().unwrap_or_else(|p| p.into_inner())
}

/// Ejecuta un salto en el hilo decodificador.
///
/// El flush del audio y el reset de `seek_base` ocurren AQUÍ, después de que
/// el backend acepta el salto, y no en el hilo de UI: el audio viejo (anterior
/// al salto) no puede sonar con la base de posición nueva. También se arma el
/// filtro de pre-roll: MF aterriza en el fotograma clave anterior al objetivo,
/// así que las primeras muestras traen PTS anterior a `target` y hay que
/// descartarlas hasta que el stream llegue al objetivo.
fn apply_seek(
    decoder: &mut Box<dyn Decoder>,
    audio: &AudioBuffer,
    shared: &Arc<Mutex<Shared>>,
    seek_target: &mut Option<Duration>,
    target: Duration,
) {
    if let Err(e) = decoder.seek(target) {
        tracing::debug!(error = %e, "el salto falló");
        return;
    }
    audio.flush();
    {
        let mut s = lock(shared);
        s.seek_base = target;
        s.reached_end = false;
    }
    *seek_target = Some(target);
}

/// Cuerpo del hilo decodificador. Todo COM vive y muere aquí.
#[allow(clippy::too_many_arguments)]
fn decoder_thread(
    path: PathBuf,
    viewport: (u32, u32),
    audio_rate: u32,
    autoplay: bool,
    ring: Arc<FrameRing>,
    audio: Arc<AudioBuffer>,
    shared: Arc<Mutex<Shared>>,
    shutdown: Arc<AtomicBool>,
    cmd_rx: mpsc::Receiver<PlayerCommand>,
) {
    // El rate del dispositivo lo consulta `open` antes de crear el buffer
    // (ver comentario ahí); llega listo por parámetro.
    let mut decoder = match backend::open(&path, viewport, audio_rate) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "no se pudo abrir el video");
            let mut s = lock(&shared);
            s.error = Some(e.to_string());
            s.ready = true;
            return;
        }
    };

    let mut info = decoder.info();
    {
        let mut s = lock(&shared);
        s.info = info;
        s.audio_sample_rate = audio_rate;
        s.ready = true;
    }

    // El stream de cpal vive en este hilo: `cpal::Stream` no es `Send`.
    let stream = if info.has_audio {
        audio_device().and_then(|d| start_audio_stream(d, Arc::clone(&audio)))
    } else {
        None
    };
    if info.has_audio && stream.is_none() {
        // Sin salida de audio real: el audio no puede ser reloj maestro
        // ni fuente de contrapresión, o el video se congela.
        info.has_audio = false;
        lock(&shared).info.has_audio = false;
        lock(&shared).audio_sample_rate = 0;
    }
    let _stream = stream;

    let mut playing = autoplay;
    let mut seek_target: Option<Duration> = None;
    let mut duration_poll_count: u64 = 0;
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        // Drenar órdenes sin bloquear.
        let mut disconnected = false;
        loop {
            match cmd_rx.try_recv() {
                Ok(PlayerCommand::Play) => playing = true,
                Ok(PlayerCommand::Pause) => playing = false,
                Ok(PlayerCommand::SeekTo(target)) => {
                    apply_seek(&mut decoder, &audio, &shared, &mut seek_target, target);
                }
                Ok(PlayerCommand::Shutdown) => return,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if disconnected {
            break;
        }

        // En pausa, con al menos un frame listo, dormir hasta la próxima orden:
        // es lo que da 0 % de CPU con el video parado.
        if !playing && !ring.is_empty() {
            match cmd_rx.recv() {
                Ok(PlayerCommand::Play) => playing = true,
                Ok(PlayerCommand::Pause) => {}
                Ok(PlayerCommand::SeekTo(target)) => {
                    apply_seek(&mut decoder, &audio, &shared, &mut seek_target, target);
                }
                Ok(PlayerCommand::Shutdown) | Err(_) => break,
            }
            continue;
        }

        if lock(&shared).reached_end {
            // Terminado: esperar a un salto o a que se reanude.
            match cmd_rx.recv() {
                Ok(PlayerCommand::SeekTo(target)) => {
                    apply_seek(&mut decoder, &audio, &shared, &mut seek_target, target);
                }
                Ok(PlayerCommand::Play) => playing = true,
                Ok(PlayerCommand::Pause) => playing = false,
                Ok(PlayerCommand::Shutdown) | Err(_) => break,
            }
            continue;
        }

        // Contrapresión: se espera si CUALQUIERA de los dos buffers está
        // lleno. Dejar que el video corra libre mientras sólo el audio frena
        // (como probamos antes) hace que el decode se adelante segundos a la
        // reproducción real: el ring (sólo 3 huecos) se llena enseguida con
        // los primeros frames y se queda ahí, porque nada nuevo entra hasta
        // que el audio se llena del todo — para entonces esos frames ya están
        // obsoletos y se descartan, dejando el ring vacío y la imagen
        // congelada aunque el audio siga avanzando. El ring necesita seguir
        // frenando el decode para mantenerse cerca del tiempo real.
        //
        // EXCEPCIÓN que rompe el deadlock reloj↔ring: si la cola de audio
        // está por debajo de la marca de agua baja, se sigue decodificando
        // AUNQUE el ring esté lleno. Si no, el video priorizado llena el
        // ring antes de que el audio fluya, el callback se queda sin nada,
        // frames_played (reloj maestro) se congela y la UI nunca drena: el
        // video se clava en el primer segundo (observado con dispositivo
        // a 192 kHz).
        // La marca de agua por la que el bus del audio pide avidez tiene que ser
        // en tiempo, no en muestras: con dispositivos a 192 kHz, capacity/3 son
        // ~170 ms de margen; un ring lleno tardaría poco en dejar al audio sin
        // nada y armar el deadlock. 0.15 s es robusto en cualquier rate.
        const AUDIO_LOW_SECONDS: f32 = 0.15;
        let audio_low = ((audio.capacity() as f32
            / crate::core::video::audio::AUDIO_BUFFER_SECONDS)
            * AUDIO_LOW_SECONDS) as usize;
        let audio_hungry = audio.len() < audio_low;
        if audio.is_full() || (ring.is_full() && !audio_hungry) {
            match cmd_rx.recv_timeout(Duration::from_millis(4)) {
                Ok(PlayerCommand::Shutdown) => break,
                Ok(PlayerCommand::Play) => playing = true,
                Ok(PlayerCommand::Pause) => playing = false,
                Ok(PlayerCommand::SeekTo(target)) => {
                    apply_seek(&mut decoder, &audio, &shared, &mut seek_target, target);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            continue;
        }

        // Throttle de video por PTS: no encolar si el frame más nuevo ya está
        // >40 ms adelante del audio consumido. Sin esto, con el colchón de
        // 1.5 s el decodificador corre segundos adelantado y tick ve siempre
        // el próximo a +50 ms (tope) → 20 fps.
        if let Some(newest) = ring.peek_newest_pts() {
            let a_clock = {
                let s = lock(&shared);
                if s.info.has_audio && s.audio_sample_rate > 0 {
                    crate::core::video::clock::audio_position(
                        s.seek_base,
                        audio.frames_played(),
                        s.audio_sample_rate,
                        Duration::ZERO,
                    )
                } else {
                    Duration::ZERO
                }
            };
            if newest.saturating_sub(a_clock) > Duration::from_millis(40) {
                match cmd_rx.recv_timeout(Duration::from_millis(4)) {
                    Ok(PlayerCommand::SeekTo(t)) => {
                        apply_seek(&mut decoder, &audio, &shared, &mut seek_target, t);
                    }
                    Ok(PlayerCommand::Play) => playing = true,
                    Ok(PlayerCommand::Pause) => playing = false,
                    Ok(PlayerCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                continue;
            }
        }

        let epoch = ring.epoch();
        match decoder.next_sample() {
            Ok(Sample::Video(frame)) => {
                // El video pre-roll del salto lo descarta la UI vía `epoch` +
                // `frame_decision` (Drop). El bloqueo aquí es intencional: es
                // lo que mantiene al decodificador cerca del ritmo real. Ver
                // el comentario del gate de contrapresión más arriba sobre
                // por qué `try_push` (sin bloquear) resultó peor, no mejor.
                //
                // EXCEPCIÓN (deadlock reloj↔ring): si el audio pasa hambre y
                // el ring está lleno, el frame de video se sacrifica (píxeles
                // reciclados) en vez de bloquear el hilo: el audio tiene
                // prioridad porque ES el reloj maestro.
                if ring.is_full() && audio.len() < audio_low {
                    let expected = frame.pixels.len();
                    ring.recycle(frame.pixels, expected);
                    continue;
                }
                if !ring.push(epoch, frame) && ring.is_closed() {
                    break;
                }
            }
            Ok(Sample::Audio { pts, samples }) => {
                if seek::discard_before_seek_target(pts, seek_target) {
                    // Pre-roll del salto: descartar audio anterior al objetivo.
                    continue;
                }
                // El audio ya alcanzó el objetivo: el filtro cumplió su papel.
                seek_target = None;
                if !samples.is_empty() {
                    audio.push(&samples);
                }
            }
            Ok(Sample::EndOfStream) => {
                // Al llegar al final la duración ya debería ser conocida; si
                // no lo era, éste es el último momento para averiguarla.
                if info.duration.is_none() {
                    if let Some(d) = decoder.refresh_duration() {
                        info.duration = Some(d);
                        lock(&shared).info.duration = Some(d);
                    }
                }
                lock(&shared).reached_end = true;
            }
            Err(e) => {
                tracing::warn!(error = %e, "fallo al decodificar; se detiene el video");
                lock(&shared).error = Some(e.to_string());
                break;
            }
        }

        // Algunos contenedores no reportan la duración hasta leer parte del
        // archivo; re-consultarla periódicamente mientras siga desconocida.
        if info.duration.is_none() {
            duration_poll_count += 1;
            if duration_poll_count.is_multiple_of(30) {
                if let Some(d) = decoder.refresh_duration() {
                    info.duration = Some(d);
                    lock(&shared).info.duration = Some(d);
                }
            }
        }
    }

    // El decodificador (y con él el IMFSourceReader) se suelta aquí, antes de
    // que MfSession haga MFShutdown en su Drop.
    drop(decoder);
}

/// Dispositivo de salida elegido.
struct AudioDevice {
    #[cfg(windows)]
    device: cpal::Device,
    #[cfg(windows)]
    config: cpal::StreamConfig,
    sample_rate: u32,
    #[cfg(windows)]
    channels: usize,
}

/// Consulta la frecuencia del dispositivo de salida.
///
/// Pensada para ejecutarse en un hilo de usar y tirar: cpal inicializa COM en
/// STA sobre el hilo que lo llama, y el hilo decodificador tiene que quedarse
/// en MTA para Media Foundation.
fn audio_sample_rate_probe() -> u32 {
    // On CI headless Windows (and in unit tests) cpal can AV when no device exists.
    // Tests open missing files and only need the error path — audio rate is irrelevant.
    if cfg!(test) {
        return 0;
    }
    // Also skip probe on CI env where no audio hardware exists (extra safety for integration tests)
    if std::env::var("CI").is_ok() {
        return 0;
    }
    audio_device().map(|d| d.sample_rate).unwrap_or(0)
}

#[cfg(windows)]
fn audio_device() -> Option<AudioDevice> {
    if cfg!(test) {
        return None;
    }
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let supported = device.default_output_config().ok()?;
    if supported.sample_format() != cpal::SampleFormat::F32 {
        // Sólo f32: convertir formatos exóticos añadiría código por un caso
        // que WASAPI no produce en la práctica.
        tracing::debug!(format = ?supported.sample_format(), "formato de audio no soportado");
        return None;
    }
    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    Some(AudioDevice {
        config: supported.into(),
        device,
        sample_rate,
        channels,
    })
}

#[cfg(not(windows))]
fn audio_device() -> Option<AudioDevice> {
    None
}

#[cfg(windows)]
fn start_audio_stream(dev: AudioDevice, audio: Arc<AudioBuffer>) -> Option<cpal::Stream> {
    use cpal::traits::{DeviceTrait, StreamTrait};
    let k = ramp_coefficient(dev.sample_rate);
    let channels = dev.channels;
    let mut gain = 0.0f32;
    let stream = dev
        .device
        .build_output_stream(
            &dev.config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                gain = audio.fill(data, gain, k, channels);
            },
            |err| tracing::warn!(error = %err, "error en el stream de audio"),
            None,
        )
        .ok()?;
    if let Err(e) = stream.play() {
        tracing::warn!(error = %e, "no se pudo arrancar el audio");
        return None;
    }
    Some(stream)
}

#[cfg(not(windows))]
fn start_audio_stream(_dev: AudioDevice, _audio: Arc<AudioBuffer>) -> Option<()> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player_for(path: &str) -> VideoPlayer {
        VideoPlayer::open(Path::new(path), (1920, 1080), 80, false)
    }

    #[test]
    fn opening_a_missing_file_reports_an_error_without_panicking() {
        let p = player_for("no_existe_98765.mp4");
        // El hilo publica el error; se espera un poco a que arranque.
        for _ in 0..100 {
            if p.is_ready() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let snap = p.snapshot(Instant::now());
        assert!(
            snap.error.is_some(),
            "debería reportar un error de apertura"
        );
    }

    #[test]
    fn dropping_the_player_joins_its_thread() {
        // Es lo que evita audio solapado al cambiar de video y procesos zombie
        // al cerrar la app.
        let p = player_for("no_existe_98765.mp4");
        drop(p);
    }

    #[test]
    fn volume_controls_clamp_and_remember() {
        let mut p = player_for("no_existe_98765.mp4");
        assert_eq!(p.volume(), 80);
        p.volume_up();
        assert_eq!(p.volume(), 85);
        p.volume_down();
        assert_eq!(p.volume(), 80);
        for _ in 0..30 {
            p.volume_down();
        }
        assert_eq!(p.volume(), 0);
        for _ in 0..30 {
            p.volume_up();
        }
        assert_eq!(p.volume(), 100);
    }

    #[test]
    fn mute_roundtrips_the_volume() {
        let mut p = player_for("no_existe_98765.mp4");
        p.set_volume(60);
        p.toggle_mute();
        assert!(p.is_muted());
        p.toggle_mute();
        assert!(!p.is_muted());
        assert_eq!(p.volume(), 60);
    }

    #[test]
    fn volume_up_while_muted_unmutes() {
        let mut p = player_for("no_existe_98765.mp4");
        p.set_volume(50);
        p.toggle_mute();
        p.volume_up();
        assert!(!p.is_muted(), "subir el volumen debe quitar el silencio");
    }

    #[test]
    fn paused_player_requests_no_repaint() {
        // La condición de 0 % de CPU en pausa.
        let p = player_for("no_existe_98765.mp4");
        assert_eq!(p.state(), PlaybackState::Stopped);
        assert_eq!(p.repaint_after(Instant::now()), None);
    }

    #[test]
    fn toggle_play_pause_moves_the_state() {
        let mut p = player_for("no_existe_98765.mp4");
        let now = Instant::now();
        p.toggle_play_pause(now);
        assert_eq!(p.state(), PlaybackState::Playing);
        p.toggle_play_pause(now);
        assert_eq!(p.state(), PlaybackState::Paused);
    }

    #[test]
    fn seek_backward_at_zero_stays_at_zero() {
        let mut p = player_for("no_existe_98765.mp4");
        let now = Instant::now();
        p.seek_backward(now);
        assert_eq!(p.pending_seek, Some(Duration::ZERO));
    }

    #[test]
    fn tick_with_full_ring_always_makes_progress_even_with_frozen_clock() {
        // Deadlock reloj↔ring: con el reloj maestro congelado (sin audio), el
        // segundo frame (16 ms adelante) sería "Wait" eterno y el ring lleno
        // no drenaría jamás. La garantía de progreso presenta el más viejo.
        let mut p = player_for("no_existe_98765.mp4");
        // El hilo decodificador murió ya (stub); el ring lo llenamos a mano.
        let epoch = p.ring.epoch();
        for i in 0..6u64 {
            let frame = VideoFrame {
                pts: Duration::from_millis(16 * i + 16), // todos "tempranos" vs reloj 0
                width: 4,
                height: 4,
                pixels: vec![0u8; crate::core::video::ring::expected_bytes(4, 4)],
            };
            assert!(p.ring.push(epoch, frame), "el ring debe aceptar 6 frames");
        }
        assert_eq!(p.ring.len(), 6);

        // Reloj congelado en 0: ningún frame alcanza al reloj.
        let presented = p.tick(Instant::now());
        assert!(
            presented.is_some(),
            "ring lleno + reloj congelado debe presentar el frame más viejo"
        );
        assert!(p.ring.len() < 6, "el ring debe drenar");
    }

    #[test]
    fn repeated_seeks_coalesce_into_one_target() {
        // Mantener pulsada la tecla no debe encolar un salto por pulsación.
        let mut p = player_for("no_existe_98765.mp4");
        let now = Instant::now();
        for _ in 0..5 {
            p.seek_forward(now);
        }
        assert_eq!(
            p.pending_seek,
            Some(Duration::from_secs(25)),
            "cinco saltos de 5 s deben acumularse en uno de 25 s"
        );
    }
}
