# Fase 2 — Subproyecto 2: Integración del LRU Cache y Pre-carga: Design Spec

> Fecha: 2026-08-01
> Proyecto: Sh_Images (visor de imágenes nativo en Rust, `egui` + `eframe`)
> Estado: Aprobado por el usuario (2026-08-01)

---

## 1. Contexto

El Subproyecto 1 de Fase 2 implementó el módulo `core/image_cache.rs` (cache LRU
thread-safe, evicción por límite de memoria en MiB, `get`/`insert`/`len`/
`memory_used`/`is_empty`/`hit_ratio`) pero **no está conectado a la app**: hoy
`app.rs` decodifica cada imagen desde disco en cada apertura o navegación, sin
cachear nada.

Este subproyecto conecta el cache al flujo de carga de `app.rs` y añade
**pre-carga de la imagen siguiente/anterior** (N±1) para que la navegación con
`←`/`→` sea inmediata cuando se vuelve a una imagen ya vista o se avanza a una
pre-cargada.

La carga asíncrona básica (un thread worker por apertura) ya existe desde Fase 1;
aquí se generaliza a un **canal único persistente** que sirve tanto a cargas
foreground como a pre-cargas background.

## 2. Alcance (in/out of scope)

### In scope
- Conectar `Arc<ImageCache>` al estado de `ShImagesApp` (campo `cache`).
- Canal único `mpsc` persistente compartido por todos los workers.
- `LoadEvent` sin imagen: `{ path, result: Result<()> }`. El cache es la única
  fuente de la imagen decodificada.
- `start_load(path)` con 3 ramas: cache hit (textura inmediata), in-flight
  (no-op), miss (spawn worker).
- Deduplicación de cargas en curso con `in_flight: Arc<Mutex<HashSet<PathBuf>>>`.
- Pre-carga de N±1 tras mostrar la imagen actual, saltando paths cacheados o en
  vuelo.
- `Navigation::neighbor_paths()` para obtener prev/next circulares.
- Módulo `core/preload.rs` con `preload_targets()` (función pura y testeable).
- Método `ImageCache::contains()` (chequeo sin reordenar LRU ni contar hit/miss).
- `poll_loader` que drena todos los eventos pendientes del canal y solo actúa
  sobre el path actual (ignora pre-cargas obsoletas; no tostifique errores de
  pre-carga).
- Tests unitarios para todo lo nuevo.

### Out of scope (subproyectos posteriores de Fase 2)
- Miniaturas y barra lateral (`thumbnail_gen.rs`, `sidebar.rs`) — S3.
- Benchmarks y tests de integración de flujos completos — S4.
- Persistencia del cache en disco.
- Decodificación diferida (tiling).
- Configuración de profundidad de pre-carga (default fijo N±1).

## 3. Decisiones de diseño (acordadas con el usuario)

1. **Canal único + cache como fuente de verdad**: todos los workers (foreground y
   pre-carga) decodifican → `cache.insert(path, image)` → envían un evento ligero
   `LoadEvent { path, result: Result<()> }`. La UI construye la textura leyendo el
   cache **solo** cuando el path recibido es el actual. No se mueve la
   `DynamicImage` (decenas de MB) por el canal (AGENTS.md §2.3: sin allocations
   innecesarias en hot paths).
2. **Pre-carga solo N±1**: tras mostrar la imagen `N`, precargar `N-1` y `N+1`
   (circular). Coste mínimo de I/O en background; cubre la mayoría de las
   navegaciones. No configurable en este subproyecto (YAGNI).
3. **Worker inserta al cache**: la inserción ocurre en el thread worker (más
   concurrente; la UI nunca bloquea para escribir). La UI solo lee.
4. **Deduplicación in-flight**: `Arc<Mutex<HashSet<PathBuf>>>` compartido. Antes
   de spawnear un worker se inserta el path; el worker se quita a sí mismo al
   terminar (éxito o error). Evita lanzar dos decodificaciones del mismo archivo
   cuando el usuario navega rápido y la pre-carga ya cubrió el path.
5. **`poll_loader` drena el canal**: `while let Ok(event) = rx.try_recv()`. Hoy
   solo consume un evento por frame; con pre-carga pueden llegar varios. Cada
   evento se compara contra `navigation.current_path()`.
