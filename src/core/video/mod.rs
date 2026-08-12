//! Lógica de reproducción de video.
//!
//! Todos los módulos de aquí son **puros**: sin `Instant::now()`, sin I/O, sin
//! `#[cfg]` y sin egui. Reciben el tiempo y el estado como argumentos, igual
//! que `core::slideshow`. Eso permite testear la máquina de estados, la
//! aritmética de seek, el volumen y la política de presentación de frames en
//! las tres plataformas sin decodificar un solo byte.
//!
//! Las excepciones son `backend` (la frontera, con dos ramas `cfg`) y `mf`
//! (Media Foundation, sólo Windows). El backend nativo sólo produce datos
//! planos —marcas temporales, píxeles, muestras PCM— y no toma ninguna
//! decisión.

pub mod audio;
pub mod autoplay;
pub mod backend;
pub mod clock;
pub mod format;
#[cfg(windows)]
pub mod mf;
pub mod playback;
pub mod player;
pub mod ring;
pub mod seek;
pub mod thumbnail;
pub mod timeline;
pub mod volume;

use std::time::Duration;

pub use autoplay::AutoplayDecision;
pub use playback::{PlaybackEvent, PlaybackState};
pub use player::{PlayerSnapshot, VideoPlayer};
pub use ring::{FrameRing, VideoFrame};
pub use timeline::FrameDecision;

/// Metadatos de un video, tal como los reporta el backend al abrirlo.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    /// `None` si el contenedor no la declara o el índice está corrupto.
    pub duration: Option<Duration>,
    pub fps: Option<f32>,
    pub has_audio: bool,
    /// Rotación declarada en el contenedor, en cuartos de vuelta (0-3).
    /// Los videos verticales de móvil vienen girados por metadato.
    pub container_rotation: u8,
    /// `false` en streams no indexados: hay que deshabilitar los controles de
    /// avance y retroceso.
    pub can_seek: bool,
}

/// Rotación total a aplicar: la del usuario más la del contenedor.
pub fn effective_rotation(user: u8, container: u8) -> u8 {
    (user + container) % 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_rotation_wraps_at_four_quarters() {
        assert_eq!(effective_rotation(0, 0), 0);
        assert_eq!(effective_rotation(1, 0), 1);
        assert_eq!(effective_rotation(0, 1), 1);
        assert_eq!(effective_rotation(2, 3), 1);
        assert_eq!(effective_rotation(3, 3), 2);
    }

    #[test]
    fn effective_rotation_handles_out_of_range_input() {
        assert_eq!(effective_rotation(7, 0), 3);
        assert_eq!(effective_rotation(0, 250), 2);
    }

    #[test]
    fn default_video_info_is_conservative() {
        let info = VideoInfo::default();
        assert!(
            !info.has_audio,
            "sin audio hasta que el backend lo confirme"
        );
        assert!(!info.can_seek, "sin seek hasta que el backend lo confirme");
        assert_eq!(info.duration, None);
        assert_eq!(info.container_rotation, 0);
    }
}
