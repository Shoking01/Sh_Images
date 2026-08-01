# Fase 2 — Subproyecto 1: LRU Cache de Imágenes: Design Spec

> Fecha: 2026-08-01
> Proyecto: Sh_Images (visor de imágenes nativo en Rust, `egui` + `eframe`)
> Estado: Aprobado por el usuario (2026-08-01)

---

## 1. Contexto

La Fase 1 dejó la app mostrando imágenes con zoom/pan/fit, navegación por carpeta,
carga asíncrona mínima (un thread worker por apertura, sin cache) y toasts de error.
Cada apertura o navegación decodifica la imagen desde disco desde cero: al volver a
una imagen ya vista se re-decodifica. La Fase 2 introduce un **cache LRU de imágenes
decodificadas** para reutilizarlas, y en subproyectos posteriores añadirá pre-carga
de siguiente/anterior, miniaturas y benchmarks.

Este subproyecto implementa **solo el módulo `core/image_cache.rs`** + tests. No se
conecta todavía a `app.rs` (la integración con carga/pre-carga es el subproyecto 2).
`image_cache.rs` hoy es un stub con solo `memory_limit_mb` (Fase 0).

## 2. Alcance (in/out of scope)

### In scope
- Estructura LRU **propia** (sin crates externos): `HashMap<PathBuf, índice>` + arena
  de nodos con lista doblemente enlazada por índices (`Vec<Node>` + `prev`/`next`).
- API `get`/`insert`/`len`/`memory_used`/`is_empty`/`hit_ratio`.
- Evicción LRU por **límite de memoria en MiB** (compatible con el campo
  `cache_memory_limit_mb` ya existente en `Settings`).
- Estimación del coste en bytes de una `DynamicImage` por dimensiones + canales.
- Contadores de hit/miss para el ratio exigido por AGENTS.md §4.2.
- **Thread-safe**: `Mutex<CacheInner>` interno; el `Arc` lo pone el caller (app.rs)
  en el subproyecto 2.
- Tests unitarios completos (ver §6).

### Out of scope (subproyectos posteriores de Fase 2)
- Conectar el cache a `app.rs` / flujo de carga (subproyecto 2).
- Pre-carga de imagen siguiente/anterior (subproyecto 2).
- Miniaturas y barra lateral (`thumbnail_gen.rs`, `sidebar.rs`) (subproyecto 3).
- Benchmarks de cache y tests de integración (subproyecto 4).
- Persistencia del cache en disco.
- Decodificación diferida (solo se cachean imágenes ya decodificadas).

## 3. Decisiones de diseño (acordadas con el usuario)

1. **Módulo core puro + tests**: `core/` no depende de `egui` (AGENTS.md §3.2). La
   `DynamicImage` es de `image`, permitida en core. Sin tocar `app.rs` en este
   subproyecto.
2. **Implementación propia del LRU** (AGENTS.md §7.2: no añadir dependencias sin
   justificación): arena de nodos en `Vec<Node>` con `prev`/`next` como índices
   `usize` (no punteros). `HashMap<PathBuf, usize>` para lookups O(1). `get`/`insert`
   O(1), evicción O(1).
3. **Thread-safe desde el diseño**: `ImageCache` envuelve `CacheInner` en
   `std::sync::Mutex`. `get`/`insert`/etc. toman `&self` y bloquean internamente. No
   se usa `RwLock`: cada `get` reordena la lista LRU, de modo que todo acceso es
   escritura.
4. **`get` no clona los pixels**: clonar un `DynamicImage` (decenas de MB) en cada hit
   es inaceptable (AGENTS.md §2.3). Como el cache vive detrás de un `Mutex`, no se
   puede devolver un `&DynamicImage` con lifetime libre (el guard se soltaría al
   retornar). En su lugar, `get` devuelve `Option<CacheEntryRef<'_>>`: una estructura
   que mantiene el `MutexGuard` vivo y hace `Deref<Target = DynamicImage>`. El caller
   la usa como si fuera `&DynamicImage` sin copiar bytes.
