# Fase 2 — Subproyecto 1: LRU Cache de Imágenes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implementar el cache LRU thread-safe de imágenes decodificadas en `core/image_cache.rs` con API `get`/`insert`/`len`/`memory_used`/`is_empty`/`hit_ratio`, evicción LRU por límite de memoria en MiB y tests unitarios completos.

**Architecture:** Arena de nodos (`Vec<Node>` con enlaces `prev`/`next` como índices `usize`, sentinela `NO_NODE = usize::MAX`) + `HashMap<PathBuf, usize>` para lookups O(1). Estado protegido por `Mutex<CacheInner>`; `get` devuelve `CacheEntryRef<'a>` (guarda el lock y `Deref<Target = DynamicImage>`) para no clonar pixels. `core/` no depende de `egui` (AGENTS.md §3.2); `Arc` lo pone el caller en el subproyecto 2.

**Tech Stack:** `std::collections::HashMap`, `std::sync::Mutex`, `image::DynamicImage` (ya en `Cargo.toml`). Sin dependencias nuevas.

**Spec:** `docs/superpowers/specs/2026-08-01-fase2-lru-cache-design.md`

---

## File Structure

```
src/core/image_cache.rs   # REWRITE — reemplaza el stub de Fase 0 (hoy solo `memory_limit_mb`)
```

No se toca `Cargo.toml` (sin dependencias nuevas) ni `app.rs` (integración en subproyecto 2).

---

## Task 1: Tests que fallan — API del LRU cache

**Files:**
- Rewrite: `src/core/image_cache.rs`

- [ ] **Step 1: Escribir los tests**

Reemplazar `src/core/image_cache.rs` completo por el siguiente contenido (solo tests; el build fallará porque la API aún no existe):

