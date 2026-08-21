//! Frontera con el decodificador nativo.
//!
//! El backend sólo produce datos planos: marcas temporales, píxeles y muestras
//! PCM. No decide cuándo presentar, cuándo descartar ni cuándo terminar; eso es
//! de los módulos puros de este mismo paquete. Así toda la lógica se testea en
//! las tres plataformas aunque el decodificador sólo exista en una.

use std::path::Path;
use std::time::Duration;

use super::ring::VideoFrame;
use super::VideoInfo;
use crate::utils::errors::Result;

/// Lo que el decodificador entrega en cada lectura.
#[derive(Debug)]
pub enum Sample {
    Video(VideoFrame),
    /// PCM intercalado f32, con la marca temporal de la primera muestra.
    Audio {
        pts: Duration,
        samples: Vec<f32>,
    },
    /// Se acabó el archivo.
    EndOfStream,
}

/// Decodificador de un archivo de video.
///
/// Las implementaciones **no** son `Send`: los punteros COM tienen afinidad de
/// apartamento y nunca deben cruzar de hilo. El objeto se crea, se usa y se
/// destruye en el mismo hilo dedicado; lo que viaja por `mpsc` son los
/// `Sample`, que sí son datos planos.
pub trait Decoder {
    /// Metadatos leídos al abrir.
    fn info(&self) -> VideoInfo;

    /// Siguiente muestra disponible. Bloquea mientras decodifica.
    fn next_sample(&mut self) -> Result<Sample>;

    /// Salta a `target`. Puede aterrizar antes (el decodificador va al
    /// fotograma clave anterior); descartar hasta `target` es del llamante.
    fn seek(&mut self, target: Duration) -> Result<()>;

    /// Renegocia la resolución de salida tras un cambio de tamaño de ventana.
    fn set_output_size(&mut self, width: u32, height: u32) -> Result<()>;

    /// Reconsulta la duración, que algunos contenedores no reportan hasta
    /// leer el índice. Devuelve `None` si sigue sin conocerse.
    ///
    /// La implementación por defecto devuelve lo ya conocido; los backends
    /// que puedan mejorarla la sobrescriben.
    fn refresh_duration(&mut self) -> Option<Duration> {
        self.info().duration
    }
}

/// Abre un archivo de video con el backend de la plataforma.
///
/// `audio_rate` es la frecuencia del dispositivo de salida, para que el
/// decodificador remuestree él y no haya que hacerlo en Rust.
pub fn open(path: &Path, output_size: (u32, u32), audio_rate: u32) -> Result<Box<dyn Decoder>> {
    platform::open(path, output_size, audio_rate)
}

/// Abre sólo la pista de video, sin negociar ni decodificar audio.
///
/// Esta variante se usa para extraer miniaturas: seleccionar la pista de
/// audio desperdiciaría trabajo y podría hacer que varias muestras PCM
/// llegasen antes que el primer fotograma visible.
pub fn open_video_only(path: &Path, output_size: (u32, u32)) -> Result<Box<dyn Decoder>> {
    platform::open_video_only(path, output_size)
}

/// `true` si esta plataforma puede reproducir video.
///
/// En Windows depende además de que Media Foundation esté presente: las
/// ediciones "N" no la traen hasta instalar el Media Feature Pack.
pub fn is_available() -> bool {
    platform::is_available()
}

// Slice1: FFmpeg is primary backend. MF kept for rollback comparison (deleted Slice3).
// All platforms now route through ffmpeg stub (probe via LoadLibraryW). The
// `video` feature enables real ffmpeg-next linking; without it we still probe
// and return Media errors without panicking (REQ-FF-011).

#[cfg(windows)]
mod ffmpeg_platform {
    use super::{Decoder, Result};
    use std::path::Path;
    pub fn open(path: &Path, output_size: (u32, u32), audio_rate: u32) -> Result<Box<dyn Decoder>> {
        super::super::ffmpeg::FfmpegDecoder::open(path, output_size, audio_rate)
            .map(|d| Box::new(d) as Box<dyn Decoder>)
    }
    pub fn open_video_only(path: &Path, output_size: (u32, u32)) -> Result<Box<dyn Decoder>> {
        super::super::ffmpeg::FfmpegDecoder::open_video_only(path, output_size)
            .map(|d| Box::new(d) as Box<dyn Decoder>)
    }
    pub fn is_available() -> bool {
        super::super::ffmpeg::is_available()
    }
}
#[cfg(windows)]
use ffmpeg_platform as platform;

