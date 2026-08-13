//! Clasificación de archivos por tipo de medio.
//!
//! `Navigation` lista imágenes y videos juntos, pero el resto del pipeline
//! (miniaturas, precarga, EXIF, decodificación) sólo sabe tratar imágenes.
//! Este módulo es el punto único donde se decide qué es cada archivo, para que
//! esos consumidores puedan filtrar en vez de intentar decodificar un `.mp4`
//! con el crate `image` y fallar tres veces por archivo.

use std::path::Path;

/// Tipo de medio deducido de la extensión del archivo.
///
/// La clasificación es por extensión, no por contenido: validar que el archivo
/// sea reproducible es responsabilidad del backend de video, que lo hace al
/// abrirlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Image,
    Video,
}

/// Extensiones decodificables por el crate `image`.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif", "avif",
];

/// Extensiones de video soportadas.
///
/// Conjunto conservador: contenedores que Media Foundation maneja de fábrica en
/// una instalación limpia de Windows. MKV y WebM dependen de extensiones de la
/// Store que pueden no estar instaladas, así que no se prometen por extensión.
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mov"];

/// Clasifica un archivo por su extensión. `None` si no es un medio soportado.
pub fn media_kind(path: &Path) -> Option<MediaKind> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    if IMAGE_EXTENSIONS.iter().any(|s| s.eq_ignore_ascii_case(ext)) {
        return Some(MediaKind::Image);
    }
    if VIDEO_EXTENSIONS.iter().any(|s| s.eq_ignore_ascii_case(ext)) {
        return Some(MediaKind::Video);
    }
    None
}

/// `true` si el archivo es un video según su extensión.
pub fn is_video(path: &Path) -> bool {
    media_kind(path) == Some(MediaKind::Video)
}

/// `true` si el archivo es una imagen según su extensión.
pub fn is_image(path: &Path) -> bool {
    media_kind(path) == Some(MediaKind::Image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::navigation::SUPPORTED_EXTENSIONS;
    use std::path::PathBuf;

    #[test]
    fn mp4_is_video() {
        assert_eq!(
            media_kind(Path::new("a/b/clip.mp4")),
            Some(MediaKind::Video)
        );
        assert!(is_video(Path::new("clip.mp4")));
        assert!(!is_image(Path::new("clip.mp4")));
    }

    #[test]
    fn png_is_image() {
        assert_eq!(
            media_kind(Path::new("a/b/foto.png")),
            Some(MediaKind::Image)
        );
        assert!(is_image(Path::new("foto.png")));
        assert!(!is_video(Path::new("foto.png")));
    }

    #[test]
    fn extension_match_is_case_insensitive() {
        assert_eq!(media_kind(Path::new("CLIP.MP4")), Some(MediaKind::Video));
        assert_eq!(media_kind(Path::new("FOTO.JPG")), Some(MediaKind::Image));
        assert_eq!(media_kind(Path::new("clip.MoV")), Some(MediaKind::Video));
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert_eq!(media_kind(Path::new("notas.txt")), None);
        assert_eq!(media_kind(Path::new("audio.mp3")), None);
        assert_eq!(media_kind(Path::new("video.mkv")), None);
    }

    #[test]
    fn file_without_extension_returns_none() {
        assert_eq!(media_kind(Path::new("README")), None);
        assert_eq!(media_kind(Path::new("")), None);
    }

    #[test]
    fn dotfile_named_mp4_is_not_video() {
        // Path::extension() devuelve None para ".mp4": es un archivo oculto
        // sin extensión, no un video. Se fija aquí para que nadie lo "arregle".
        assert_eq!(Path::new(".mp4").extension(), None);
        assert_eq!(media_kind(Path::new(".mp4")), None);
    }

    #[test]
    fn video_and_image_extension_sets_are_disjoint() {
        for v in VIDEO_EXTENSIONS {
            assert!(
                !IMAGE_EXTENSIONS.iter().any(|i| i.eq_ignore_ascii_case(v)),
                "la extensión {v} está en ambas listas"
            );
        }
    }

    #[test]
    fn supported_extensions_is_union_of_image_and_video() {
        // SUPPORTED_EXTENSIONS se escribe a mano (los slices const no se
        // concatenan en const fn). Este test impide que se desincronice.
        let mut union: Vec<&str> = IMAGE_EXTENSIONS
            .iter()
            .chain(VIDEO_EXTENSIONS.iter())
            .copied()
            .collect();
        union.sort_unstable();
        let mut supported: Vec<&str> = SUPPORTED_EXTENSIONS.to_vec();
        supported.sort_unstable();
        assert_eq!(
            supported, union,
            "SUPPORTED_EXTENSIONS debe ser exactamente IMAGE_EXTENSIONS + VIDEO_EXTENSIONS"
        );
    }

    #[test]
    fn every_supported_extension_has_a_media_kind() {
        for ext in SUPPORTED_EXTENSIONS {
            let path = PathBuf::from(format!("archivo.{ext}"));
            assert!(
                media_kind(&path).is_some(),
                "{ext} está en SUPPORTED_EXTENSIONS pero media_kind no la clasifica"
            );
        }
    }
}