5. **Tracking de memoria por dimensión**: `estimate_bytes` = `w * h * bytes_per_pixel`
   según `ColorType` (fallback conservador 4 B/px). `memory_used` se mantiene como
   suma incremental; evicción ocurre mientras `memory_used > limit`.
6. **Imagen que no cabe (all-or-nothing)**: si una imagen sola excede el límite,
   `insert` no evicta nada y devuelve `cached: false` con `evicted_keys` vacío. El
   cache queda intacto; el caller carga la imagen sin cache.
7. **`Arc` lo pone el caller**: `ImageCache::new(limit_mb)` devuelve `ImageCache`;
   `app.rs` (subproyecto 2) lo envolverá en `Arc<ImageCache>` para compartir con los
   workers. `core/` no importa `Arc`.
8. **Sin errores `Result`**: la cache es puramente en memoria, sin I/O. Los casos
   límite se comunican con `InsertResult` (no panics — AGENTS.md §2.1).
9. **Defensivo en invariantes**: si un índice de la arena no apunta a un nodo
   (invariante corrupta), se loguea `tracing::warn!` y se reconstruye la lista; nunca
   panic en producción.

## 4. Arquitectura y módulos

### Estructura resultante

```
src/core/
├── mod.rs          # sin cambios (image_cache ya declarado)
└── image_cache.rs  # REWRITE — LRU completo (reemplaza el stub de Fase 0)
```

### `core/image_cache.rs` — tipos

```rust
/// Entrada del cache: imagen decodificada + coste en bytes.
struct CacheEntry {
    image: DynamicImage,
    bytes: u64,
}

/// Nodo de la lista LRU; `prev`/`next` son índices en `nodes`
/// (`usize::MAX` = sin enlace, para lista con sentinelas de extremo).
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
    head: usize,   // índice del nodo más reciente (MRU)
    tail: usize,   // índice del nodo más antiguo (LRU)
    memory_used: u64,
    memory_limit_mb: u64,
    hit_count: u64,
    miss_count: u64,
}

/// Cache LRU de imágenes decodificadas, thread-safe.
pub struct ImageCache {
    inner: Mutex<CacheInner>,
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
```

### Invariantes de la lista

- `head` apunta al nodo más recientemente usado (MRU); `tail` al menos reciente (LRU).
- Extremos vacíos: `head`/`tail` = `usize::MAX` con `nodes` vacío.
- Enlaces `prev`/`next` siempre válidos salvo en los extremos (sentinelas).
- `map` y `nodes` siempre en sincronía: todo path en `map` tiene un nodo en `nodes`
  y viceversa.

### API pública

```rust
impl ImageCache {
    /// Crea el cache con límite en MiB.
    pub fn new(memory_limit_mb: u64) -> Self;

    /// Inserta una imagen decodificada; evicta en orden LRU hasta caber.
    /// All-or-nothing: si la imagen sola excede el límite, no cachea ni evicta.
    pub fn insert(&self, path: PathBuf, image: DynamicImage) -> InsertResult;

    /// Devuelve la imagen cacheada (la marca como recién usada), o `None`.
    /// El `CacheEntryRef` mantiene el lock; se usa como `&DynamicImage` vía `Deref`.
    pub fn get(&self, path: &Path) -> Option<CacheEntryRef<'_>>;

    /// Número de entradas en el cache.
    pub fn len(&self) -> usize;

    /// Bytes totales en uso.
    pub fn memory_used(&self) -> u64;

    /// `true` si no hay entradas.
    pub fn is_empty(&self) -> bool;

    /// Ratio de aciertos: hits / (hits + misses), 0.0 si no hubo accesos.
    pub fn hit_ratio(&self) -> f32;
}
```

> El `Default` de `ImageCache` usa 512 MiB, coincidiendo con `Settings::default()`
> (`cache_memory_limit_mb = 512`). `get` devuelve `CacheEntryRef` (guarda el lock
> vivo y deref a `DynamicImage`), no un `&DynamicImage` libre: esto es lo único que
> permite devolver una referencia sin clonar cuando el estado vive en un `Mutex`.

