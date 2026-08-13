//! Helpers compartidos para los tests de integración.
//!
//! Generan carpetas temp con imágenes sintéticas y archivos de error
//! (corrupto/vacío), para no committear fixtures grandes (AGENTS.md §8.2).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use image::codecs::gif::GifEncoder;
use image::{Delay, DynamicImage, Frame, ImageFormat, Rgba, RgbaImage};
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

/// Crea un `.mp4` vacío (0 bytes).
///
/// No se committea ningún video real: `image 0.25` no encodea contenedores y
/// añadir un muxer sólo para el test rompería la justificación de dependencias
/// (AGENTS.md §7.2). Toda la lógica de reproducción es aritmética pura y se
/// testea sin archivo; aquí sólo hace falta *una ruta* con extensión de video.
pub fn empty_video_path(dir: &Path) -> PathBuf {
    let path = dir.join("vacio.mp4");
    fs::write(&path, b"").expect("escribir video vacío");
    path
}

/// Crea un `.mp4` con contenido que no es un contenedor válido.
pub fn corrupt_video_path(dir: &Path) -> PathBuf {
    let path = dir.join("corrupto.mp4");
    fs::write(&path, b"esto no es un contenedor mp4").expect("escribir video corrupto");
    path
}

/// Guarda un PNG válido con extensión `.mp4`.
///
/// La clasificación es por extensión, así que esto debe seguir contando como
/// video; validar el contenido es del backend.
pub fn png_renamed_as_mp4(dir: &Path) -> PathBuf {
    let path = dir.join("disfrazado.mp4");
    gradient_image(8, 8)
        .save_with_format(&path, ImageFormat::Png)
        .expect("guardar png disfrazado");
    path
}

/// Carpeta temp con imágenes, videos y un `.txt` que debe quedar fuera.
///
/// Devuelve `(TempDir, rutas ordenadas de los medios soportados)`.
pub fn make_folder_with_mixed_media(
    images: usize,
    videos: usize,
) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempdir().expect("tempdir en test");
    let mut paths = Vec::with_capacity(images + videos);
    for i in 0..images {
        let path = dir.path().join(format!("img_{i:04}.jpg"));
        gradient_image(32, 32)
            .save_with_format(&path, ImageFormat::Jpeg)
            .expect("guardar imagen sintética");
        paths.push(path);
    }
    for i in 0..videos {
        let path = dir.path().join(format!("vid_{i:04}.mp4"));
        fs::write(&path, b"").expect("escribir video de prueba");
        paths.push(path);
    }
    // Un archivo que no debe aparecer en la navegación.
    fs::write(dir.path().join("notas.txt"), b"x").expect("escribir txt");
    paths.sort();
    (dir, paths)
}

/// Crea una carpeta temp con `n` imágenes `.jpg` sintéticas de `w`x`h`.
///
/// Devuelve `(TempDir, rutas ordenadas)`. `TempDir` se elimina al dropear.
pub fn make_folder_with_rect_images(n: usize, w: u32, h: u32) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempdir().expect("tempdir en test");
    let mut paths = Vec::with_capacity(n);
    for i in 0..n {
        let path = dir.path().join(format!("img_{i:04}.jpg"));
        gradient_image(w, h)
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

/// Guarda un GIF animado sintético con `delays_ms` retardos por frame.
pub fn make_animated_gif(dir: &Path, delays_ms: &[u64]) -> PathBuf {
    let path = dir.join("animated.gif");
    let mut out = fs::File::create(&path).expect("crear gif");
    let mut encoder = GifEncoder::new(&mut out);
    let frames = delays_ms.iter().map(|&ms| {
        let buf = gradient_image(8, 8).to_rgba8();
        Frame::from_parts(
            buf,
            0,
            0,
            Delay::from_saturating_duration(Duration::from_millis(ms)),
        )
    });
    encoder.encode_frames(frames).expect("encodificar gif");
    path
}

/// Copia un fixture committeado a un directorio temp y devuelve su ruta.
pub fn copy_fixture(dir: &Path, name: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    let dst = dir.join(name);
    fs::copy(&src, &dst).expect("copiar fixture");
    dst
}
