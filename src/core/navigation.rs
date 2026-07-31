//! Navegación entre imágenes de una carpeta (implementación completa en Fase 1).

/// Estado de navegación sobre la lista ordenada de imágenes de una carpeta.
pub struct Navigation {
    /// Índice de la imagen actual en la lista.
    pub current: usize,
}

impl Navigation {
    /// Crea una navegación empezando en `current`.
    pub fn new(current: usize) -> Self {
        Self { current }
    }
}