```rust
//! Cache LRU de imágenes decodificadas, thread-safe.
//!
//! `core/` no depende de `egui` (AGENTS.md §3.2). Implementación completa en el
//! siguiente task: aquí solo viven los tests (TDD, fail-first).

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    const MIB: u64 = 1024 * 1024;

    /// Helper: imagen RGBA de `w x h` (4 B/px).
    fn rgba(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(w, h))
    }

    #[test]
    fn estimate_bytes_counts_channels() {
        assert_eq!(estimate_bytes(&rgba(16, 16)), 16 * 16 * 4);
        let rgb = DynamicImage::ImageRgb8(image::RgbImage::new(16, 16));
        assert_eq!(estimate_bytes(&rgb), 16 * 16 * 3);
    }

    #[test]
    fn default_matches_settings_default() {
        let cache = ImageCache::default();
        assert_eq!(cache.memory_limit_mb(), 512);
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn new_with_limit_exposes_limit() {
        let cache = ImageCache::new(64);
        assert_eq!(cache.memory_limit_mb(), 64);
    }

    #[test]
    fn is_empty_reflects_state() {
        let cache = ImageCache::new(1);
        assert!(cache.is_empty());
        cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn insert_then_get_roundtrips_small_image() {
        let cache = ImageCache::new(1);
        let res = cache.insert(PathBuf::from("a.png"), rgba(64, 32));
        assert!(res.cached);
        assert!(res.evicted_keys.is_empty());
        let got = cache.get(Path::new("a.png")).expect("debería estar cacheada");
        assert_eq!(got.dimensions(), (64, 32));
    }

    #[test]
    fn get_on_missing_path_returns_none() {
        let cache = ImageCache::new(1);
        assert!(cache.get(Path::new("nope.png")).is_none());
    }

    #[test]
    fn insert_existing_path_replaces_entry() {
        let cache = ImageCache::new(1);
        cache.insert(PathBuf::from("a.png"), rgba(256, 256));
        let res = cache.insert(PathBuf::from("a.png"), rgba(128, 128));
        assert!(res.cached);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.memory_used(), 128 * 128 * 4);
        assert_eq!(
            cache.get(Path::new("a.png")).expect("cacheada").dimensions(),
            (128, 128)
        );
    }

    #[test]
    fn oversized_image_is_not_cached_all_or_nothing() {
        let cache = ImageCache::new(1); // 1 MiB
        cache.insert(PathBuf::from("a.png"), rgba(256, 256)); // 256 KiB
        let res = cache.insert(PathBuf::from("big.png"), rgba(1024, 1024)); // 4 MiB
        assert!(!res.cached);
        assert!(res.evicted_keys.is_empty());
        assert_eq!(cache.len(), 1);
        assert!(cache.get(Path::new("a.png")).is_some());
        assert!(cache.get(Path::new("big.png")).is_none());
    }

    #[test]
    fn zero_dimension_image_fits() {
        let cache = ImageCache::new(1);
        let res = cache.insert(PathBuf::from("zero.png"), rgba(0, 64));
        assert!(res.cached);
        assert_eq!(cache.memory_used(), 0);
    }

    #[test]
    fn zero_memory_limit_rejects_normal_image() {
        let cache = ImageCache::new(0);
        let res = cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert!(!res.cached);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn eviction_removes_least_recently_used_first() {
        let cache = ImageCache::new(1); // 1 MiB = 4 × 256 KiB
        for name in ["a.png", "b.png", "c.png", "d.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.memory_used(), 4 * 256 * 256 * 4);

        let res = cache.insert(PathBuf::from("e.png"), rgba(256, 256));
        assert!(res.cached);
        assert_eq!(res.evicted_keys, vec![PathBuf::from("a.png")]);
        assert_eq!(cache.len(), 4);
        assert!(cache.get(Path::new("a.png")).is_none());
        for name in ["b.png", "c.png", "d.png", "e.png"] {
            assert!(cache.get(Path::new(name)).is_some());
        }
    }

    #[test]
    fn get_moves_entry_to_most_recent() {
        let cache = ImageCache::new(1);
        for name in ["a.png", "b.png", "c.png", "d.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        // Acceder a la más vieja (a) la mueve al frente (MRU).
        assert!(cache.get(Path::new("a.png")).is_some());
        let res = cache.insert(PathBuf::from("e.png"), rgba(256, 256));
        // La evictada ahora es b, no a.
        assert_eq!(res.evicted_keys, vec![PathBuf::from("b.png")]);
        assert!(cache.get(Path::new("a.png")).is_some());
        assert!(cache.get(Path::new("b.png")).is_none());
    }

    #[test]
    fn memory_used_never_exceeds_limit() {
        let cache = ImageCache::new(1);
        for i in 0..50 {
            let name = format!("img_{i}.png");
            cache.insert(PathBuf::from(&name), rgba(64, 64)); // 16 KiB cada una
            assert!(cache.memory_used() <= 1 * MIB);
        }
    }

    #[test]
    fn memory_used_and_len_correct_after_evictions() {
        let cache = ImageCache::new(1);
        for name in ["a.png", "b.png", "c.png", "d.png", "e.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.memory_used(), 4 * 256 * 256 * 4);
        for name in ["b.png", "c.png", "d.png", "e.png"] {
            assert!(cache.get(Path::new(name)).is_some());
        }
        assert!(cache.get(Path::new("a.png")).is_none());
    }

    #[test]
    fn hit_ratio_tracks_hits_and_misses() {
        let cache = ImageCache::new(1);
        cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert_eq!(cache.hit_ratio(), 0.0);

        assert!(cache.get(Path::new("a.png")).is_some()); // hit
        assert!(cache.get(Path::new("a.png")).is_some()); // hit
        assert!(cache.get(Path::new("b.png")).is_none()); // miss
        let ratio = cache.hit_ratio();
        assert!((ratio - 2.0 / 3.0).abs() < 1e-3);
    }
}
```

- [ ] **Step 2: Ejecutar los tests para verificar que fallan**

Run: `cargo test --lib core::image_cache`
Expected: FAIL — errores de compilación (`ImageCache`, `InsertResult`, `CacheEntryRef`, `estimate_bytes`, `DynamicImage` no definidos en `super`).

