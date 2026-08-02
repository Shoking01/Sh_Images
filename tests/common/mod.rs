//! Helpers compartidos para los tests de integración.
//!
//! Generan carpetas temp con imágenes sintéticas y archivos de error
//! (corrupto/vacío), para no committear fixtures grandes (AGENTS.md §8.2).

use std::fs;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use tempfile::tempdir;

/// Crea una imagen de gradiente determinista (misma lógica que benches/common).
pub fn gradient_image(w: u32, h: u32) -> DynamicImage {
    let mut img = RgbaImage::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]);
    }
    DynamicImage::ImageRgba8(img)
}

/// Crea una carpeta temp con `n` imágenes `.jpg` sintéticas de 64x64.
///
/// Devuelve `(TempDir, rutas ordenadas)`. `TempDir` se elimina al dropear.
pub fn make_folder_with_images(n: usize) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempdir().expect("tempdir en test");
    let mut paths = Vec::with_capacity(n);
    for i in 0..n {
        let path = dir.path().join(format!("img_{i:04}.jpg"));
        gradient_image(64, 64)
            .save_with_format(&path, ImageFormat::Jpeg)
            .expect("guardar imagen sintética");
        paths.push(path);
    }
    paths.sort();
    (dir, paths)
}

/// Escribe un archivo `.png` con bytes que NO son una imagen válida.
pub fn corrupt_png_path(dir: &Path) -> PathBuf {
    let path = dir.join("corrupt.png");
    fs::write(&path, b"this is definitely not a valid png file").expect("escribir corrupto");
    path
}

/// Escribe un archivo `.png` vacío (0 bytes).
pub fn empty_png_path(dir: &Path) -> PathBuf {
    let path = dir.join("empty.png");
    fs::write(&path, []).expect("escribir archivo vacío");
    path
}

/// Guarda un GIF 1x1 válido y devuelve su ruta.
pub fn gif_path(dir: &Path) -> PathBuf {
    let path = dir.join("one.gif");
    gradient_image(1, 1)
        .save_with_format(&path, ImageFormat::Gif)
        .expect("guardar gif");
    path
}
