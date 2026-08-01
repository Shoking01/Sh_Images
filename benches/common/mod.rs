//! Helpers compartidos para generar imágenes sintéticas en los benchmarks.
//!
//! Las imágenes se generan en memoria (patrón de gradiente determinista) y se
//! guardan en un directorio temporal: repo limpio, reproducible, sin fixtures
//! grandes commiteados (AGENTS.md §8.2 limita fixtures a <100KB).

use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

/// Resoluciones objetivo de los benchmarks (AGENTS.md §6.2).
pub const RES_1080P: (u32, u32) = (1920, 1080);
/// Resolución 4K (UHD).
pub const RES_4K: (u32, u32) = (3840, 2160);
/// Resolución 8K (UHD).
pub const RES_8K: (u32, u32) = (7680, 4320);

/// Crea una imagen de gradiente determinista con las dimensiones dadas.
///
/// Patrón: `r = x % 256`, `g = y % 256`, `b = (x + y) % 256`, alpha = 255.
/// El patrón es reproducible en cada ejecución (sin ruido aleatorio).
pub fn gradient_image(w: u32, h: u32) -> DynamicImage {
    let mut img = RgbaImage::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgba([
            (x % 256) as u8,
            (y % 256) as u8,
            ((x + y) % 256) as u8,
            255,
        ]);
    }
    DynamicImage::ImageRgba8(img)
}

/// Genera y guarda una imagen sintética en `dir`, devolviendo su ruta.
///
/// El nombre de archivo codifica el formato y la resolución para que los
/// benchmarks que comparten un directorio no colisionen.
pub fn synthetic_image_path(dir: &Path, w: u32, h: u32, format: ImageFormat) -> PathBuf {
    let ext = format
        .extensions_str()
        .first()
        .expect("cada ImageFormat tiene al menos una extensión");
    let path = dir.join(format!("synthetic_{w}x{h}.{ext}"));
    gradient_image(w, h)
        .save_with_format(&path, format)
        .expect("guardar imagen sintética en temp debe funcionar");
    path
}
