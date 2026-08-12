//! Backend de decodificación sobre Media Foundation (Windows).
//!
//! Se usa `IMFSourceReader` en vez de `IMFMediaEngine` porque entrega los
//! frames en RAM, listos para subir a la textura, sin necesidad de un
//! dispositivo D3D11 ni de compartir superficies con wgpu. A cambio, el reloj,
//! el audio y el seek los lleva Rust, que es justo la parte testeable.
//!
//! # Afinidad de hilo
//!
//! `winit` deja el hilo principal en STA (para arrastrar y soltar) y `rfd`
//! también usa COM en STA; Media Foundation quiere MTA. **Nada de este módulo
//! puede tocarse desde el hilo de UI.** Todo vive en un hilo dedicado que hace
//! `CoInitializeEx(MTA)` → `MFStartup` → trabajo → soltar objetos →
//! `MFShutdown` → `CoUninitialize`, en ese orden. `MFShutdown` en un hilo
//! distinto al de `MFStartup`, o antes de soltar los objetos, cuelga.
//!
//! Las interfaces de `windows-core` no son `Send` ni `Sync`, y es correcto que
//! no lo sean: los punteros COM tienen afinidad de apartamento. No se
//! implementa `Send` a mano; lo que cruza de hilo son sólo datos planos.

use std::path::Path;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Media::MediaFoundation::{
    IMFMediaType, IMFSample, IMFSourceReader, MFAudioFormat_Float, MFCreateAttributes,
    MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Audio, MFMediaType_Video,
    MFShutdown, MFStartup, MFVideoFormat_RGB32, MFMEDIASOURCE_CAN_SEEK, MFSTARTUP_LITE,
    MF_API_VERSION, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_DEFAULT_STRIDE,
    MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_MT_VIDEO_ROTATION,
    MF_PD_DURATION, MF_SDK_VERSION, MF_SOURCE_READERF_ENDOFSTREAM,
    MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_SOURCE_READER_MEDIASOURCE,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::LoadLibraryW;

use super::backend::{Decoder, Sample};
use super::ring::{expected_bytes, VideoFrame};
use super::VideoInfo;
use crate::utils::errors::{Result, ShImagesError};

/// Versión de MF: `(MF_SDK_VERSION << 16) | MF_API_VERSION`. El crate `windows`
/// no expone el `MF_VERSION` compuesto.
const MF_VERSION: u32 = (MF_SDK_VERSION << 16) | MF_API_VERSION;

/// Media Foundation cuenta el tiempo en unidades de 100 ns.
const HNS_PER_SEC: u64 = 10_000_000;

/// Canales de la salida de audio. Se fuerza estéreo para no tener que
/// remezclar en Rust.
const AUDIO_CHANNELS: u32 = 2;

fn hns_to_duration(hns: u64) -> Duration {
    // Por partes en vez de `from_nanos(hns * 100)`: multiplicar primero
    // desbordaría a partir de ~58 años de duración.
    Duration::new(hns / HNS_PER_SEC, ((hns % HNS_PER_SEC) * 100) as u32)
}

fn duration_to_hns(d: Duration) -> i64 {
    let hns = d.as_nanos() / 100;
    i64::try_from(hns).unwrap_or(i64::MAX)
}

fn mf_err(context: &str, e: windows::core::Error) -> ShImagesError {
    ShImagesError::Media(format!("{context}: {e}"))
}

fn stream_index(c: windows::Win32::Media::MediaFoundation::MF_SOURCE_READER_CONSTANTS) -> u32 {
    c.0 as u32
}

/// Comprueba que Media Foundation exista antes de tocarla.
///
/// Las ediciones "N" de Windows no la traen hasta instalar el Media Feature
/// Pack. Con `/DELAYLOAD` (ver `build.rs`) el fallo se pospone hasta la primera
/// llamada; esta comprobación lo convierte en un error normal en vez de una
/// excepción del cargador.
pub fn is_available() -> bool {
    // SAFETY: LoadLibraryW con un literal terminado en NUL es seguro; sólo
    // incrementa el contador de referencias de una DLL del sistema. No se
    // libera a propósito: si está, la vamos a usar durante toda la sesión.
    unsafe { LoadLibraryW(PCWSTR(windows::core::w!("mfplat.dll").as_ptr())).is_ok() }
}

/// Sesión COM + Media Foundation de un hilo.
///
/// Debe crearse y destruirse en el mismo hilo, y vivir más que cualquier objeto
/// de MF creado bajo ella.
struct MfSession {
    com_initialized: bool,
}

impl MfSession {
    fn new() -> Result<Self> {
        if !is_available() {
            return Err(ShImagesError::Media(
                "Media Foundation no está disponible en este sistema \
                 (¿edición N sin Media Feature Pack?)"
                    .to_string(),
            ));
        }
        // SAFETY: se llama una vez al principio del hilo dedicado, que no ha
        // entrado en ningún apartamento todavía. RPC_E_CHANGED_MODE sólo puede
        // ocurrir si alguien lo metió en STA antes; en ese caso no marcamos
        // com_initialized y no llamaremos a CoUninitialize.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let com_initialized = hr.is_ok();
        if !com_initialized {
            tracing::warn!(
                ?hr,
                "CoInitializeEx MTA falló; el hilo ya estaba en otro apartamento"
            );
        }
        // SAFETY: COM ya está inicializado en este hilo. MFSTARTUP_LITE evita
        // cargar la parte de red de la plataforma, que no usamos.
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE) }.map_err(|e| mf_err("MFStartup", e))?;
        Ok(Self { com_initialized })
    }
}