- [ ] **Step 3: Commit de los tests rojos**

```bash
git add src/core/image_cache.rs
git commit -m "test: add failing LRU cache API tests (TDD red)"
```

---

## Task 2: Implementación completa del LRU cache

**Files:**
- Rewrite: `src/core/image_cache.rs`

- [ ] **Step 1: Escribir la implementación**

Reemplazar `src/core/image_cache.rs` completo (tests del Task 1 + implementación). El archivo final:

```rust
//! Cache LRU de imágenes decodificadas, thread-safe.
//!
//! `core/` no depende de `egui` (AGENTS.md §3.2). La `DynamicImage` viene del
//! crate `image`, ya presente. El cache es puramente en memoria: `insert` y
//! `get` no devuelven `Result` porque no hay I/O (Decisión 8 del spec).

use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use image::DynamicImage;

/// Límite de memoria por defecto en MiB (coincide con `Settings::default()`).
pub const DEFAULT_MEMORY_LIMIT_MB: u64 = 512;

/// Índice "sin enlace" para las sentinelas de la lista LRU.
const NO_NODE: usize = usize::MAX;

/// Entrada del cache: imagen decodificada + coste en bytes.
struct CacheEntry {
    image: DynamicImage,
    bytes: u64,
}

/// Nodo de la lista LRU; `prev`/`next` son índices en `nodes`
/// (`NO_NODE` = sin enlace).
struct Node {
    key: PathBuf,
    value: CacheEntry,
    prev: usize,
    next: usize,
}

/// Estado interno del cache, protegido por `Mutex`.
struct CacheInner {
    map: HashMap<PathBuf, usize>,
    nodes: Vec<Node>,
    /// Índice del nodo más recientemente usado (MRU).
    head: usize,
    /// Índice del nodo menos recientemente usado (LRU).
    tail: usize,
    memory_used: u64,
    memory_limit_mb: u64,
    hit_count: u64,
    miss_count: u64,
}

impl CacheInner {
    /// Límite de memoria en bytes.
    fn limit_bytes(&self) -> u64 {
        self.memory_limit_mb.saturating_mul(1024 * 1024)
    }

    /// Inserta `index` al frente de la lista (MRU).
    fn push_front(&mut self, index: usize) {
        let old_head = self.head;
        self.nodes[index].prev = NO_NODE;
        self.nodes[index].next = old_head;
        if old_head != NO_NODE {
            self.nodes[old_head].prev = index;
        } else {
            self.tail = index;
        }
        self.head = index;
    }

    /// Desengancha `index` de la lista (mantiene `head`/`tail` correctos).
    fn unlink(&mut self, index: usize) {
        let (prev, next) = (self.nodes[index].prev, self.nodes[index].next);
        if prev != NO_NODE {
            self.nodes[prev].next = next;
        } else {
            self.head = next;
        }
        if next != NO_NODE {
            self.nodes[next].prev = prev;
        } else {
            self.tail = prev;
        }
        self.nodes[index].prev = NO_NODE;
        self.nodes[index].next = NO_NODE;
    }

    /// Mueve `index` al frente de la lista (MRU).
    fn move_to_front(&mut self, index: usize) {
        if self.head == index {
            return;
        }
        self.unlink(index);
        self.push_front(index);
    }

    /// Elimina el nodo `index` del arena con `swap_remove` y repara los índices
    /// del nodo desplazado (`map`, `head`/`tail` y enlaces de sus vecinos).
    fn remove_node(&mut self, index: usize) -> Node {
        let last = self.nodes.len() - 1;
        let node = self.nodes.swap_remove(index);
        if index != last {
            let moved_key = self.nodes[index].key.clone();
            self.map.insert(moved_key, index);
            let (mprev, mnext) = (self.nodes[index].prev, self.nodes[index].next);
            if mprev != NO_NODE {
                self.nodes[mprev].next = index;
            } else {
                self.head = index;
            }
            if mnext != NO_NODE {
                self.nodes[mnext].prev = index;
            } else {
                self.tail = index;
            }
        }
        node
    }

    /// Borra todo el estado (recuperación defensiva ante invariantes rotas).
    fn clear(&mut self) {
        self.map.clear();
        self.nodes.clear();
        self.head = NO_NODE;
        self.tail = NO_NODE;
        self.memory_used = 0;
    }

    /// Inserta una imagen; evicta en orden LRU hasta caber. All-or-nothing: si
    /// la imagen sola excede el límite, no cachea ni evicta nada.
    fn insert(&mut self, path: PathBuf, image: DynamicImage) -> InsertResult {
        let bytes = estimate_bytes(&image);

        if bytes > self.limit_bytes() {
            return InsertResult {
                cached: false,
                evicted_keys: Vec::new(),
            };
        }

        let mut evicted_keys = Vec::new();

        // Reemplazo de una key existente.
        if let Some(&index) = self.map.get(&path) {
            let old_bytes = self.nodes[index].value.bytes;
            self.memory_used = self
                .memory_used
                .saturating_sub(old_bytes)
                .saturating_add(bytes);
            self.nodes[index].value = CacheEntry { image, bytes };
            self.move_to_front(index);
            return InsertResult {
                cached: true,
                evicted_keys,
            };
        }

        // Inserción de nodo nuevo.
        let index = self.nodes.len();
        self.nodes.push(Node {
            key: path.clone(),
            value: CacheEntry { image, bytes },
            prev: NO_NODE,
            next: NO_NODE,
        });
        self.map.insert(path, index);
        self.memory_used = self.memory_used.saturating_add(bytes);
        self.push_front(index);

        // Evictar del tail (LRU) mientras exceda el límite.
        while self.memory_used > self.limit_bytes() {
            if self.tail == NO_NODE {
                tracing::warn!("lru cache invariant violated; clearing cache");
                self.clear();
                evicted_keys.clear();
                return InsertResult {
                    cached: false,
                    evicted_keys,
                };
            }
            let tail = self.tail;
            let key = self.nodes[tail].key.clone();
            self.unlink(tail);
            let node = self.remove_node(tail);
            self.memory_used = self.memory_used.saturating_sub(node.value.bytes);
            self.map.remove(&key);
            evicted_keys.push(key);
        }

        InsertResult {
            cached: true,
            evicted_keys,
        }
    }

    /// Devuelve el índice del nodo para `path` (marcándolo como MRU) o `None`,
    /// actualizando los contadores de hit/miss.
    fn get_index(&mut self, path: &Path) -> Option<usize> {
        match self.map.get(path).copied() {
            Some(index) => {
                self.hit_count = self.hit_count.saturating_add(1);
                self.move_to_front(index);
                Some(index)
            }
            None => {
                self.miss_count = self.miss_count.saturating_add(1);
                None
            }
        }
    }
}

/// Cache LRU de imágenes decodificadas, thread-safe.
pub struct ImageCache {
    inner: Mutex<CacheInner>,
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new(DEFAULT_MEMORY_LIMIT_MB)
    }
}

impl ImageCache {
    /// Crea el cache con límite en MiB.
    pub fn new(memory_limit_mb: u64) -> Self {
        Self {
            inner: Mutex::new(CacheInner {
                map: HashMap::new(),
                nodes: Vec::new(),
                head: NO_NODE,
                tail: NO_NODE,
                memory_used: 0,
                memory_limit_mb,
                hit_count: 0,
                miss_count: 0,
            }),
        }
    }

    /// Bloquea el mutex recuperándose de un lock envenenado (nunca panic).
    fn lock(&self) -> MutexGuard<'_, CacheInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Inserta una imagen decodificada; evicta en orden LRU hasta caber.
    ///
    /// All-or-nothing: si la imagen sola excede el límite, no cachea ni evicta.
    pub fn insert(&self, path: PathBuf, image: DynamicImage) -> InsertResult {
        self.lock().insert(path, image)
    }

    /// Devuelve la imagen cacheada (la marca como recién usada), o `None`.
    ///
    /// El `CacheEntryRef` mantiene el lock; se usa como `&DynamicImage` vía `Deref`.
    pub fn get(&self, path: &Path) -> Option<CacheEntryRef<'_>> {
        let mut inner = self.lock();
        let index = inner.get_index(path)?;
        Some(CacheEntryRef {
            guard: inner,
            index,
        })
    }

    /// Número de entradas en el cache.
    pub fn len(&self) -> usize {
        self.lock().map.len()
    }

    /// `true` si no hay entradas.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Bytes totales en uso.
    pub fn memory_used(&self) -> u64 {
        self.lock().memory_used
    }

    /// Límite de memoria configurado en MiB.
    pub fn memory_limit_mb(&self) -> u64 {
        self.lock().memory_limit_mb
    }

    /// Ratio de aciertos: hits / (hits + misses), 0.0 si no hubo accesos.
    pub fn hit_ratio(&self) -> f32 {
        let inner = self.lock();
        let total = inner.hit_count + inner.miss_count;
        if total == 0 {
            0.0
        } else {
            inner.hit_count as f32 / total as f32
        }
    }
}

/// Resultado de `insert`.
pub struct InsertResult {
    /// `true` si la imagen entró al cache.
    pub cached: bool,
    /// Claves evictadas para hacer espacio (vacío si `cached: false`).
    pub evicted_keys: Vec<PathBuf>,
}

/// Acceso a una entrada cacheada; mantiene el `MutexGuard` vivo.
///
/// El caller lo usa como `&DynamicImage` vía `Deref` (sin clonar pixels).
pub struct CacheEntryRef<'a> {
    guard: MutexGuard<'a, CacheInner>,
    index: usize,
}

impl Deref for CacheEntryRef<'_> {
    type Target = DynamicImage;
    fn deref(&self) -> &DynamicImage {
        &self.guard.nodes[self.index].value.image
    }
}

/// Coste de una `DynamicImage` en bytes (dimensiones × canales).
fn estimate_bytes(image: &DynamicImage) -> u64 {
    let (w, h) = (image.width() as u64, image.height() as u64);
    let bpp = match image.color() {
        image::ColorType::Rgb8 => 3,
        image::ColorType::Rgba8 => 4,
        image::ColorType::L8 => 1,
        image::ColorType::La8 => 2,
        _ => 4, // fallback conservador
    };
    w.saturating_mul(h).saturating_mul(bpp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    const MIB: u64 = 1024 * 1024;

    /// Helper: imagen RGBA de `w x h` (4 B/px).
    fn rgba(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(w, h))
    }

    #[test]
    fn estimate_bytes_counts_channels() {
        assert_eq!(estimate_bytes(&rgba(16, 16)), 16 * 16 * 4);
        let rgb = DynamicImage::ImageRgb8(image::RgbImage::new(16, 16));
        assert_eq!(estimate_bytes(&rgb), 16 * 16 * 3);
    }

    #[test]
    fn default_matches_settings_default() {
        let cache = ImageCache::default();
        assert_eq!(cache.memory_limit_mb(), 512);
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn new_with_limit_exposes_limit() {
        let cache = ImageCache::new(64);
        assert_eq!(cache.memory_limit_mb(), 64);
    }

    #[test]
    fn is_empty_reflects_state() {
        let cache = ImageCache::new(1);
        assert!(cache.is_empty());
        cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn insert_then_get_roundtrips_small_image() {
        let cache = ImageCache::new(1);
        let res = cache.insert(PathBuf::from("a.png"), rgba(64, 32));
        assert!(res.cached);
        assert!(res.evicted_keys.is_empty());
        let got = cache.get(Path::new("a.png")).expect("debería estar cacheada");
        assert_eq!(got.dimensions(), (64, 32));
    }

    #[test]
    fn get_on_missing_path_returns_none() {
        let cache = ImageCache::new(1);
        assert!(cache.get(Path::new("nope.png")).is_none());
    }

    #[test]
    fn insert_existing_path_replaces_entry() {
        let cache = ImageCache::new(1);
        cache.insert(PathBuf::from("a.png"), rgba(256, 256));
        let res = cache.insert(PathBuf::from("a.png"), rgba(128, 128));
        assert!(res.cached);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.memory_used(), 128 * 128 * 4);
        assert_eq!(
            cache.get(Path::new("a.png")).expect("cacheada").dimensions(),
            (128, 128)
        );
    }

    #[test]
    fn oversized_image_is_not_cached_all_or_nothing() {
        let cache = ImageCache::new(1); // 1 MiB
        cache.insert(PathBuf::from("a.png"), rgba(256, 256)); // 256 KiB
        let res = cache.insert(PathBuf::from("big.png"), rgba(1024, 1024)); // 4 MiB
        assert!(!res.cached);
        assert!(res.evicted_keys.is_empty());
        assert_eq!(cache.len(), 1);
        assert!(cache.get(Path::new("a.png")).is_some());
        assert!(cache.get(Path::new("big.png")).is_none());
    }

    #[test]
    fn zero_dimension_image_fits() {
        let cache = ImageCache::new(1);
        let res = cache.insert(PathBuf::from("zero.png"), rgba(0, 64));
        assert!(res.cached);
        assert_eq!(cache.memory_used(), 0);
    }

    #[test]
    fn zero_memory_limit_rejects_normal_image() {
        let cache = ImageCache::new(0);
        let res = cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert!(!res.cached);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn eviction_removes_least_recently_used_first() {
        let cache = ImageCache::new(1); // 1 MiB = 4 × 256 KiB
        for name in ["a.png", "b.png", "c.png", "d.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.memory_used(), 4 * 256 * 256 * 4);

        let res = cache.insert(PathBuf::from("e.png"), rgba(256, 256));
        assert!(res.cached);
        assert_eq!(res.evicted_keys, vec![PathBuf::from("a.png")]);
        assert_eq!(cache.len(), 4);
        assert!(cache.get(Path::new("a.png")).is_none());
        for name in ["b.png", "c.png", "d.png", "e.png"] {
            assert!(cache.get(Path::new(name)).is_some());
        }
    }

    #[test]
    fn get_moves_entry_to_most_recent() {
        let cache = ImageCache::new(1);
        for name in ["a.png", "b.png", "c.png", "d.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        // Acceder a la más vieja (a) la mueve al frente (MRU).
        assert!(cache.get(Path::new("a.png")).is_some());
        let res = cache.insert(PathBuf::from("e.png"), rgba(256, 256));
        // La evictada ahora es b, no a.
        assert_eq!(res.evicted_keys, vec![PathBuf::from("b.png")]);
        assert!(cache.get(Path::new("a.png")).is_some());
        assert!(cache.get(Path::new("b.png")).is_none());
    }

    #[test]
    fn memory_used_never_exceeds_limit() {
        let cache = ImageCache::new(1);
        for i in 0..50 {
            let name = format!("img_{i}.png");
            cache.insert(PathBuf::from(&name), rgba(64, 64)); // 16 KiB cada una
            assert!(cache.memory_used() <= 1 * MIB);
        }
    }

    #[test]
    fn memory_used_and_len_correct_after_evictions() {
        let cache = ImageCache::new(1);
        for name in ["a.png", "b.png", "c.png", "d.png", "e.png"] {
            cache.insert(PathBuf::from(name), rgba(256, 256));
        }
        assert_eq!(cache.len(), 4);
        assert_eq!(cache.memory_used(), 4 * 256 * 256 * 4);
        for name in ["b.png", "c.png", "d.png", "e.png"] {
            assert!(cache.get(Path::new(name)).is_some());
        }
        assert!(cache.get(Path::new("a.png")).is_none());
    }

    #[test]
    fn hit_ratio_tracks_hits_and_misses() {
        let cache = ImageCache::new(1);
        cache.insert(PathBuf::from("a.png"), rgba(16, 16));
        assert_eq!(cache.hit_ratio(), 0.0);

        assert!(cache.get(Path::new("a.png")).is_some()); // hit
        assert!(cache.get(Path::new("a.png")).is_some()); // hit
        assert!(cache.get(Path::new("b.png")).is_none()); // miss
        let ratio = cache.hit_ratio();
        assert!((ratio - 2.0 / 3.0).abs() < 1e-3);
    }
}
```

