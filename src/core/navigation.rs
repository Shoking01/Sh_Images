//! Navegación entre imágenes de una carpeta.

use std::fs;
use std::path::{Path, PathBuf};

use crate::utils::errors::{Result, ShImagesError};

/// Extensiones de imagen soportadas (sin punto; el filtro es case-insensitive).
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif", "avif",
];

/// Estado de navegación sobre la lista ordenada de imágenes de una carpeta.
#[derive(Debug)]
pub struct Navigation {
    /// Rutas absolutas de las imágenes, ordenadas alfabéticamente.
    pub images: Vec<PathBuf>,
    /// Índice de la imagen actual.
    pub current: usize,
}

impl Navigation {
    /// Crea la navegación sobre la carpeta del archivo `image_path`.
    ///
    /// Lee el directorio padre de `image_path`, filtra por extensiones
    /// soportadas (case-insensitive), ordena alfabéticamente y localiza el
    /// índice de `image_path` (o 0 si no está).
    ///
    /// # Errors
    /// * `ShImagesError::Io` si el directorio no existe o no puede leerse.
    /// * `ShImagesError::Config` si `image_path` no tiene directorio padre.
    pub fn from_folder(image_path: &Path, supported_exts: &[&str]) -> Result<Self> {
        let folder = image_path.parent().ok_or_else(|| {
            ShImagesError::Config("image path has no parent directory".to_string())
        })?;
        let mut images = Vec::new();
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && has_supported_extension(&path, supported_exts) {
                images.push(path);
            }
        }
        images.sort();
        let current = images.iter().position(|p| p == image_path).unwrap_or(0);
        Ok(Self { images, current })
    }

    /// Avanza a la siguiente imagen; al final de la lista vuelve al inicio.
    pub fn next(&mut self) {
        if self.images.is_empty() {
            return;
        }
        self.current = (self.current + 1) % self.images.len();
    }

    /// Retrocede a la imagen anterior; al inicio de la lista va al final.
    pub fn prev(&mut self) {
        if self.images.is_empty() {
            return;
        }
        self.current = (self.current + self.images.len() - 1) % self.images.len();
    }

    /// Ruta de la imagen actual, o `None` si la lista está vacía.
    pub fn current_path(&self) -> Option<&PathBuf> {
        self.images.get(self.current)
    }
}

/// Devuelve `true` si `path` tiene una extensión en `supported_exts`.
pub fn has_supported_extension(path: &Path, supported_exts: &[&str]) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| supported_exts.iter().any(|s| s.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_folder() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let folder = dir.path().to_path_buf();
        for name in ["b.png", "a.jpg", "c.txt", "d.JPG"] {
            fs::write(folder.join(name), b"x").unwrap();
        }
        (dir, folder)
    }

    #[test]
    fn from_folder_filters_by_extension() {
        let (_d, folder) = setup_folder();
        let nav = Navigation::from_folder(&folder.join("a.jpg"), SUPPORTED_EXTENSIONS).unwrap();
        assert_eq!(nav.images.len(), 3); // b.png, a.jpg, d.JPG (c.txt excluido)
        for p in &nav.images {
            assert!(has_supported_extension(p, SUPPORTED_EXTENSIONS));
        }
    }

    #[test]
    fn from_folder_sorts_alphabetically() {
        let (_d, folder) = setup_folder();
        let nav = Navigation::from_folder(&folder.join("a.jpg"), SUPPORTED_EXTENSIONS).unwrap();
        let names: Vec<String> = nav
            .images
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["a.jpg", "b.png", "d.JPG"]);
    }

    #[test]
    fn from_folder_sets_current_to_matching_path() {
        let (_d, folder) = setup_folder();
        let nav = Navigation::from_folder(&folder.join("d.JPG"), SUPPORTED_EXTENSIONS).unwrap();
        assert_eq!(nav.current, 2);
    }

    #[test]
    fn from_folder_falls_back_to_zero_if_not_found() {
        let (_d, folder) = setup_folder();
        let nav = Navigation::from_folder(&folder.join("zzz.png"), SUPPORTED_EXTENSIONS).unwrap();
        assert_eq!(nav.current, 0);
    }

    #[test]
    fn from_folder_returns_io_error_on_missing_dir() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("no_such_dir").join("a.png");
        let err = Navigation::from_folder(&missing, SUPPORTED_EXTENSIONS).unwrap_err();
        assert!(matches!(err, ShImagesError::Io(_)));
    }

    #[test]
    fn from_folder_returns_config_error_when_no_parent() {
        let err = Navigation::from_folder(Path::new(""), SUPPORTED_EXTENSIONS).unwrap_err();
        assert!(matches!(err, ShImagesError::Config(_)));
    }

    #[test]
    fn next_wraps_circularly() {
        let (_d, folder) = setup_folder();
        let mut nav = Navigation::from_folder(&folder.join("a.jpg"), SUPPORTED_EXTENSIONS).unwrap();
        nav.next();
        assert_eq!(nav.current, 1);
        nav.next();
        assert_eq!(nav.current, 2);
        nav.next();
        assert_eq!(nav.current, 0);
    }

    #[test]
    fn prev_wraps_circularly() {
        let (_d, folder) = setup_folder();
        let mut nav = Navigation::from_folder(&folder.join("a.jpg"), SUPPORTED_EXTENSIONS).unwrap();
        nav.prev();
        assert_eq!(nav.current, 2);
    }

    #[test]
    fn next_on_empty_is_noop() {
        let dir = tempdir().unwrap();
        let empty = dir.path().join("nothing.png");
        let mut nav = Navigation::from_folder(&empty, SUPPORTED_EXTENSIONS).unwrap();
        assert!(nav.images.is_empty());
        nav.next();
        nav.prev();
        assert_eq!(nav.current, 0);
        assert!(nav.current_path().is_none());
    }
}
