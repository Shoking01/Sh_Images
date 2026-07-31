//! Carga y decodificación síncrona de imágenes.
//!
//! En Fase 0 se implementa la variante síncrona mínima para el benchmark base.
//! La carga asíncrona (threads worker) llega en Fase 2.

use std::path::Path;

use image::DynamicImage;

use crate::utils::errors::{Result, ShImagesError};

/// Carga y decodifica una imagen desde el filesystem.
///
/// # Arguments
/// * `path` - Ruta absoluta al archivo de imagen.
///
/// # Returns
/// * `Ok(DynamicImage)` si la decodificación fue exitosa.
/// * `Err(ShImagesError::Io)` si hay problemas de lectura del filesystem.
/// * `Err(ShImagesError::UnsupportedFormat)` si la extensión no corresponde a un formato soportado.
/// * `Err(ShImagesError::Decode)` si el archivo está corrupto.
pub fn load_image(path: &Path) -> Result<DynamicImage> {
    image::open(path).map_err(|e| match e {
        image::ImageError::IoError(io) => ShImagesError::Io(io),
        image::ImageError::Unsupported(msg) => ShImagesError::UnsupportedFormat(msg.to_string()),
        other => ShImagesError::Decode(other.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample.png")
    }

    #[test]
    fn decoding_valid_png_returns_image() {
        let img = load_image(&fixture()).unwrap();
        assert_eq!(img.width(), 1);
        assert_eq!(img.height(), 1);
    }

    #[test]
    fn decoding_valid_jpeg_returns_image() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample.jpg");
        let img = load_image(&path).unwrap();
        assert_eq!(img.width(), 16);
        assert_eq!(img.height(), 16);
    }

    #[test]
    fn loading_missing_file_returns_io_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does_not_exist.png");
        let err = load_image(&path).unwrap_err();
        assert!(matches!(err, ShImagesError::Io(_)));
    }

    #[test]
    fn truncated_png_returns_decode_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("truncated.png");
        let bytes = fs::read(fixture()).unwrap();
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();

        let err = load_image(&path).unwrap_err();
        assert!(matches!(
            err,
            ShImagesError::Decode(_) | ShImagesError::Io(_)
        ));
    }

    #[test]
    fn garbage_content_with_png_extension_returns_decode_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("random.png");
        fs::write(&path, b"this is definitely not an image").unwrap();

        let err = load_image(&path).unwrap_err();
        assert!(matches!(err, ShImagesError::Decode(_)));
    }

    #[test]
    fn unknown_extension_returns_unsupported_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("random.xyz");
        fs::write(&path, b"this is definitely not an image").unwrap();

        let err = load_image(&path).unwrap_err();
        assert!(matches!(err, ShImagesError::UnsupportedFormat(_)));
    }
}