- [ ] **Step 2: Ejecutar los tests para verificar que pasan**

Run: `cargo test --lib core::image_cache`
Expected: PASS — 15 tests (13 del spec + `estimate_bytes_counts_channels` + `new_with_limit_exposes_limit`).

- [ ] **Step 3: Verificar compilación y lints**

Run:
```bash
cargo check
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
Expected: 0 warnings, 0 diffs de formato.

- [ ] **Step 4: Commit**

```bash
git add src/core/image_cache.rs
git commit -m "feat: add thread-safe LRU image cache with memory limit"
```

---

## Task 3: Verificación final según AGENTS.md

**Files:** ninguno (QA)

- [ ] **Step 1: Correr la suite completa**

Run:
```bash
cargo check
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
cargo test --release
```
Expected: todo pasa, 0 warnings, ~59 tests totales (44 previos + 15 de image_cache).

- [ ] **Step 2: Verificar ausencia de panics en producción**

Run: `rg -n "unwrap\(|expect\(" src/`
Expected: solo apariciones dentro de `#[cfg(test)]`. Nota: `lock()` usa
`unwrap_or_else` (recuperación de lock envenenado), que no es un `unwrap(`.

- [ ] **Step 3: Verificar cobertura de `core/image_cache.rs`**

Run: `cargo test --lib core::image_cache`
Expected: los 15 tests pasan (cubre inserción, evicción LRU, límite de memoria,
hit/miss ratio, all-or-nothing — AGENTS.md §4.2).