#[cfg(not(windows))]
mod ffmpeg_platform_nw {
    use super::{Decoder, Result};
    use std::path::Path;
    pub fn open(path: &Path, output_size: (u32, u32), audio_rate: u32) -> Result<Box<dyn Decoder>> {
        super::super::ffmpeg::FfmpegDecoder::open(path, output_size, audio_rate)
            .map(|d| Box::new(d) as Box<dyn Decoder>)
    }
    pub fn open_video_only(path: &Path, output_size: (u32, u32)) -> Result<Box<dyn Decoder>> {
        super::super::ffmpeg::FfmpegDecoder::open_video_only(path, output_size)
            .map(|d| Box::new(d) as Box<dyn Decoder>)
    }
    pub fn is_available() -> bool {
        super::super::ffmpeg::is_available()
    }
}
#[cfg(not(windows))]
use ffmpeg_platform_nw as platform;

/// Backend inactivo para las plataformas sin decodificador.
///
/// Existe para que `core::video` compile y se testee igual en Linux y macOS.
#[cfg(not(windows))]
pub mod stub {
    use super::{Decoder, Result};
    use crate::utils::errors::ShImagesError;
    use std::path::Path;

    pub fn open(
        path: &Path,
        _output_size: (u32, u32),
        _audio_rate: u32,
    ) -> Result<Box<dyn Decoder>> {
        Err(ShImagesError::Media(format!(
            "reproducción de video no disponible en esta plataforma: {}",
            path.display()
        )))
    }

    pub fn open_video_only(path: &Path, _output_size: (u32, u32)) -> Result<Box<dyn Decoder>> {
        Err(ShImagesError::Media(format!(
            "reproducción de video no disponible en esta plataforma: {}",
            path.display()
        )))
    }

    pub fn is_available() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn unsupported_platform_reports_unavailable() {
        assert!(!is_available());
    }

    #[test]
    #[cfg(not(windows))]
    fn unsupported_platform_fails_to_open_instead_of_panicking() {
        use crate::utils::errors::ShImagesError;
        let err = open(Path::new("clip.mp4"), (1920, 1080), 48_000)
            .err()
            .expect("abrir video debe fallar sin backend");
        assert!(matches!(err, ShImagesError::Media(_)));
    }

    #[test]
    fn sample_variants_carry_flat_data_only() {
        // Documenta la invariante: nada de lo que cruza la frontera lleva
        // punteros con afinidad de hilo.
        let s = Sample::Audio {
            pts: Duration::from_millis(20),
            samples: vec![0.0f32; 4],
        };
        match s {
            Sample::Audio { pts, samples } => {
                assert_eq!(pts, Duration::from_millis(20));
                assert_eq!(samples.len(), 4);
            }
            _ => panic!("variante equivocada"),
        }
    }

    /// Backend mínimo para probar el contrato del trait sin tocar COM.
    struct StubDecoder {
        info: VideoInfo,
    }

    impl Decoder for StubDecoder {
        fn info(&self) -> VideoInfo {
            self.info
        }

        fn next_sample(&mut self) -> Result<Sample> {
            Ok(Sample::EndOfStream)
        }

        fn seek(&mut self, _target: Duration) -> Result<()> {
            Ok(())
        }

        fn set_output_size(&mut self, _width: u32, _height: u32) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn default_refresh_duration_returns_cached_info_duration() {
        // Los backends que no sobrescriben `refresh_duration` devuelven lo ya
        // conocido; los que sí (mf) actualizan y reportan la nueva duración.
        let mut decoder = StubDecoder {
            info: VideoInfo {
                duration: Some(Duration::from_secs(90)),
                ..VideoInfo::default()
            },
        };
        assert_eq!(decoder.refresh_duration(), Some(Duration::from_secs(90)));
        assert_eq!(decoder.info().duration, Some(Duration::from_secs(90)));
    }

    #[test]
    fn default_refresh_duration_reports_none_when_duration_unknown() {
        let mut decoder = StubDecoder {
            info: VideoInfo::default(),
        };
        assert_eq!(decoder.refresh_duration(), None);
    }
}