6. **Pre-carga no tostifica errores**: si una pre-carga falla (archivo corrupto),
   se descarta silenciosamente (solo `tracing::debug!`). Solo el path actual
   muestra toast de error (comportamiento actual).
7. **`ImageCache::contains()` sin efectos**: `get` reordena la lista LRU y cuenta
   hit — no sirve para "solo preguntar si está". Se añade `contains(&Path) -> bool`
   que no reordena ni cuenta. Aislada en `core`, testeable.
8. **`preload_targets` como función pura**: `core/preload.rs` define
   `preload_targets(nav, depth, is_cached, is_in_flight) -> Vec<PathBuf>`. Sin I/O,
   sin threads: testeable al 100%. El orquestado (spawnear workers) queda en
   `app.rs`.
9. **No bloquear el UI thread**: la textura se construye desde el cache en el UI
   thread (como hoy, `make_texture`), pero la decodificación siempre es en worker.

## 4. Arquitectura y módulos

### Estructura resultante

```
src/
├── app.rs               # Estado global y loop principal (modificado)
├── core/
│   ├── image_cache.rs   # Añadir ImageCache::contains (API existente intacta)
│   ├── navigation.rs    # Añadir Navigation::neighbor_paths
│   └── preload.rs       # NUEVO: preload_targets (función pura)
```

### Cambios en `app.rs`

- Nuevos campos en `ShImagesApp`:
  ```rust
  cache: Arc<ImageCache>,
  in_flight: Arc<Mutex<HashSet<PathBuf>>>,
  tx: mpsc::Sender<LoadEvent>,
  rx: Option<mpsc::Receiver<LoadEvent>>,   // se crea una vez en new()
  ```
- `LoadEvent`:
  ```rust
  struct LoadEvent {
      path: PathBuf,
      result: Result<()>,
  }
  ```
- `start_load(path: PathBuf)`:
  ```rust
  // 1. Cache hit → textura inmediata + pre-carga.
  if let Some(entry) = self.cache.get(&path) {
      apply_decoded(&entry);          // textura + transform + preload
      return;
  }
  // 2. In-flight → no-op.
  if self.in_flight.lock().unwrap_or_else(|p| p.into_inner()).contains(&path) {
      return;
  }
  // 3. Miss → worker.
  self.spawn_load(path, /* is_preload */ false);
  ```
- `spawn_load(path, is_preload)`:
  - Añade `path` a `in_flight`.
  - Clona `tx`, `cache`, `in_flight`, `ctx`.
  - `std::thread::spawn`:
    ```rust
    let result = load_image(&path)
        .map(|image| { cache.insert(path.clone(), image); })
        .map_err(|e| { /* error tipado */ });
    in_flight.lock()...remove(&path);
    if tx.send(LoadEvent { path, result }).is_err() { tracing::debug!(...) }
    ctx.request_repaint();
    ```
  - Nota: `result` ya es `Result<(), ShImagesError>` (inserción no falla: no hay I/O).
- `poll_loader`:
  ```rust
  while let Some(event) = rx.try_recv().ok() {
      let is_current = self.navigation.as_ref()
          .and_then(|n| n.current_path())
          .map(|p| p == &event.path)
          .unwrap_or(false);
      if !is_current { continue; }   // pre-carga obsoleta: ya está en cache
      match event.result {
          Ok(()) => {
              if let Some(entry) = self.cache.get(&event.path) {
                  apply_decoded(&entry);
              }
          }
          Err(e) => self.toasts.push(format!("No se pudo abrir: {e}"), t),
      }
  }
  ```
- `apply_decoded(image: &DynamicImage)` — extraído del bloque `Ok` actual:
  construye textura, resetea transform, `user_interacted=false`, `last_viewport=None`,
  y llama `preload_neighbors()`.
- `preload_neighbors()`:
  ```rust
  let Some(nav) = &self.navigation else { return };
  let targets = preload_targets(nav, PRELOAD_DEPTH, |p| self.cache.contains(p),
      |p| self.in_flight.lock()...contains(p));
  for path in targets { self.spawn_load(path, /* is_preload */ true); }
  ```
  (`PRELOAD_DEPTH = 1`.)