- [ ] **Step 4: Commit final (solo si la verificación modificó algo)**

```bash
git add -A
git commit -m "chore: final verification pass for Fase 2 LRU cache"
```

> Si el árbol está limpio tras `cargo fmt --check`, no hay commit.

---

## Self-Review

**Spec coverage:**
- Estructura LRU propia (arena + índices, sin punteros) ✓ (Task 2)
- API `get`/`insert`/`len`/`memory_used`/`is_empty`/`hit_ratio` ✓ (Task 2)
- Evicción LRU por límite MiB ✓ (Task 2)
- `estimate_bytes` por dimensiones + canales ✓ (Task 2)
- Contadores hit/miss ✓ (Task 2)
- Thread-safe con `Mutex` interno ✓ (Task 2)
- All-or-nothing (imagen > límite no cachea ni evicta) ✓ (Task 2, test
  `oversized_image_is_not_cached_all_or_nothing`)
- `Default` = 512 MiB coincide con `Settings::default()` ✓ (test
  `default_matches_settings_default`)
- 13 casos de prueba del spec ✓ (Task 1)
- Sin dependencias nuevas ✓ (sin cambios en `Cargo.toml`)
- Sin `.unwrap()`/`.expect()` en producción ✓ (Task 3 Step 2)
- Docstrings en toda la API pública ✓ (Task 2)

**Placeholder scan:** Sin "TBD"/"TODO" en el plan. Todo paso tiene código completo.

**Type consistency:**
- `ImageCache::new(u64) -> Self`, `Default` → `new(DEFAULT_MEMORY_LIMIT_MB)` — consistente.
- `insert(&self, PathBuf, DynamicImage) -> InsertResult` — consistente entre Task 1 (tests) y Task 2 (impl).
- `get(&self, &Path) -> Option<CacheEntryRef<'_>>` — consistente.
- `CacheEntryRef` con `Deref<Target = DynamicImage>` — consistente.
- `estimate_bytes(&DynamicImage) -> u64` — consistente entre test y impl.
- `DEFAULT_MEMORY_LIMIT_MB = 512` coincide con `Settings::default().cache_memory_limit_mb`.
