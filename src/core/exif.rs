//! Extracción de metadatos EXIF (Fase 4).
//!
//! `core/` no depende de la UI: expone `ExifImage` curado, `Rational` y
//! `read_exif`. Este módulo solo lee y mapea; la UI formatea.

use std::path::Path;

use exif::{In, Reader, Tag, Value};

use crate::utils::errors::{Result, ShImagesError};

/// Número racional EXIF (numerador/denominador).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rational {
    pub num: u64,
    pub den: u64,
}

impl Rational {
    /// Crea un racional.
    pub fn new(num: u64, den: u64) -> Self {
        Self { num, den }
    }

    /// Valor decimal (n/m), o `f64::NAN` si `den == 0`.
    pub fn to_f64(self) -> f64 {
        if self.den == 0 {
            f64::NAN
        } else {
            self.num as f64 / self.den as f64
        }
    }
}

/// Metadatos curados que el panel muestra.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExifImage {
    pub make: Option<String>,
    pub model: Option<String>,
    pub fecha: Option<String>,
    pub iso: Option<u32>,
    pub f_number: Option<Rational>,
    pub shutter_speed: Option<Rational>,
    pub focal_length: Option<Rational>,
    pub orientacion: Option<u16>,
}

/// Resultado de una lectura: distingue "sin EXIF" de "error".
#[derive(Debug)]
pub enum ExifRead {
    /// Metadatos leídos.
    Found(ExifImage),
    /// El archivo no tiene bloque EXIF.
    None,
    /// Error al leer/parsear.
    Error(ShImagesError),
}

/// Los formatos que pueden llevar EXIF en esta fase.
fn is_exif_capable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "tif" | "tiff"
            )
        })
        .unwrap_or(false)
}

/// Lee un campo ASCII (primer valor), p. ej. Make/Model/DateTime.
///
/// `Value::Ascii` es `Vec<Vec<u8>>`; se toma el primer elemento y se limpia.
fn ascii_value(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    if let Value::Ascii(v) = &field.value {
        v.first()
            .map(|b| String::from_utf8_lossy(b).trim().to_string())
    } else {
        None
    }
}

/// Lee un RATIONAL (primer valor) como `Rational`, si el tag es de ese tipo.
fn rational_value(exif: &exif::Exif, tag: Tag) -> Option<Rational> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    if let Value::Rational(v) = &field.value {
        v.first()
            .map(|r| Rational::new(r.num.into(), r.denom.into()))
    } else {
        None
    }
}

/// Lee un entero sin signo (BYTE/SHORT/LONG), p. ej. Orientation.
fn uint_value(exif: &exif::Exif, tag: Tag) -> Option<u32> {
    exif.get_field(tag, In::PRIMARY)?.value.get_uint(0)
}

/// ISO: algunos archivos usan `PhotographicSensitivity`, otros `ISOSpeed`.
fn iso_value(exif: &exif::Exif) -> Option<u32> {
    uint_value(exif, Tag::PhotographicSensitivity).or_else(|| uint_value(exif, Tag::ISOSpeed))
}

fn build_image(exif: &exif::Exif) -> ExifImage {
    ExifImage {
        make: ascii_value(exif, Tag::Make),
        model: ascii_value(exif, Tag::Model),
        fecha: ascii_value(exif, Tag::DateTimeOriginal)
            .or_else(|| ascii_value(exif, Tag::DateTime)),
        iso: iso_value(exif),
        f_number: rational_value(exif, Tag::FNumber),
        shutter_speed: rational_value(exif, Tag::ExposureTime),
        focal_length: rational_value(exif, Tag::FocalLength),
        orientacion: uint_value(exif, Tag::Orientation).map(|v| v as u16),
    }
}

/// Lee los metadatos EXIF de una imagen.
///
/// `Ok(Some(img))` si hay EXIF; `Ok(None)` si el archivo es de un tipo sin
/// EXIF (PNG/BMP/WebP) o es un JPEG/TIFF sin bloque EXIF; `Err` si no se puede
/// leer el archivo o su EXIF es corrupto.
pub fn read_exif(path: &Path) -> Result<Option<ExifImage>> {
    if !is_exif_capable(path) {
        return Ok(None);
    }
    let file = std::fs::File::open(path)?;
    let exif = match Reader::new().read_from_container(&mut std::io::BufReader::new(file)) {
        Ok(exif) => exif,
        // JPEG/TIFF sin bloque APP1 → el crate reporta NotFound → "sin EXIF".
        Err(exif::Error::NotFound(_)) => return Ok(None),
        Err(other) => return Err(ShImagesError::Exif(other.to_string())),
    };
    Ok(Some(build_image(&exif)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn read_exif_from_jpeg_populates_fields() {
        let img = read_exif(&fixture("exif.jpg"))
            .expect("jpg válido")
            .expect("jpg tiene EXIF");
        assert!(
            img.make.is_some() || img.model.is_some() || img.fecha.is_some(),
            "jpg con EXIF expone cámara o fecha"
        );
    }

    #[test]
    fn read_exif_from_tiff_populates_fields() {
        assert!(read_exif(&fixture("exif.tif"))
            .expect("tif válido")
            .is_some());
    }

    #[test]
    fn read_exif_from_png_returns_none() {
        assert_eq!(read_exif(&fixture("sample.png")).expect("ok"), None);
    }

    #[test]
    fn read_exif_from_jpeg_without_exif_returns_none() {
        assert_eq!(read_exif(&fixture("sample.jpg")).expect("ok"), None);
    }

    #[test]
    fn read_exif_corrupt_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("corrupt.jpg");
        std::fs::write(&p, b"definitely not a jpeg").expect("escribir");
        let err = read_exif(&p).expect_err("corrupto da error");
        assert!(matches!(err, ShImagesError::Exif(_) | ShImagesError::Io(_)));
    }

    #[test]
    fn rational_turns_decimal() {
        assert_eq!(Rational::new(1, 2).to_f64(), 0.5);
        assert!(Rational::new(1, 0).to_f64().is_nan());
    }

    #[test]
    fn is_exif_capable_checks_extensions() {
        assert!(is_exif_capable(Path::new("a.JPG")));
        assert!(is_exif_capable(Path::new("a.TIFF")));
        assert!(!is_exif_capable(Path::new("a.PNG")));
    }
}
