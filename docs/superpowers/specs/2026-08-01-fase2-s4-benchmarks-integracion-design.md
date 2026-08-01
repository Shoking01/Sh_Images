# Fase 2 — Subproyecto 4: Benchmarks y Tests de Integración — Design

> Fecha: 2026-08-01 · Estado: Aprobado por usuario

## 1. Contexto

Sh_Images (Fases 0–3 completas) tiene:

- **Benchmark único** (`benches/opening.rs`) que solo abre el fixture PNG de 70 bytes. AGENTS.md §6.2 exige medir apertura a 1080p/4K/8K, latencia de navegación, y otros umbrales.
- **Cero tests de integración** (`tests/` solo contiene fixtures). AGENTS.md §8.1 exige tests de integración para 5 flujos críticos.

El S4 cierra estas dos deudas de calidad.

## 2. Decisiones de diseño

### 2.1 Tests de integración = lógica pura, sin egui

**Contexto:** Los 5 flujos de AGENTS.md §8.1 se describen con pasos de GUI (abrir app, diálogo nativo, renderizar). CI es headless (Windows/Linux sin display), `ShImagesApp::new` exige un `eframe::CreationContext`, y `open_path` usa `rfd` (diálogo nativo no automatizable).

**Decisión:** Cada flujo se testea componiendo las piezas de `core/` que lo implementan, sin instanciar la GUI. El paso "diálogo nativo" se abstrae: `open_path` ya recibe un `PathBuf`, que es exactamente lo que el diálogo produce.

**Consecuencias:**
- Corre en CI headless, rápido y determinista.
- Sin dependencias nuevas (no `egui_kittest`).
- No cubre render/painting real; eso queda como QA manual (§5.2 de AGENTS.md).

### 2.2 Fixtures de alta resolución = sintéticas en runtime

**Contexto:** AGENTS.md §8.2 limita fixtures a <100KB; una imagen 8K real supera eso y no debe commitearse. Los benchmarks necesitan 1080p/4K/8K.

**Decisión:** Criterion genera imágenes sintéticas en memoria (patrón de gradiente determinista, encode PNG/JPEG) y las guarda en `tempfile` durante el `setup`. Repo limpio, reproducible.

**Nota de conformidad (§8.2):** El texto de §8.2 dice "guarda imágenes de test en `tests/fixtures/`". El usuario eligió generar GIF/corrupto/vacío en `tempfile` (decidido en brainstorming). Esto satisface el *intento* de §8.2 (testear cada formato) aunque difiere de la letra. Queda registrado aquí como decisión; los fixtures `sample.png`/`sample.jpg` existentes se mantienen commiteados.

### 2.3 Benchmarks = 5 grupos

Se cubren todos los umbrales medibles de §6.2 sin GUI:

| Grupo | Mide | Umbral §6.2 |
|-------|------|-------------|
| `opening` | apertura PNG/JPEG 1080p/4K/8K | <100 / <200 / <500 ms |
| `navigation` | `from_folder` 1000 archivos + `next`/`prev`/`neighbor_paths` | navegación <50 ms |
| `thumbnail_gen` | `generate_thumbnail` 4K→96px | — |
| `image_cache` | insert/get/contains LRU | — |
| `preload` | `preload_targets` N±1 | — |

Los umbrales de RAM (idle <50MB, 4K <150MB) requieren `valgrind`/`dhat` (Linux-only) y no son medibles con criterion; se documentan como QA manual, ya contemplado en AGENTS.md §5.2.

## 3. Componentes

### 3.1 `benches/common/mod.rs` (nuevo)

Helpers compartidos para generar imágenes sintéticas:

- `gradient_image(w: u32, h: u32) -> DynamicImage` — patrón de gradiente determinista (RGBA8).
- `synthetic_image_path(dir: &Path, w: u32, h: u32, fmt: Format) -> PathBuf` — encode y guarda en `dir`, devuelve la ruta.
- Constantes `RES_1080P`/`RES_4K`/`RES_8K` como `(u32, u32)`.

### 3.2 `benches/opening.rs` (ampliar)

Genera PNG y JPEG a 1080p/4K/8K en `setup` y mide `load_image`. Benchmark functions:

- `open_png_1080p`, `open_png_4k`, `open_png_8k`, `open_jpeg_1080p`, `open_jpeg_4k`, `open_jpeg_8k`.

### 3.3 `benches/navigation.rs` (nuevo)

`setup` crea una carpeta temp con 1000 archivos `.jpg` (touched, no decodificados). Mide:

- `navigation_from_folder_1000` — construir `Navigation`.
- `navigation_next_1000` / `navigation_prev_1000` — `next()`/`prev()`.
- `neighbor_paths_1000` — `neighbor_paths()`.

### 3.4 `benches/thumbnail_gen.rs` (nuevo)

