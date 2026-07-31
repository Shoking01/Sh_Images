//! Cache LRU de imágenes decodificadas (implementación completa en Fase 2).

/// Cache de imágenes decodificadas con límite de memoria configurable.
pub struct ImageCache {
    /// Límite de memoria en MiB.
    pub memory_limit_mb: u64,
}

impl ImageCache {
    /// Crea una cache con el límite de memoria dado.
    pub fn new(memory_limit_mb: u64) -> Self {
        Self { memory_limit_mb }
    }
}
