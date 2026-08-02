//! Generación de miniaturas: downscale puro sin I/O ni threads.
//!
//! `core/` no depende de `egui` (AGENTS.md §3.2). La `DynamicImage` viene del
//! crate `image`, ya presente.

use image::{DynamicImage, GenericImageView};

/// Tamaño por defecto del lado mayor de una miniatura (px).
pub const THUMB_MAX: u32 = 96;

/// Devuelve el tamaño de miniatura manteniendo el aspect ratio.
///
/// Nunca amplía: si la imagen ya cabe en `max`, devuelve las dimensiones
/// originales. `(0, 0)` si `max`, `w` o `h` es `0`.
///
/// Se calcula en `f64` para evitar overflow en dimensiones grandes.
pub fn thumbnail_size(w: u32, h: u32, max: u32) -> (u32, u32) {
    if max == 0 || w == 0 || h == 0 {
        return (0, 0);
    }
    let (w, h) = (w as f64, h as f64);
    let max = max as f64;
    if w <= max && h <= max {
        return (w as u32, h as u32);
    }
    let scale = max / w.max(h);
    let nw = (w * scale).round() as u32;
    let nh = (h * scale).round() as u32;
    (nw.max(1), nh.max(1))
}

/// Genera una miniatura de `image` con el lado mayor = `max`.
///
/// Con `max == 0` devuelve la imagen original sin modificar: `DynamicImage::thumbnail`
/// con dimensión 0 no está definida y podría panic. Igual si la imagen ya cabe.
pub fn generate_thumbnail(image: &DynamicImage, max: u32) -> DynamicImage {
    let (w, h) = image.dimensions();
    let (nw, nh) = thumbnail_size(w, h, max);
    if nw == 0 || (nw == w && nh == h) {
        return image.clone();
    }
    image.thumbnail(nw, nh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, RgbaImage};

    fn rgba(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(w, h))
    }

    #[test]
    fn thumbnail_size_keeps_aspect_ratio_wide() {
        assert_eq!(thumbnail_size(1920, 1080, THUMB_MAX), (96, 54));
    }

    #[test]
    fn thumbnail_size_keeps_aspect_ratio_tall() {
        assert_eq!(thumbnail_size(1080, 1920, THUMB_MAX), (54, 96));
    }

    #[test]
    fn thumbnail_size_keeps_aspect_ratio_square() {
        assert_eq!(thumbnail_size(100, 100, THUMB_MAX), (96, 96));
    }

    #[test]
    fn thumbnail_size_does_not_upscale() {
        assert_eq!(thumbnail_size(50, 30, THUMB_MAX), (50, 30));
    }

    #[test]
    fn thumbnail_size_zero_max_returns_zero() {
        assert_eq!(thumbnail_size(1920, 1080, 0), (0, 0));
    }

    #[test]
    fn thumbnail_size_zero_dimension_returns_zero() {
        assert_eq!(thumbnail_size(0, 1080, THUMB_MAX), (0, 0));
        assert_eq!(thumbnail_size(1920, 0, THUMB_MAX), (0, 0));
    }

    #[test]
    fn thumbnail_size_never_returns_zero_for_small_dimensions() {
        assert_eq!(thumbnail_size(1, 1, THUMB_MAX), (1, 1));
        assert_eq!(thumbnail_size(1920, 1, THUMB_MAX), (96, 1));
    }

    #[test]
    fn generate_thumbnail_downscales_to_max() {
        let img = generate_thumbnail(&rgba(1920, 1080), THUMB_MAX);
        assert_eq!(img.dimensions(), (96, 54));
    }

    #[test]
    fn generate_thumbnail_small_image_unchanged() {
        let img = generate_thumbnail(&rgba(50, 30), THUMB_MAX);
        assert_eq!(img.dimensions(), (50, 30));
    }

    #[test]
    fn generate_thumbnail_zero_max_returns_original() {
        let img = generate_thumbnail(&rgba(1920, 1080), 0);
        assert_eq!(img.dimensions(), (1920, 1080));
    }
}