### Estimación de memoria

```rust
fn estimate_bytes(image: &DynamicImage) -> u64 {
    let (w, h) = (image.width() as u64, image.height() as u64);
    let bpp = match image.color() {
        image::ColorType::Rgb8 => 3,
        image::ColorType::Rgba8 => 4,
        image::ColorType::L8 => 1,
        image::ColorType::La8 => 2,
        _ => 4, // fallback conservador
    };
    w * h * bpp
}
```

- Límite en bytes: `memory_limit_mb * 1024 * 1024` (con `saturating_mul`).
- `insert`: añade el nodo, suma `bytes` a `memory_used`, y mientras
  `memory_used > limit` evicta `tail`. Si tras evictar todo la imagen aún no cabe
  (imagen sola > límite), revierte la inserción y restaura los nodos evictados
  (all-or-nothing).
- `get`: si el path está en `map`, incrementa `hit_count`, mueve el nodo a `head`,
  devuelve `Some(CacheEntryRef)` apuntando a `value.image`. Si no, incrementa
  `miss_count`, devuelve `None`.

## 5. Manejo de errores

- `insert`/`get` no devuelven `Result` (sin I/O; ver Decisión 8).
- Sin `.unwrap()`/`.expect()` en código de producción. Acceso a `HashMap` con
  `match`/`if let`.
- Invariantes de arena corrompidas → `tracing::warn!` + reconstrucción defensiva,
  nunca panic.

## 6. Casos de prueba

> Nombre descriptivo (AGENTS.md §4.3), cada test independiente, `tempdir` no
> necesario (no hay I/O; las imágenes se construyen en memoria con `image`).

1. `insert_then_get_roundtrips_small_image` — insertar una imagen pequeña y obtenerla
   por la misma key devuelve la imagen con sus dimensiones.
2. `get_on_missing_path_returns_none` — `get` de una key no insertada → `None`.
3. `eviction_removes_least_recently_used_first` — insertar más imágenes de las que
   caben; la evictada es la de menor uso (orden verificado con `evicted_keys`).
4. `get_moves_entry_to_most_recent` — acceder a la entrada más vieja; al insertar
   de nuevo, la evictada es otra (no la accedida).
5. `memory_used_never_exceeds_limit` — tras varias inserciones, `memory_used <= limit`.
6. `memory_used_and_len_correct_after_evictions` — verificar contabilidad tras
   evicción (suma incremental).
7. `oversized_image_is_not_cached_all_or_nothing` — imagen > límite → `cached: false`,
   `evicted_keys` vacío, cache intacto (`len` sin cambios).
8. `hit_ratio_tracks_hits_and_misses` — secuencia de gets → ratio esperado.
9. `insert_existing_path_replaces_entry` — re-insertar la misma key reemplaza la
   imagen y actualiza el tamaño.
10. `zero_memory_limit_rejects_everything` — límite 0 → cualquier insert devuelve
    `cached: false`.
11. `zero_dimension_image_fits` — imagen con ancho o alto 0 → `estimate_bytes` = 0,
    entra sin problema.
12. `is_empty_reflects_state` — `is_empty` es `true` al crear y `false` tras insertar.
13. `default_matches_settings_default` — `ImageCache::default()` usa 512 MiB.

## 7. Dependencias

Sin dependencias nuevas. `std::collections::HashMap`, `std::sync::Mutex`,
`image::DynamicImage` (ya en `Cargo.toml`). No se modifica `Cargo.toml`.

## 8. Criterios de aceptación

- [ ] `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`
      y `cargo test` pasan.
- [ ] Cobertura de `core/image_cache.rs` ≥ 85% (AGENTS.md §4.1, core ≥ 90% objetivo).
- [ ] Sin `.unwrap()`/`.expect()` fuera de `#[cfg(test)]`.
- [ ] Docstrings `///` en toda la API pública.
- [ ] `ImageCache::default()` = 512 MiB coincide con `Settings::default()`.
- [ ] Los 13 casos de prueba de §6 pasan.
