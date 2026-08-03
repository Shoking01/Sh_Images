//! Tipos de error centralizados de Sh_Images.

use std::io;

/// Error global de Sh_Images.
///
/// Todos los errores del proyecto convergen en este tipo para que la UI y el
/// logging manejen un único tipo de fallo.
#[derive(Debug, thiserror::Error)]
pub enum ShImagesError {
    /// Error de entrada/salida (lectura/escritura de archivos).
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    /// Error de configuración (lectura, escritura o parseo).
    #[error("configuration error: {0}")]
    Config(String),
    /// Error decodificando una imagen.
    #[error("decode error: {0}")]
    Decode(String),
    /// Formato de imagen no soportado.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    /// Error parseando metadatos EXIF.
    #[error("exif error: {0}")]
    Exif(String),
}

/// Alias de resultado del proyecto.
pub type Result<T> = std::result::Result<T, ShImagesError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_converts_from_std_io_error() {
        let source = io::Error::new(io::ErrorKind::NotFound, "missing file");
        let err: ShImagesError = source.into();
        assert!(matches!(err, ShImagesError::Io(_)));
    }

    #[test]
    fn display_messages_are_human_readable() {
        assert_eq!(
            ShImagesError::Config("bad file".to_string()).to_string(),
            "configuration error: bad file"
        );
        assert_eq!(
            ShImagesError::Decode("crc failed".to_string()).to_string(),
            "decode error: crc failed"
        );
        assert_eq!(
            ShImagesError::UnsupportedFormat("webp".to_string()).to_string(),
            "unsupported format: webp"
        );
        assert!(ShImagesError::Io(io::Error::other("boom"))
            .to_string()
            .contains("io error"));
    }

    #[test]
    fn exif_error_displays_message() {
        let err = ShImagesError::Exif("bad block".to_string());
        assert_eq!(err.to_string(), "exif error: bad block");
        assert!(matches!(err, ShImagesError::Exif(_)));
    }

    #[test]
    fn result_alias_uses_sh_images_error() {
        fn fallible() -> Result<()> {
            Err(ShImagesError::Config("nope".to_string()))
        }
        assert!(fallible().is_err());
    }
}
