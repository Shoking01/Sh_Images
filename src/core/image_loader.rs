//! Carga y decodificación síncrona de imágenes.

use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, GenericImageView, ImageFormat, ImageReader};

use crate::utils::errors::{Result, ShImagesError};

/// Un frame de una imagen animada: buffer RGBA ya compuesto + retardo.
#[derive(Debug)]
pub struct AnimatedFrame {
    pub image: DynamicImage,
    pub delay: Duration,
}

/// Imagen animada (GIF): frames en orden de reproducción y duración total.
///
/// El loader garantiza `frames` no vacío y `total_duration > 0`.
#[derive(Debug)]
pub struct AnimatedImage {
    pub frames: Vec<AnimatedFrame>,
    pub total_duration: Duration,
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

    /// Índice del frame activo para `elapsed` (bucle infinito sobre la
    /// duración total).
    pub fn frame_index_at(&self, elapsed: Duration) -> usize {
        match self {
            LoadedImage::Static(_) => 0,
            LoadedImage::Animated(anim) => {
                let total_ms = anim.total_duration.as_millis();
                if total_ms == 0 {
                    return 0;
                }
                let mut t = elapsed.as_millis() % total_ms;
                for (i, frame) in anim.frames.iter().enumerate() {
                    if t < frame.delay.as_millis() {
                        return i;
                    }
                    t -= frame.delay.as_millis();
                }
                anim.frames.len().saturating_sub(1)
            }
        }
    }

    /// Imagen del frame activo para `elapsed`.
    pub fn frame_at(&self, elapsed: Duration) -> &DynamicImage {
        match self {
            LoadedImage::Static(img) => img,
            LoadedImage::Animated(anim) => &anim.frames[self.frame_index_at(elapsed)].image,
        }
    }

    /// Tiempo restante hasta el próximo cambio de frame (para programar
    /// repaints). En `Static` devuelve una hora (no hay animación).
    pub fn time_to_next_frame(&self, elapsed: Duration) -> Duration {
        match self {
            LoadedImage::Static(_) => Duration::from_secs(3600),
            LoadedImage::Animated(anim) => {
                let total_ms = anim.total_duration.as_millis();
                if total_ms == 0 {
                    return MIN_FRAME_DELAY;
                }
                let t = elapsed.as_millis() % total_ms;
                let mut cum = 0u128;
                for frame in &anim.frames {
                    let d = frame.delay.as_millis();
                    if t < cum + d {
                        let remaining = (cum + d - t) as u64;
                        return Duration::from_millis(remaining.max(1));
                    }
                    cum += d;
                }
                anim.frames
                    .first()
                    .map(|f| f.delay)
                    .unwrap_or(MIN_FRAME_DELAY)
            }
        }
    }
}

/// Retardo mínimo de un frame animado; evita busy-loop de repaint con delay 0.
pub const MIN_FRAME_DELAY: Duration = Duration::from_millis(20);

/// Carga y decodifica una imagen desde el filesystem.
///
/// Devuelve `LoadedImage::Static` para formatos sin animación (o un GIF de un
/// solo frame) y `LoadedImage::Animated` para GIFs animados (frames ya
/// compuestos con sus retardos).
pub fn load_image(path: &Path) -> Result<LoadedImage> {
    let reader = ImageReader::open(path)?;
    let reader = reader.with_guessed_format()?;
    if reader.format() == Some(ImageFormat::Gif) {
        return load_animated_gif(path);
    }
    let image = reader.decode().map_err(map_image_error)?;
    Ok(LoadedImage::Static(image))
}

/// Decodifica un GIF animado completo.
///
/// Un GIF de un solo frame se devuelve como `Static` (no hay nada que animar).
fn load_animated_gif(path: &Path) -> Result<LoadedImage> {
    let file = std::fs::File::open(path)?;
    let decoder = GifDecoder::new(BufReader::new(file)).map_err(map_image_error)?;
    let mut frames = Vec::new();
    for frame in decoder.into_frames() {
        let frame = frame.map_err(map_image_error)?;
        let delay = clamp_delay(Duration::from(frame.delay()));
        frames.push(AnimatedFrame {
            image: DynamicImage::ImageRgba8(frame.into_buffer()),
            delay,
        });
    }
    match frames.len() {
        0 => Err(ShImagesError::Decode("gif sin frames".to_string())),
        1 => Ok(LoadedImage::Static(frames.remove(0).image)),
        _ => {
            let total_duration = frames.iter().map(|f| f.delay).sum();
            Ok(LoadedImage::Animated(AnimatedImage {
                frames,
                total_duration,
            }))
        }
    }
}

