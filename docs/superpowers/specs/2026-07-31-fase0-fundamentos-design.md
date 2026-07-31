# Design Spec — Fase 0: Fundamentos

> Proyecto: Sh_Images — Visor de Imágenes Nativo en Rust
> Fecha: 2026-07-31
> Estado: Aprobado por el usuario

---

## 1. Objetivo

Dejar el proyecto compilando con una ventana vacía de `eframe`, la base de
errores y configuración operativas, CI funcionando, y métricas de rendimiento
base definidas. Todo el código debe adherirse a `AGENTS.md` (sin `unsafe`, sin
`unwrap`/`expect` en producción, clippy limpio, cobertura de tests).

## 2. Alcance

### Incluido en Fase 0
- Setup de crate binario `sh_images` (edition 2021).
- Dependencias: `eframe`, `egui`, `thiserror`, `serde`, `toml`, `tracing`,
  `tracing-subscriber`; dev-deps: `criterion`. También `image` (decidido en
  revisión) para el benchmark base de apertura.
- Estructura de módulos completa como skeleton compilable (todo el árbol del
  `Plan.md` §3), con stubs documentados en `core/` y `ui/`.
- Sistema de errores `ShImagesError` completo (real, no stub).
- Sistema de configuración TOML completo (real, no stub).
- Ventana vacía de `eframe` renderizándose (`app.rs` + `main.rs`).
- Benchmark base de apertura de imagen con `criterion` + fixture PNG.
- Workflow CI de GitHub Actions (check, clippy, fmt, test).
- `docs/ARCHITECTURE.md` con ADRs.

### Excluido de Fase 0 (fases posteriores)
- Carga real de imágenes en la UI (Fase 1). La función `core::image_loader::load_image` se implementa en Fase 0 de forma síncrona básica solo para el benchmark.
- Diálogo de archivos, zoom/pan, navegación (Fase 1).
- LRU cache, carga asíncrona, miniaturas (Fase 2).
- EXIF (Fase 4).
- Logging se limita a inicialización del subscriber (Fase 0).

## 3. Arquitectura

### 3.1 Estructura de Módulos

Se añade un target de **librería** (`src/lib.rs`) además del binario. Razones:
- En un crate binario, los ítems `pub` no referenciados disparan el lint
  `dead_code`, que con `-D warnings` rompería el build de los stubs.
- Permite que los tests de integración (`tests/`) y los benchmarks (`benches/`)
  importen la lógica del crate.
- `main.rs` queda como wrapper fino que llama a la librería.

```
src/
├── lib.rs           # Declara módulos; target de librería (evita dead_code en stubs)
├── main.rs          # Inicialización + arranque de eframe (≤50 líneas)
├── app.rs           # App struct, estado mínimo, update() con ventana vacía
├── ui/
│   ├── mod.rs       # Re-exporta subcomponentes
│   ├── viewer.rs    # Stub documentado (visión del visor)
│   ├── toolbar.rs   # Stub documentado
│   ├── sidebar.rs   # Stub documentado
│   └── theme.rs     # Stub documentado (tema oscuro default)
├── core/
│   ├── mod.rs
│   ├── image_loader.rs  # load_image(path) real (decodificación síncrona con `image`)
│   ├── image_cache.rs   # Stub: struct ImageCache + límite
│   ├── thumbnail_gen.rs # Stub
│   ├── navigation.rs    # Stub
│   └── exif.rs          # Stub
├── config/
│   ├── mod.rs
│   └── settings.rs  # Settings + load/save real
└── utils/
    ├── mod.rs
    ├── paths.rs     # config_dir() para Windows/macOS/Linux
    └── errors.rs    # ShImagesError completo
```

### 3.2 Sistema de Errores

`enum ShImagesError` con `thiserror`:
- `Io(#[from] std::io::Error)`
- `Config(String)` — problema de lectura/escritura/parseo de configuración
- `Decode(String)` — error decodificando imagen (usado por `image` en benchmark)
- `UnsupportedFormat(String)` — formato no soportado

Alias `type Result<T> = std::result::Result<T, ShImagesError>`.

Variant `Io` se convierte con `#[from]`; `Decode` se mapea desde
`image::ImageError` con `map_err` (evita acoplar el tipo de error de `image` al
error global).

### 3.3 Configuración

```rust
#[derive(Serialize, Deserialize, Default)]
struct Settings {
    cache_memory_limit_mb: u64,  // default 512 (via Default)
    theme: String,               // default "dark"
}
```

- `Settings::load(path) -> Result<Settings>`: si el archivo no existe, devuelve
  `Settings::default()` y crea el archivo con defaults. Si existe pero está
  corrupto, devuelve `ShImagesError::Config` (no panic).