impl Drop for MfSession {
    fn drop(&mut self) {
        // SAFETY: se ejecuta en el mismo hilo que llamó a MFStartup, y después
        // de que el IMFSourceReader se haya soltado (el orden de campos de
        // MfDecoder lo garantiza: `reader` va antes que `_session`). Invertir
        // el orden cuelga el proceso.
        unsafe {
            if let Err(e) = MFShutdown() {
                tracing::warn!(error = %e, "MFShutdown falló");
            }
            if self.com_initialized {
                CoUninitialize();
            }
        }
    }
}

/// Decodificador de video basado en `IMFSourceReader`.
pub struct MfDecoder {
    /// Debe declararse **antes** que `_session`: Rust suelta los campos en
    /// orden de declaración, y el reader tiene que morir antes de MFShutdown.
    reader: IMFSourceReader,
    _session: MfSession,
    info: VideoInfo,
    /// Dimensiones negociadas de salida, que pueden ser menores que las nativas.
    out_width: u32,
    out_height: u32,
    /// Negativo = la imagen viene de abajo arriba (lo normal en RGB de MF).
    stride: i32,
    audio_sample_rate: u32,
    /// Índices **reales** de los streams, resueltos al abrir.
    ///
    /// `ReadSample` devuelve el índice real (0, 1, …), no la pseudo-constante
    /// `MF_SOURCE_READER_FIRST_VIDEO_STREAM` (0xFFFFFFFC) con la que se
    /// configuran los formatos. Compararlos entre sí clasificaría todos los
    /// frames de video como audio.
    real_video_stream: Option<u32>,
    real_audio_stream: Option<u32>,
}

impl MfDecoder {
    /// Abre un archivo y negocia los formatos de salida.
    ///
    /// `audio_rate` es la frecuencia del dispositivo de salida. Se le pide a MF
    /// que remuestree él: hacerlo en Rust costaría código y calidad, y MF ya
    /// tiene el remuestreador en la misma cadena.
    pub fn open(path: &Path, output_size: (u32, u32), audio_rate: u32) -> Result<Self> {
        Self::open_with_audio(path, output_size, Some(audio_rate))
    }

    /// Abre el reader dejando deseleccionadas todas las pistas de audio.
    pub fn open_video_only(path: &Path, output_size: (u32, u32)) -> Result<Self> {
        Self::open_with_audio(path, output_size, None)
    }

    fn open_with_audio(
        path: &Path,
        output_size: (u32, u32),
        audio_rate: Option<u32>,
    ) -> Result<Self> {
        let session = MfSession::new()?;

        let wide: Vec<u16> = path.as_os_str().encode_wide_nul().collect::<Vec<u16>>();

        // Atributos del reader: pedirle que convierta el color y escale por
        // nosotros. Sin esto habría que traer NV12 y convertir a mano.
        // SAFETY: el puntero de salida es una variable local válida.
        let attrs = unsafe {
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 1).map_err(|e| mf_err("MFCreateAttributes", e))?;
            let attrs = attrs.ok_or_else(|| {
                ShImagesError::Media("MFCreateAttributes no devolvió atributos".to_string())
            })?;
            // El procesador *avanzado*, no el básico: el básico sólo convierte
            // el formato de color, y rechaza con MF_E_INVALIDMEDIATYPE
            // cualquier cambio de resolución. Sin esto, un clip 2560×1072 no
            // se puede reducir a 1920 de ancho y no abre.
            attrs
                .SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
                .map_err(|e| mf_err("habilitar el procesador de video", e))?;
            attrs
        };

