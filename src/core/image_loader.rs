//! Carga y decodificación síncrona de imágenes.

use std::path::Path;

use image::{DynamicImage, GenericImageView, ImageReader};

use crate::utils::errors::{Result, ShImagesError};

/// Un frame de una imagen animada: buffer RGBA ya compuesto + retardo.
#[derive(Debug)]
pub struct AnimatedFrame {
    pub image: DynamicImage,
    pub delay: std::time::Duration,
}

/// Imagen animada (GIF): frames en orden de reproducción y duración total.
///
/// El loader garantiza `frames` no vacío y `total_duration > 0`.
#[derive(Debug)]
pub struct AnimatedImage {
    pub frames: Vec<AnimatedFrame>,
    pub total_duration: std::time::Duration,
}

/// Imagen decodificada: estática o animada.
#[derive(Debug)]
pub enum LoadedImage {
    Static(DynamicImage),
    Animated(AnimatedImage),
}

impl From<DynamicImage> for LoadedImage {
    fn from(image: DynamicImage) -> Self {
        LoadedImage::Static(image)
    }
}

impl LoadedImage {
    /// `true` si la imagen tiene animación (varios frames).
    pub fn is_animated(&self) -> bool {
        matches!(self, LoadedImage::Animated(_))
    }

    /// Dimensiones de la imagen (las del primer frame; todos comparten tamaño).
    pub fn dimensions(&self) -> (u32, u32) {
        self.first_frame().dimensions()
    }

    /// Primer frame (imagen completa para `Static`).
    pub fn first_frame(&self) -> &DynamicImage {
        match self {
            LoadedImage::Static(img) => img,
            LoadedImage::Animated(anim) => &anim.frames[0].image,
        }
    }
}

/// Carga y decodifica una imagen desde el filesystem.
///
/// Por ahora devuelve el primer frame como `Static` (la rama GIF animado se
/// añade en la Task 2).
pub fn load_image(path: &Path) -> Result<LoadedImage> {
    let reader = ImageReader::open(path)?;
    let reader = reader.with_guessed_format()?;
    let image = reader.decode().map_err(map_image_error)?;
    Ok(LoadedImage::Static(image))
}

fn map_image_error(e: image::ImageError) -> ShImagesError {
    match e {
        image::ImageError::IoError(io) => ShImagesError::Io(io),
        image::ImageError::Unsupported(msg) => ShImagesError::UnsupportedFormat(msg.to_string()),
        other => ShImagesError::Decode(other.to_string()),
    }
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
    fn decoding_valid_png_returns_static_image() {
        let img = load_image(&fixture()).unwrap();
        assert_eq!(img.dimensions(), (1, 1));
        assert!(!img.is_animated());
        assert_eq!(img.first_frame().width(), 1);
    }

    #[test]
    fn decoding_valid_jpeg_returns_static_image() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample.jpg");
        let img = load_image(&path).unwrap();
        assert_eq!(img.dimensions(), (16, 16));
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