- `Settings::save(path) -> Result<()>`: escribe TOML atomically (escribir a
  temp + rename).
- Ruta: `utils/paths.rs` — `config_dir()` resuelve:
  - Windows: `%APPDATA%/sh_images/settings.toml`
  - macOS: `$HOME/Library/Application Support/sh_images/settings.toml`
  - Linux: `$XDG_CONFIG_HOME` o `$HOME/.config/sh_images/settings.toml`
  - Sin dependencia `dirs` en Fase 0; se usan env vars + `std::env`.

### 3.4 App Shell

- `main.rs`: inicializa `tracing_subscriber`, llama `eframe::run_native` con
  `ShImagesApp::new()`.
- `app.rs`: `struct ShImagesApp { settings: Settings }`. `update()` renderiza un
  `CentralPanel` vacío con fondo del tema. El título de ventana y el tamaño
  inicial (1280x800) se configuran en las `NativeOptions`.
- La configuración se carga al construir `ShImagesApp` (log WARN si falla, no
  crashea).

### 3.5 Logging

- `tracing-subscriber` con formato default, nivel `INFO` en release, `DEBUG`
  en debug (feature `tracing_release_max_level_info` o `EnvFilter` simple).
- En Fase 0 solo se usa en `main.rs` (inicialización) y en la carga de config.

## 4. Componentes

| Componente | Responsabilidad | Interfaz pública |
|-----------|----------------|------------------|
| `utils::errors` | Tipos de error globales | `ShImagesError`, `Result<T>` |
| `config::settings` | Persistencia de preferencias | `Settings::load`, `Settings::save` |
| `utils::paths` | Resolución de rutas | `config_dir()`, `settings_path()` |
| `core::image_loader` | Carga síncrona básica (Fase 0) | `load_image(path) -> Result<DynamicImage>` |
| `core::image_cache` | Stub de cache | `ImageCache::new(memory_limit_mb)` (sin implementación real) |
| `app` | Glue UI/core | `ShImagesApp::new()`, `ShImagesApp::update()` |
| `ui::theme` | Stub de tema | `apply_dark_theme(ctx)` (skeleton aplicado si trivial) |

## 5. Manejo de Errores

- Ningún `unwrap()`/`expect()` en producción. Usar `?` y `map_err`.
- La carga de configuración fallida en arranque → log WARN + defaults (app no
  crashea nunca en Fase 0).
- Todos los paths se construyen con `PathBuf`, no strings concatenadas.

## 6. Testing

- `utils/errors.rs`: test de conversión `From<std::io::Error>`, `Display` de
  cada variant.
- `config/settings.rs`: serialización→deserialización roundtrip, defaults,
  archivo inexistente crea defaults + archivo, archivo corrupto devuelve
  `Config` error. Usa `tempfile` (dev-dependency justificada).
- `utils/paths.rs`: resolución de `config_dir()` bajo env vars simuladas.
- `core::image_loader.rs`: tests de decodificación del fixture PNG, archivo inexistente → `Io` error, archivo corrupto → `Decode` error.
- Benchmark `benches/opening.rs`: `criterion`, carga el fixture PNG de
  `tests/fixtures/sample.png` (<100KB) usando `core::image_loader::load_image`.
  Establece la línea base.
- Fixture: se commitea un PNG mínimo (~16x16, <1KB) en `tests/fixtures/sample.png`,
  generado con un script one-off. En Fase 1 se ampliará el set de fixtures.

## 7. CI

`.github/workflows/ci.yml`:
- Matrix: `windows-latest`, `ubuntu-latest`.
- Steps: checkout, rust-toolchain (stable), `cargo check`, `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `cargo test`.
- `cargo test --release` no se ejecuta en CI de Fase 0 (tiempo); se ejecuta en
  pre-commit local.

## 8. Métricas

- Baseline (benchmark `criterion`): tiempo de apertura del fixture PNG.
- Objetivos del plan (referencia): apertura 4K < 200ms — se medirá en Fase 2 con
  imágenes grandes; en Fase 0 solo se establece la infraestructura.

## 9. Riesgos

- `eframe` en Windows requiere `Visual C++` runtime — documentar en README.
- Primera compilación de `eframe` es lenta (~minutos); no es un error.
- Benchmark necesita el fixture presente en CI — se commitea en `tests/fixtures/`.

## 10. Documentación

- `docs/ARCHITECTURE.md`: ADRs:
  - ADR-001: `eframe` + `egui` como GUI.
  - ADR-002: `ShImagesError` centralizado con `thiserror`.
  - ADR-003: `image` crate para decodificación.
- Docstrings `///` en toda función pública.