        // SAFETY: `wide` está terminado en NUL y vive durante toda la llamada.
        let reader = unsafe {
            MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), &attrs)
                .map_err(|e| mf_err("abrir el archivo de video", e))?
        };

        let mut decoder = Self {
            reader,
            _session: session,
            info: VideoInfo::default(),
            out_width: 0,
            out_height: 0,
            stride: 0,
            audio_sample_rate: 0,
            real_video_stream: None,
            real_audio_stream: None,
        };
        decoder.resolve_stream_indices();
        decoder.configure(output_size, audio_rate)?;
        Ok(decoder)
    }

    /// Descubre qué índice real ocupa cada stream, leyendo su tipo mayor.
    ///
    /// Se recorren los índices hasta que el reader devuelve error
    /// (`MF_E_INVALIDSTREAMNUMBER`), con un tope por si un contenedor
    /// patológico declarase muchísimos.
    fn resolve_stream_indices(&mut self) {
        const MAX_STREAMS: u32 = 32;
        for index in 0..MAX_STREAMS {
            // SAFETY: el reader está vivo en este hilo; un índice fuera de rango
            // devuelve Err en vez de comportamiento indefinido.
            let Ok(ty) = (unsafe { self.reader.GetNativeMediaType(index, 0) }) else {
                break;
            };
            // SAFETY: `ty` es un IMFMediaType válido recién obtenido.
            let Ok(major) = (unsafe { ty.GetGUID(&MF_MT_MAJOR_TYPE) }) else {
                continue;
            };
            if major == MFMediaType_Video && self.real_video_stream.is_none() {
                self.real_video_stream = Some(index);
            } else if major == MFMediaType_Audio && self.real_audio_stream.is_none() {
                self.real_audio_stream = Some(index);
            }
        }
        tracing::debug!(
            video = ?self.real_video_stream,
            audio = ?self.real_audio_stream,
            "índices de stream resueltos"
        );
    }

    fn configure(&mut self, output_size: (u32, u32), audio_rate: Option<u32>) -> Result<()> {
        // Se usan los índices reales, no las pseudo-constantes: los grabadores
        // de partidas producen contenedores con varias pistas (juego, micro),
        // y ahí `FIRST_AUDIO_STREAM` no selecciona lo que uno espera.
        let video = self.real_video_stream.ok_or_else(|| {
            ShImagesError::Media("el archivo no contiene ninguna pista de video".to_string())
        })?;

        // SAFETY: todas las llamadas van sobre `self.reader`, vivo y en este
        // hilo, con índices que acaba de devolver el propio reader.
        unsafe {
            self.reader
                .SetStreamSelection(stream_index(ALL_STREAMS), false)
                .map_err(|e| mf_err("deseleccionar streams", e))?;
            self.reader
                .SetStreamSelection(video, true)
                .map_err(|e| mf_err("seleccionar el stream de video", e))?;
        }

        let native = self.native_video_type()?;
        let (native_w, native_h) = unsafe { packed_pair(&native, &MF_MT_FRAME_SIZE)? };
        if native_w == 0 || native_h == 0 {
            return Err(ShImagesError::Media(
                "el video no declara dimensiones".to_string(),
            ));
        }
        let (out_w, out_h) = super::ring::output_size((native_w, native_h), output_size);

        self.set_video_output(out_w, out_h)?;

        let fps = unsafe { packed_pair(&native, &MF_MT_FRAME_RATE) }
            .ok()
            .and_then(|(num, den)| {
                if den == 0 {
                    None
                } else {
                    Some(num as f32 / den as f32)
                }
            });
        // SAFETY: `native` sigue vivo; GetUINT32 sobre una clave ausente
        // devuelve Err, que aquí se degrada a 0.
        let rotation = unsafe { native.GetUINT32(&MF_MT_VIDEO_ROTATION) }
            .map(|deg| ((deg / 90) % 4) as u8)
            .unwrap_or(0);

        let has_audio = match (self.real_audio_stream, audio_rate) {
            (Some(audio), Some(rate)) => self.set_audio_output(audio, rate).unwrap_or(false),
            _ => false,
        };

        self.info = VideoInfo {
            width: out_w,
            height: out_h,
            duration: self.read_duration(),
            fps,
            has_audio,
            container_rotation: rotation,
            can_seek: self.read_can_seek(),
        };
        self.out_width = out_w;
        self.out_height = out_h;
        Ok(())
    }

    fn native_video_type(&self) -> Result<IMFMediaType> {
        let video = self.real_video_stream.unwrap_or(0);
        // SAFETY: el reader está vivo y el índice es una constante de MF.
        unsafe {
            self.reader
                .GetNativeMediaType(video, 0)
                .map_err(|e| mf_err("leer el formato nativo del video", e))
        }
    }

    /// Intenta negociar RGB32 con un tamaño concreto.
    ///
    /// `None` en `size` deja que el procesador use la resolución nativa.
    ///
    /// # Safety
    /// Debe llamarse desde el hilo dueño del reader.
    unsafe fn try_video_type(&self, video: u32, size: Option<(u32, u32)>) -> Result<()> {
        // SAFETY: el tipo se crea aquí y sólo se le escriben claves conocidas
        // antes de entregárselo al reader, que está vivo en este hilo.
        unsafe {
            let target = MFCreateMediaType().map_err(|e| mf_err("MFCreateMediaType", e))?;
            target
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(|e| mf_err("fijar el tipo mayor de video", e))?;
            // RGB32 es BGRA en memoria; el swizzle se hace al copiar.
            target
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
                .map_err(|e| mf_err("fijar el subtipo RGB32", e))?;
            if let Some((w, h)) = size {
                target
                    .SetUINT64(&MF_MT_FRAME_SIZE, pack_pair(w, h))
                    .map_err(|e| mf_err("fijar el tamaño de salida", e))?;
            }
            self.reader
                .SetCurrentMediaType(video, None, &target)
                .map_err(|e| mf_err("negociar el formato de salida", e))
        }
    }

    fn set_video_output(&mut self, width: u32, height: u32) -> Result<()> {
        let video = self.real_video_stream.unwrap_or(0);
        let (mut width, mut height) = (width, height);

        // SAFETY: todas las llamadas van sobre `self.reader`, vivo en este hilo.
        unsafe {
            if let Err(first) = self.try_video_type(video, Some((width, height))) {
                // Algunos procesadores no escalan a cualquier tamaño. Antes de
                // dar el archivo por perdido, se prueba a resolución nativa y se
                // deja que la GPU haga la reducción al pintar.
                tracing::debug!(error = %first, width, height, "escalado rechazado; probando resolución nativa");
                self.try_video_type(video, None).map_err(|second| {
                    ShImagesError::Media(format!(
                        "este sistema no puede decodificar este video \
                         (puede faltar el códec, p. ej. HEVC): {second}"
                    ))
                })?;
            }

            // El stride real puede no ser width*4, y si es negativo la imagen
            // viene de abajo arriba.
            let current = self
                .reader
                .GetCurrentMediaType(video)
                .map_err(|e| mf_err("releer el formato de salida", e))?;
            // Las dimensiones efectivas son las que decidió el procesador, no
            // las que pedimos: si cayó al camino nativo, son otras.
            if let Ok((w, h)) = packed_pair(&current, &MF_MT_FRAME_SIZE) {
                if w > 0 && h > 0 {
                    width = w;
                    height = h;
                }
            }
            self.stride = current
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .map(|s| s as i32)
                .unwrap_or((width * 4) as i32);
        }
        self.out_width = width;
        self.out_height = height;
        Ok(())
    }

    /// Devuelve `Ok(true)` si hay pista de audio utilizable.
    fn set_audio_output(&mut self, audio: u32, desired_rate: u32) -> Result<bool> {
        // SAFETY: si no hay stream de audio, SetStreamSelection devuelve Err y
        // se propaga como "sin audio"; no se toca nada más.
        unsafe {
            if self.reader.SetStreamSelection(audio, true).is_err() {
                return Ok(false);
            }
            let target = MFCreateMediaType().map_err(|e| mf_err("MFCreateMediaType audio", e))?;
            target
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
                .map_err(|e| mf_err("fijar el tipo mayor de audio", e))?;
            // f32 intercalado: es lo que espera cpal y evita una conversión.
            target
                .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)
                .map_err(|e| mf_err("fijar el subtipo de audio", e))?;
            target
                .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, AUDIO_CHANNELS)
                .map_err(|e| mf_err("fijar los canales de audio", e))?;
            if desired_rate > 0 {
                // Que MF remuestree a la frecuencia del dispositivo; si no,
                // habría que hacerlo en Rust con peor calidad.
                target
                    .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, desired_rate)
                    .map_err(|e| mf_err("fijar la frecuencia de audio", e))?;
            }
            if self
                .reader
                .SetCurrentMediaType(audio, None, &target)
                .is_err()
            {
                let _ = self.reader.SetStreamSelection(audio, false);
                return Ok(false);
            }
            let current = self
                .reader
                .GetCurrentMediaType(audio)
                .map_err(|e| mf_err("releer el formato de audio", e))?;
            self.audio_sample_rate = current
                .GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)
                .unwrap_or(48_000);
        }
        Ok(true)
    }

    fn read_duration(&self) -> Option<Duration> {
        let src = stream_index(MF_SOURCE_READER_MEDIASOURCE);
        // SAFETY: el reader está vivo; el PROPVARIANT devuelto se libera solo
        // en su Drop.
        let pv: PROPVARIANT =
            unsafe { self.reader.GetPresentationAttribute(src, &MF_PD_DURATION) }.ok()?;
        let hns = u64::try_from(&pv).ok()?;
        if hns == 0 {
            return None;
        }
        Some(hns_to_duration(hns))
    }

    fn read_can_seek(&self) -> bool {
        let src = stream_index(MF_SOURCE_READER_MEDIASOURCE);
        // SAFETY: igual que read_duration.
        let Ok(pv) = (unsafe {
            self.reader.GetPresentationAttribute(
                src,
                &windows::Win32::Media::MediaFoundation::MF_SOURCE_READER_MEDIASOURCE_CHARACTERISTICS,
            )
        }) else {
            return false;
        };
        u64::try_from(&pv)
            .map(|c| (c as i32 & MFMEDIASOURCE_CAN_SEEK.0) != 0)
            .unwrap_or(false)
    }

    /// Copia un frame de MF a un `Vec<u8>` RGBA, corrigiendo las tres trampas
    /// del formato: BGRA en vez de RGBA, stride distinto de `ancho*4`, y orden
    /// de filas invertido cuando el stride es negativo.
    fn copy_frame(&self, sample: &IMFSample, pts: Duration) -> Result<VideoFrame> {
        let width = self.out_width as usize;
        let height = self.out_height as usize;
        let needed = expected_bytes(self.out_width, self.out_height);
        let mut pixels = vec![0u8; needed];

        // SAFETY: `buffer` mantiene vivo el bloqueo mientras exista `guard`, y
        // `guard` hace Unlock en su Drop incluso si salimos por `?`. El puntero
        // es válido durante ese intervalo y se leen como mucho
        // `len` bytes, comprobado contra el stride y la altura.
        unsafe {
            let buffer = sample
                .ConvertToContiguousBuffer()
                .map_err(|e| mf_err("obtener el buffer del frame", e))?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            let mut len: u32 = 0;
            buffer
                .Lock(&mut ptr, None, Some(&mut len))
                .map_err(|e| mf_err("bloquear el buffer del frame", e))?;
            let _guard = BufferGuard(&buffer);
            if ptr.is_null() {
                return Err(ShImagesError::Media(
                    "el buffer del frame llegó vacío".to_string(),
                ));
            }

            let bottom_up = self.stride < 0;
            let abs_stride = self.stride.unsigned_abs() as usize;
            if abs_stride < width * 4 {
                return Err(ShImagesError::Media(format!(
                    "stride inconsistente: {abs_stride} < {}",
                    width * 4
                )));
            }
            if (len as usize) < abs_stride * height {
                return Err(ShImagesError::Media(
                    "el buffer del frame está truncado".to_string(),
                ));
            }

            let src = std::slice::from_raw_parts(ptr, abs_stride * height);
            for y in 0..height {
                let src_row = if bottom_up { height - 1 - y } else { y };
                let src_off = src_row * abs_stride;
                let dst_off = y * width * 4;
                for x in 0..width {
                    let s = src_off + x * 4;
                    let d = dst_off + x * 4;
                    // BGRA -> RGBA. El video es opaco: alfa a 255 siempre.
                    pixels[d] = src[s + 2];
                    pixels[d + 1] = src[s + 1];
                    pixels[d + 2] = src[s];
                    pixels[d + 3] = 255;
                }
            }
        }

        Ok(VideoFrame {
            pts,
            width: self.out_width,
            height: self.out_height,
            pixels,
        })
    }

    fn copy_audio(&self, sample: &IMFSample, pts: Duration) -> Result<Sample> {
        // SAFETY: mismo patrón que copy_frame; el guard garantiza el Unlock.
        let samples = unsafe {
            let buffer = sample
                .ConvertToContiguousBuffer()
                .map_err(|e| mf_err("obtener el buffer de audio", e))?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            let mut len: u32 = 0;
            buffer
                .Lock(&mut ptr, None, Some(&mut len))
                .map_err(|e| mf_err("bloquear el buffer de audio", e))?;
            let _guard = BufferGuard(&buffer);
            if ptr.is_null() {
                Vec::new()
            } else {
                let bytes = std::slice::from_raw_parts(ptr, len as usize);
                bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect()
            }
        };
        Ok(Sample::Audio { pts, samples })
    }

    /// Frecuencia de muestreo negociada para el audio.
    pub fn audio_sample_rate(&self) -> u32 {
        self.audio_sample_rate
    }
}