/// Clamp del retardo: un delay 0 (muy común en GIFs) no debe provocar un
/// repaint por frame infinito.
fn clamp_delay(d: Duration) -> Duration {
    if d < MIN_FRAME_DELAY {
        MIN_FRAME_DELAY
    } else {
        d
    }
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

    /// Genera un GIF animado sintético con `delays_ms` retardos por frame.
    fn make_animated_gif(dir: &Path, delays_ms: &[u64]) -> std::path::PathBuf {
        use image::codecs::gif::GifEncoder;
        use image::{Delay, Frame};
        let path = dir.join("animated.gif");
        let mut out = std::fs::File::create(&path).expect("crear gif");
        let mut encoder = GifEncoder::new(&mut out);
        let frames = delays_ms.iter().map(|&ms| {
            let buf = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
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

    #[test]
    fn read_animated_gif_yields_frames_and_total() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[100, 200]);
        let loaded = load_image(&path).unwrap();
        let LoadedImage::Animated(anim) = &loaded else {
            panic!("gif de 2 frames debe ser Animated");
        };
        assert_eq!(anim.frames.len(), 2);
        assert_eq!(anim.frames[0].delay, Duration::from_millis(100));
        assert_eq!(anim.frames[1].delay, Duration::from_millis(200));
        assert_eq!(anim.total_duration, Duration::from_millis(300));
        assert!(loaded.is_animated());
        assert_eq!(loaded.first_frame().dimensions(), (4, 4));
    }

    #[test]
    fn single_frame_gif_is_static() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[100]);
        let loaded = load_image(&path).unwrap();
        assert!(!loaded.is_animated());
        assert!(matches!(loaded, LoadedImage::Static(_)));
    }

    #[test]
    fn zero_delay_is_clamped_to_minimum() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[0, 0]);
        let loaded = load_image(&path).unwrap();
        let LoadedImage::Animated(anim) = &loaded else {
            panic!("gif de 2 frames debe ser Animated");
        };
        assert_eq!(anim.frames[0].delay, MIN_FRAME_DELAY);
        assert_eq!(anim.frames[1].delay, MIN_FRAME_DELAY);
    }

    #[test]
    fn corrupt_gif_returns_decode_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("corrupt.gif");
        std::fs::write(&path, b"GIF89a not really a gif").expect("escribir");
        let err = load_image(&path).expect_err("gif corrupto da error");
        assert!(matches!(
            err,
            ShImagesError::Decode(_) | ShImagesError::Io(_)
        ));
    }

    #[test]
    fn frame_selection_wraps_around_total() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[100, 200]);
        let loaded = load_image(&path).unwrap();
        assert_eq!(loaded.frame_index_at(Duration::from_millis(0)), 0);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(99)), 0);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(100)), 1);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(299)), 1);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(300)), 0);
        assert_eq!(loaded.frame_index_at(Duration::from_millis(450)), 1); // 450%300=150
    }

    #[test]
    fn time_to_next_frame_is_remaining_of_current() {
        let dir = tempdir().unwrap();
        let path = make_animated_gif(dir.path(), &[100, 200]);
        let loaded = load_image(&path).unwrap();
        assert_eq!(
            loaded.time_to_next_frame(Duration::from_millis(0)),
            Duration::from_millis(100)
        );
        assert_eq!(
            loaded.time_to_next_frame(Duration::from_millis(50)),
            Duration::from_millis(50)
        );
        assert_eq!(
            loaded.time_to_next_frame(Duration::from_millis(100)),
            Duration::from_millis(200)
        );
        assert_eq!(
            loaded.time_to_next_frame(Duration::from_millis(250)),
            Duration::from_millis(50)
        );
    }
}