Mide `generate_thumbnail(&4k_image, THUMB_MAX)`.

### 3.5 `benches/image_cache.rs` (nuevo)

Mide sobre un `ImageCache::new(512)` con una imagen 4K:

- `cache_insert_4k` — `insert`.
- `cache_get_4k` — `get` + drop del ref.
- `cache_contains_4k` — `contains`.

### 3.6 `benches/preload.rs` (nuevo)

`Navigation` de 1000 archivos; mide `preload_targets(nav, PRELOAD_DEPTH, always_false, always_false)`.

### 3.7 `tests/common/mod.rs` (nuevo)

Helpers para tests de integración:

- `make_folder_with_images(dir: &Path, n: usize) -> PathBuf` — crea `img_0001.jpg`… con contenido sintético mínimo.
- `gradient_image(w, h)` — reutiliza la misma lógica que benches (¿duplicación? ver 3.8).
- `corrupt_png_path(dir) -> PathBuf` — bytes aleatorios con extensión `.png`.
- `empty_png_path(dir) -> PathBuf` — archivo vacío `.png`.
- `gif_path(dir) -> PathBuf` — GIF 1px válido (encode con `image`).

### 3.8 ¿Duplicación entre `benches/common` y `tests/common`?

Criterion y tests de integración no pueden compartir módulos fácilmente (cada uno tiene su propio `harness = false` / crate). Opciones:

1. **Duplicar los ~30 líneas del generador** en ambos `common/mod.rs` (aceptable, código pequeño y estable).
2. **Exponer `utils::test_images`** desde la lib (`#[doc(hidden)] pub`) y que ambos lo importen.

**Decisión:** Opción 1 (duplicar). Evita exponer helpers de test en la API pública de la librería; el costo es ~30 líneas duplicadas, aceptable. Queda como ADR si en el futuro se necesita un tercer consumidor.

### 3.9 `tests/integration.rs` (nuevo)

Cinco flujos, uno por función:

1. **`flujo_apertura_completo`**: carpeta temp con PNG → `Navigation::from_folder` → `current_path` correcto → `load_image` → `ImageCache::insert` → `get` → verificar dimensiones (equivale a abrir→decodificar→cachear).
2. **`flujo_navegacion_circular`**: N imágenes → `next()` hasta el final → vuelve al primero (circular) → `prev()` correcto → `neighbor_paths` devuelve N±1.
3. **`flujo_zoom_pan_fit`**: `ViewTransform::new(img, viewport)` → `apply_zoom_at` (zoom in) → `pan_by` → `fit()` → verificar que `fit()` restaura.
4. **`flujo_imagen_corrupta_no_crash`**: `corrupt_png_path` → `load_image` devuelve `Err(Decode)` sin panic; carpeta inexistente → `from_folder` devuelve `Err(Io)`; carpeta sin imágenes soportadas (solo `.txt`) → `from_folder` devuelve `Ok` con `images` vacía y `current_path() == None`.
5. **`flujo_configuracion_persistencia`**: `Settings::default` → `save` → modificar campo → `save` de nuevo → `load` → verificar que carga el último valor, todo con paths en `tempfile` (equivale a cerrar/reabrir sin tocar el config real del usuario). **No** se invoca `ShImagesApp::load_settings()`, porque lee del path real de config (`settings_path()`) y escribiría en el HOME del usuario durante el test.

## 4. Cargo.toml

No se añaden dependencias nuevas. `criterion` (ya en dev-deps) se usa con `[[bench]] harness = false` por cada archivo nuevo:

```toml
[[bench]]
name = "navigation"
harness = false

[[bench]]
name = "thumbnail_gen"
harness = false

[[bench]]
name = "image_cache"
harness = false

[[bench]]
name = "preload"
harness = false
```

## 5. Criterios de aceptación

- [ ] `cargo bench` compila y ejecuta los 5 grupos (muestra estimaciones).
- [ ] `cargo test` pasa con los nuevos tests de integración (siguen pasando los 103 unitarios).
- [ ] `cargo clippy --all-targets -- -D warnings` sin warnings.
- [ ] `cargo fmt --check` sin diffs.
- [ ] Umbrales §6.2 medibles (apertura/navegación) documentados en el reporte de criterion.
- [ ] Sin `unwrap`/`expect`/`unsafe`/`println!` en código de producción (los benchmarks/tests pueden usar `unwrap`/`expect` — no son producción).
- [ ] README.md actualizado con cómo correr benchmarks/tests (sección "Comandos").

## 6. Fuera de alcance

- QA manual GUI (render, diálogos, accesibilidad) — AGENTS.md §5.2, humano.
- Medición de RAM con valgrind/dhat — Linux-only, QA manual.
- `egui_kittest` / snapshot testing de UI.
- Umbrales de build time / tamaño de binario (§6.1) — métricas de CI, no de S4.