/// Garantiza el `Unlock` de un buffer aunque salgamos por `?`.
struct BufferGuard<'a>(&'a windows::Win32::Media::MediaFoundation::IMFMediaBuffer);

impl Drop for BufferGuard<'_> {
    fn drop(&mut self) {
        // SAFETY: el buffer sigue vivo (lo mantiene el préstamo) y fue
        // bloqueado justo antes de construir el guard.
        unsafe {
            let _ = self.0.Unlock();
        }
    }
}

/// Constante auxiliar: `MF_SOURCE_READER_ALL_STREAMS` con el nombre corto.
const ALL_STREAMS: windows::Win32::Media::MediaFoundation::MF_SOURCE_READER_CONSTANTS =
    windows::Win32::Media::MediaFoundation::MF_SOURCE_READER_ALL_STREAMS;

/// Empaqueta dos `u32` en el `u64` que usan `MF_MT_FRAME_SIZE` y
/// `MF_MT_FRAME_RATE`: el primero en los 32 bits altos.
fn pack_pair(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

/// Desempaqueta un atributo `u64` de MF en su par de `u32`.
///
/// # Safety
/// `ty` debe ser un `IMFMediaType` vivo del hilo actual.
unsafe fn packed_pair(ty: &IMFMediaType, key: &windows::core::GUID) -> Result<(u32, u32)> {
    let packed =
        unsafe { ty.GetUINT64(key) }.map_err(|e| mf_err("leer un atributo empaquetado", e))?;
    Ok(((packed >> 32) as u32, (packed & 0xFFFF_FFFF) as u32))
}

impl Decoder for MfDecoder {
    fn info(&self) -> VideoInfo {
        self.info
    }

    fn next_sample(&mut self) -> Result<Sample> {
        let mut actual_stream: u32 = 0;
        let mut flags: u32 = 0;
        let mut timestamp: i64 = 0;
        let mut sample: Option<IMFSample> = None;

        // SAFETY: los cuatro punteros de salida son variables locales vivas
        // durante la llamada. ALL_STREAMS pide la siguiente muestra de
        // cualquier stream seleccionado, en orden temporal.
        unsafe {
            self.reader
                .ReadSample(
                    stream_index(ALL_STREAMS),
                    0,
                    Some(&mut actual_stream),
                    Some(&mut flags),
                    Some(&mut timestamp),
                    Some(&mut sample),
                )
                .map_err(|e| mf_err("leer una muestra", e))?;
        }

        if flags & (MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
            return Ok(Sample::EndOfStream);
        }

        let Some(sample) = sample else {
            // Sin muestra pero sin fin de stream: MF pide que volvamos a
            // llamar (cambio de formato, hueco en el stream). Se modela como
            // audio vacío para no bloquear el bucle del llamante.
            return Ok(Sample::Audio {
                pts: Duration::ZERO,
                samples: Vec::new(),
            });
        };

        let pts = hns_to_duration(timestamp.max(0) as u64);
        // Comparar contra el índice REAL: `ReadSample` nunca devuelve la
        // pseudo-constante con la que se configuran los formatos.
        if Some(actual_stream) == self.real_video_stream {
            self.copy_frame(&sample, pts).map(Sample::Video)
        } else if Some(actual_stream) == self.real_audio_stream {
            self.copy_audio(&sample, pts)
        } else {
            // Stream que no nos interesa (subtítulos, datos): ignorarlo sin
            // tratarlo como audio, que metería basura en el buffer de PCM.
            tracing::trace!(actual_stream, "muestra de un stream no seleccionado");
            Ok(Sample::Audio {
                pts,
                samples: Vec::new(),
            })
        }
    }

    fn seek(&mut self, target: Duration) -> Result<()> {
        if !self.info.can_seek {
            return Err(ShImagesError::Media(
                "este video no permite búsqueda".to_string(),
            ));
        }
        let pv = PROPVARIANT::from(duration_to_hns(target));
        // SAFETY: GUID_NULL como formato de tiempo significa "unidades de
        // 100 ns". `pv` vive durante toda la llamada y se libera en su Drop.
        unsafe {
            self.reader
                .SetCurrentPosition(&windows::core::GUID::zeroed(), &pv)
                .map_err(|e| mf_err("saltar a una posición", e))?;
        }
        Ok(())
    }

    fn set_output_size(&mut self, width: u32, height: u32) -> Result<()> {
        self.set_video_output(width, height)?;
        self.info.width = width;
        self.info.height = height;
        Ok(())
    }
}

/// Abre un video con Media Foundation.
pub fn open(path: &Path, output_size: (u32, u32), audio_rate: u32) -> Result<Box<dyn Decoder>> {
    let decoder = MfDecoder::open(path, output_size, audio_rate)?;
    Ok(Box::new(decoder))
}

/// Abre sólo la pista de video para trabajos puntuales como miniaturas.
pub fn open_video_only(path: &Path, output_size: (u32, u32)) -> Result<Box<dyn Decoder>> {
    let decoder = MfDecoder::open_video_only(path, output_size)?;
    Ok(Box::new(decoder))
}

/// Extensión local para obtener una cadena ancha terminada en NUL.
trait EncodeWideNul {
    fn encode_wide_nul(&self) -> std::vec::IntoIter<u16>;
}

impl EncodeWideNul for std::ffi::OsStr {
    fn encode_wide_nul(&self) -> std::vec::IntoIter<u16> {
        use std::os::windows::ffi::OsStrExt;
        let mut v: Vec<u16> = self.encode_wide().collect();
        v.push(0);
        v.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mf_version_matches_the_documented_formula() {
        // MF_VERSION = (MF_SDK_VERSION << 16) | MF_API_VERSION
        assert_eq!(MF_VERSION, (2u32 << 16) | 112u32);
    }

    #[test]
    fn hns_conversion_roundtrips() {
        let d = Duration::from_millis(1500);
        assert_eq!(duration_to_hns(d), 15_000_000);
        assert_eq!(hns_to_duration(15_000_000), d);
        assert_eq!(hns_to_duration(HNS_PER_SEC), Duration::from_secs(1));
    }

    #[test]
    fn hns_conversion_saturates_instead_of_overflowing() {
        assert_eq!(duration_to_hns(Duration::MAX), i64::MAX);
        let _ = hns_to_duration(u64::MAX);
    }

    #[test]
    fn packed_pair_roundtrips() {
        let packed = pack_pair(1920, 1080);
        assert_eq!((packed >> 32) as u32, 1920);
        assert_eq!((packed & 0xFFFF_FFFF) as u32, 1080);
    }

    #[test]
    fn wide_string_is_nul_terminated() {
        use std::ffi::OsStr;
        let v: Vec<u16> = OsStr::new("a.mp4").encode_wide_nul().collect();
        assert_eq!(v.last(), Some(&0));
        assert_eq!(v.len(), 6);
    }

    #[test]
    fn media_foundation_is_available_on_this_machine() {
        // Informativo: en una edición N sin Media Feature Pack sería false, y
        // la app debe seguir arrancando igual.
        let available = is_available();
        tracing::debug!(available, "disponibilidad de Media Foundation");
    }

    #[test]
    fn opening_a_missing_file_fails_without_panicking() {
        let err = MfDecoder::open(Path::new("no_existe_12345.mp4"), (1920, 1080), 48_000)
            .err()
            .expect("abrir un archivo inexistente debe fallar");
        assert!(matches!(err, ShImagesError::Media(_)));
    }

    #[test]
    fn opening_a_non_video_file_fails_without_panicking() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("basura.mp4");
        std::fs::write(&path, b"esto no es un mp4").expect("escribir");
        let err = MfDecoder::open(&path, (1920, 1080), 48_000)
            .err()
            .expect("un archivo corrupto debe fallar");
        assert!(matches!(err, ShImagesError::Media(_)));
    }

    /// Decodifica un archivo real de principio a fin.
    ///
    /// Ignorado por defecto: depende de tener un video en la máquina, y CI no
    /// puede committear uno (AGENTS.md §8.2 limita las fixtures a 100 KB).
    /// Ejecutar con:
    ///
    /// ```text
    /// SH_IMAGES_TEST_VIDEO=C:\ruta\clip.mp4 cargo test --lib real_video -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "necesita SH_IMAGES_TEST_VIDEO apuntando a un video real"]
    fn real_video_decodes_frames_and_seeks() {
        let Ok(path) = std::env::var("SH_IMAGES_TEST_VIDEO") else {
            panic!("define SH_IMAGES_TEST_VIDEO con la ruta de un video");
        };
        let path = std::path::PathBuf::from(path);
        let mut decoder = MfDecoder::open(&path, (1920, 1080), 48_000)
            .unwrap_or_else(|e| panic!("no se pudo abrir {}: {e}", path.display()));

        let info = decoder.info();
        println!("info: {info:?}");
        println!(
            "streams reales -> video: {:?}, audio: {:?}, rate: {}",
            decoder.real_video_stream, decoder.real_audio_stream, decoder.audio_sample_rate
        );
        assert!(info.width > 0 && info.height > 0, "dimensiones inválidas");
        assert!(
            info.width <= 1920 && info.height <= 1080,
            "el tope de resolución no se aplicó: {}x{}",
            info.width,
            info.height
        );

        // Lee un número fijo de muestras en vez de cortar al llegar a N frames:
        // el intercalado del contenedor puede entregar bastante video antes del
        // primer bloque de audio, y cortar antes daría un falso negativo.
        const SAMPLES_TO_READ: usize = 400;
        let mut video_frames = 0usize;
        let mut audio_samples = 0usize;
        let mut last_pts = Duration::ZERO;
        for _ in 0..SAMPLES_TO_READ {
            match decoder.next_sample().expect("leer muestra") {
                Sample::Video(frame) => {
                    assert!(
                        frame.is_consistent(),
                        "buffer inconsistente: {} bytes para {}x{}",
                        frame.pixels.len(),
                        frame.width,
                        frame.height
                    );
                    assert!(
                        frame.pts >= last_pts,
                        "marcas temporales no monótonas: {:?} tras {:?}",
                        frame.pts,
                        last_pts
                    );
                    last_pts = frame.pts;
                    video_frames += 1;
                }
                Sample::Audio { samples, .. } => audio_samples += samples.len(),
                Sample::EndOfStream => break,
            }
        }
        println!("frames de video: {video_frames}, muestras de audio: {audio_samples}");
        assert!(video_frames >= 10, "se decodificaron muy pocos frames");
        if info.has_audio {
            assert!(audio_samples > 0, "declara audio pero no entregó muestras");
        }

        // Comprueba que no todo el frame sea negro: valida el swizzle BGRA→RGBA
        // y el volteo vertical, que es donde falla un decodificador mal cableado.
        let mut sample_frame = None;
        for _ in 0..120 {
            if let Ok(Sample::Video(f)) = decoder.next_sample() {
                sample_frame = Some(f);
                break;
            }
        }
        if let Some(f) = sample_frame {
            let non_black = f
                .pixels
                .chunks_exact(4)
                .filter(|p| p[0] > 8 || p[1] > 8 || p[2] > 8)
                .count();
            let total = f.pixels.len() / 4;
            println!("píxeles no negros: {non_black}/{total}");
            assert!(
                non_black > total / 100,
                "el frame salió prácticamente negro"
            );
            assert!(
                f.pixels.chunks_exact(4).all(|p| p[3] == 255),
                "el alfa debe ser opaco"
            );
        }

        // Salto: debe aterrizar cerca del objetivo (MF va al fotograma clave
        // anterior, así que se admite un GOP de margen).
        if info.can_seek {
            // Dentro del clip: saltar más allá del final daría fin de stream y
            // ningún frame, que es correcto pero no es lo que se quiere medir.
            let target = info
                .duration
                .map(|d| d / 2)
                .unwrap_or(Duration::from_secs(5))
                .min(Duration::from_secs(5));
            println!("saltando a {target:?}");
            decoder.seek(target).expect("saltar");
            let mut landed = None;
            for _ in 0..300 {
                match decoder.next_sample().expect("leer tras el salto") {
                    Sample::Video(f) => {
                        landed = Some(f.pts);
                        break;
                    }
                    Sample::EndOfStream => break,
                    _ => {}
                }
            }
            let landed = landed.expect("ningún frame tras el salto");
            println!("el salto aterrizó en {landed:?}");
            assert!(
                landed <= target + Duration::from_secs(1),
                "aterrizó demasiado tarde: {landed:?}"
            );
            assert!(
                landed + Duration::from_secs(5) >= target,
                "aterrizó demasiado pronto: {landed:?}"
            );
        }
    }

    #[test]
    fn opening_an_empty_file_fails_without_panicking() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("vacio.mp4");
        std::fs::write(&path, b"").expect("escribir");
        assert!(MfDecoder::open(&path, (1920, 1080), 48_000).is_err());
    }
}
