//! Extracción de un fotograma reducido para el sidebar.

use std::path::Path;

use image::{DynamicImage, RgbaImage};

use super::backend::{self, Sample};
use super::ring::VideoFrame;
use crate::utils::errors::{Result, ShImagesError};

/// Cambios de formato sin muestra que toleramos antes de considerar que el
/// contenedor no puede entregar una miniatura.
const MAX_EMPTY_SAMPLES: usize = 64;

/// Decodifica el primer fotograma del video a una imagen RGBA acotada.
pub fn generate(path: &Path, max: u32) -> Result<DynamicImage> {
    if max == 0 {
        return Err(ShImagesError::Media(
            "el tamaño máximo de la miniatura debe ser mayor que cero".to_string(),
        ));
    }

    let mut decoder = backend::open_video_only(path, (max, max))?;
    let rotation = decoder.info().container_rotation;
    let mut empty_samples = 0;

    loop {
        match decoder.next_sample()? {
            Sample::Video(frame) => return image_from_frame(frame, rotation),
            Sample::Audio { samples, .. } if samples.is_empty() => {
                empty_samples += 1;
                if empty_samples >= MAX_EMPTY_SAMPLES {
                    return Err(ShImagesError::Media(
                        "el video no entregó ningún fotograma para la miniatura".to_string(),
                    ));
                }
            }
            Sample::Audio { .. } => {
                // La apertura video-only no selecciona audio. Se tolera por
                // robustez ante backends que entreguen una muestra residual.
            }
            Sample::EndOfStream => {
                return Err(ShImagesError::Media(
                    "el video terminó sin entregar ningún fotograma".to_string(),
                ));
            }
        }
    }
}

fn image_from_frame(frame: VideoFrame, rotation: u8) -> Result<DynamicImage> {
    if !frame.is_consistent() {
        return Err(ShImagesError::Media(
            "el fotograma de miniatura tiene un buffer inconsistente".to_string(),
        ));
    }
    let image = RgbaImage::from_raw(frame.width, frame.height, frame.pixels).ok_or_else(|| {
        ShImagesError::Media("no se pudo construir la miniatura del video".to_string())
    })?;
    let image = DynamicImage::ImageRgba8(image);
    Ok(match rotation % 4 {
        1 => image.rotate90(),
        2 => image.rotate180(),
        3 => image.rotate270(),
        _ => image,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use image::GenericImageView;

    use super::*;

    fn frame(width: u32, height: u32, pixels: Vec<u8>) -> VideoFrame {
        VideoFrame {
            pts: Duration::ZERO,
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn frame_conversion_preserves_dimensions_and_rgba() {
        let pixels = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let image = image_from_frame(frame(2, 1, pixels), 0).expect("fotograma válido");
        assert_eq!(image.dimensions(), (2, 1));
        assert_eq!(image.to_rgba8().as_raw(), &[255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn container_rotation_is_applied_to_thumbnail() {
        let image = image_from_frame(frame(4, 2, vec![0; 4 * 2 * 4]), 1).expect("fotograma válido");
        assert_eq!(image.dimensions(), (2, 4));
    }

    #[test]
    fn inconsistent_frame_is_rejected() {
        let result = image_from_frame(frame(2, 2, vec![0; 3]), 0);
        assert!(result.is_err());
    }

    #[test]
    fn zero_max_is_rejected_without_opening_backend() {
        assert!(generate(Path::new("cualquier.mp4"), 0).is_err());
    }

    /// Verificación manual con un contenedor real. Se mantiene fuera de CI
    /// porque el repositorio no incluye videos ni codecs empaquetados.
    #[test]
    #[cfg(windows)]
    #[ignore = "necesita SH_IMAGES_TEST_VIDEO apuntando a un video real"]
    fn real_video_yields_a_bounded_thumbnail() {
        let path = std::env::var_os("SH_IMAGES_TEST_VIDEO")
            .map(std::path::PathBuf::from)
            .expect("define SH_IMAGES_TEST_VIDEO con la ruta de un video");
        let image = generate(&path, 96).expect("extraer miniatura real");
        let (width, height) = image.dimensions();
        assert!(width > 0 && height > 0);
        assert!(width <= 96 && height <= 96, "miniatura: {width}x{height}");
    }
}