- `open_path` y `navigate` ahora llaman a `start_load` (que internamente decide
  hit/in-flight/miss). `spawn_load` con `is_preload=true` no cambia la lógica del
  worker — el flag existe solo para logging (nivel DEBUG vs INFO) y para evitar
  toasts de error (ya cubierto por el check `is_current`).

### `Navigation::neighbor_paths()`

```rust
/// Rutas previa y siguiente (circular) respecto al índice actual.
pub fn neighbor_paths(&self) -> [Option<&PathBuf>; 2] { /* [prev, next] */ }
```

- `[]` si `images.is_empty()`.
- Con 1 imagen: `[Some(&misma), Some(&misma)]` (circularidad natural).
- No muta el estado (a diferencia de `next()`/`prev()`).

### `core/preload.rs`

```rust
/// Profundidad de pre-carga: cuántas imágenes adyacentes por lado.
pub const PRELOAD_DEPTH: isize = 1;

/// Devuelve los paths a precargar, en orden de prioridad [N+1, N-1, N+2, N-2...].
///
/// Excluye: paths ya cacheados (`is_cached`), paths en vuelo (`is_in_flight`),
/// y duplicados. Sin I/O ni efectos.
pub fn preload_targets(
    nav: &Navigation,
    depth: isize,
    is_cached: impl Fn(&Path) -> bool,
    is_in_flight: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> { /* ... */ }
```

Prioridad de orden: N+1 primero (es lo más probable al navegar con `→`), luego
N-1, luego N+2, etc.

### `ImageCache::contains()`

```rust
/// `true` si `path` está en el cache, sin reordenar la lista LRU ni contar hits.
pub fn contains(&self, path: &Path) -> bool {
    self.lock().map.contains_key(path)
}
```

## 5. Flujo de datos (resumen)

```
Abrir imagen N (open_path)
  └─ Navigation::from_folder → nav (current = N)
  └─ start_load(N)
       ├─ cache.get(N)?  → [hit] make_texture + transform + preload_neighbors()
       ├─ in_flight?     → no-op
       └─ miss → spawn_load(N, false)
                  worker: decode → cache.insert → event {N, Ok(())} → repaint
                  poll_loader: N == current → make_texture + transform + preload
                                                    └─ preload N+1, N-1 (si no cached/in-flight)
                                                       └─ spawn_load(N±1, true) → decodifican y cachean
Usuario pulsa → (navigate +1 → N+1)
  └─ start_load(N+1)
       ├─ cache.get(N+1)?  → [hit] instantáneo (¡la pre-carga ya lo cacheó!)
       ...
```

## 6. Testing

### 6.1 `image_cache.rs`
- `contains` sobre path cacheado devuelve `true`.
- `contains` sobre path ausente devuelve `false`.
- `contains` no reordena la lista (verificar con evicción posterior) y no
  incrementa `hit_ratio`.

### 6.2 `navigation.rs`
- `neighbor_paths` en lista de 3 devuelve prev y next correctos.
- `neighbor_paths` con 1 imagen devuelve la misma dos veces.
- `neighbor_paths` con lista vacía devuelve `[None, None]`.

### 6.3 `core/preload.rs`
- Pre-carga N+1 y N-1 con cache e in-flight vacíos.
- Excluye path ya cacheado.
- Excluye path en vuelo.
- Prioridad: N+1 antes que N-1.
- Depth 2 genera 4 targets en orden N+1, N-1, N+2, N-2.
- Sin duplicados.

### 6.4 Suite previa
- Los 44 tests existentes (Fase 0/1 + S1) siguen en verde; la API de
  `ImageCache` existente no cambia (solo se añade `contains`).

## 7. Criterios de éxito

- Abrir imagen → navegación con `→` a la siguiente pre-cargada: textura visible
  sin espera perceptible (latencia ≈ solo upload de textura, sin decode).
- Volver con `←` a una imagen ya vista: instantáneo (cache hit).
- `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo test`
  y `cargo test --release` pasan sin warnings.
- Sin `.unwrap()`/`.expect()` en producción.
- `core/` sin dependencias de `egui`.
