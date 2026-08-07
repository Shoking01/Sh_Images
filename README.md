<div align="center">

# Sh_Images

**Native image viewer · Lightweight · Fast · No Electron**

[English](#english) | [Español](#español)

</div>

---

## English

A native image viewer built in Rust using `egui` + `eframe`. No Electron, no WebView — just a fast,
responsive desktop app powered by an immediate-mode GUI with wgpu rendering.

### Architecture

| Layer | Responsibility |
|-------|---------------|
| **UI** (`src/ui/`) | Immediate-mode rendering: viewer, sidebar, toolbar, info panel, toasts |
| **Core** (`src/core/`) | Image loading, LRU cache, navigation, preloading, EXIF, thumbnails |
| **Config** (`src/config/`) | TOML settings persistence (atomic write), shortcut mapping |
| **Utils** (`src/utils/`) | Error handling, path resolution, platform defaults |

**Concurrency model:** Image decoding, EXIF reading, and thumbnail generation run on dedicated
thread pools. Communication uses `mpsc` channels; shared state (`ImageCache`, `ThumbnailCache`)
is guarded by `Mutex` and wrapped in `Arc`.

**LRU Image Cache:** Bounded by configurable memory limit (default 512 MiB). Eviction removes
least-recently-used entries first. Tracks hit ratio for observability.

**Preloading:** Adjacent folder images are pre-decoded in the background with configurable depth,
ensuring instant navigation.

**Thumbnail Generation:** Capped dimensions (configurable), processed via a bounded thread pool
(3 workers) with a separate LRU cache for decoded thumbnails.

### Features

- **Instant open** — images load in <200 ms (4K) with async decoding
- **Zoom & pan** — centered zoom, mouse wheel, drag to pan, fit-to-window
- **Rotate** — 90° CW/CCW via GPU mesh transform (no re-decode)
- **Navigation** — arrow keys through folder images, sidebar thumbnails with scroll
- **EXIF metadata** — camera model, ISO, aperture, focal length, date, dimensions
- **Animated GIF** — looped playback with frame timing
- **Slideshow** — auto-advance (configurable 1–60 s interval)
- **Fullscreen** — F11 toggle, borderless window
- **Dark/light themes** — persisted between sessions
- **Configurable shortcuts** — edit keybindings via in-app dialog
- **Windows MSI installer** — associates PNG, JPEG, BMP, GIF, WebP, TIFF, AVIF

### Supported Formats

| Format | Encoding | Status |
|--------|----------|--------|
| PNG | 8/16-bit, RGBA | ✅ Supported |
| JPEG | Baseline + progressive | ✅ Supported |
| GIF | Static + animated | ✅ Supported |
| BMP | Uncompressed | ✅ Supported |
| WebP | Lossy + lossless | ✅ Supported |
| TIFF | Multi-page | ✅ Supported |
| AVIF | AV1-based | ✅ Supported |

### Installation

#### Windows (release)
[Download the MSI](https://github.com/Shoking01/Sh_Images/releases) from GitHub Releases and run it.
Supported image extensions are associated automatically.

Other platforms (macOS, Linux) — build from source.

#### From source
```bash
# Requires Rust 1.92+ (stable)
git clone https://github.com/Shoking01/Sh_Images.git
cd Sh_Images
cargo run --release
```

**Build-time requirements:**
- `rustc` 1.92+ (MSRV)
- `cargo` (stable toolchain)
- On Windows (MSI): `cargo-wix` or WiX Toolset v3
- SVG toolchain for icon rendering: `resvg` (bundled via build.rs)

### CLI
```bat
sh_images.exe "C:\path\to\image.png"
```

Pass an image path as the first argument to open it directly.

### Development
```bash
# Run all checks before committing
cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test

# Integration tests (9 flows: open, navigate, zoom, error, config, rotate, EXIF, GIF, slideshow)
cargo test --test integration

# Performance benchmarks (criterion)
cargo bench
```

**Code quality:** `clippy -- -D warnings` enforced. `cargo fmt` for formatting. Snapshot testing
via `insta` for UI state.

### Configuration

Settings are stored in `settings.toml` (platform-specific config directory):

```toml
cache_memory_limit_mb = 512
theme = "dark"
slideshow_interval_secs = 5
language = "en"
```

First run creates defaults. Atomic write (temp + rename) prevents corruption.

### Contributing
1. Fork the repo
2. Create a feature branch (`git checkout -b feature/your-feature`)
3. Write tests for new functionality
4. Run `cargo test`, `cargo fmt`, `cargo clippy -- -D warnings`
5. Open a Pull Request

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for full guidelines.

### License
MIT License — see [LICENSE](LICENSE).

---

## Español

Un visor de imágenes nativo construido en Rust con `egui` + `eframe`. Sin Electron, sin WebView —
una aplicación de escritorio rápida y responsiva basada en GUI immediate-mode con renderizado wgpu.

### Arquitectura

| Capa | Responsabilidad |
|------|----------------|
| **UI** (`src/ui/`) | Renderizado immediate-mode: visor, sidebar, toolbar, panel de información, notificaciones |
| **Core** (`src/core/`) | Carga de imágenes, caché LRU, navegación, precarga, EXIF, miniaturas |
| **Config** (`src/config/`) | Persistencia de configuración TOML (escritura atómica), mapeo de atajos |
| **Utils** (`src/utils/`) | Manejo de errores, resolución de rutas, valores por plataforma |

**Modelo de concurrencia:** La decodificación de imágenes, lectura EXIF y generación de miniaturas
se ejecutan en pools de hilos dedicados. La comunicación usa canales `mpsc`; el estado compartido
(`ImageCache`, `ThumbnailCache`) está protegido por `Mutex` y envuelto en `Arc`.

**Caché LRU de imágenes:** Acotada por límite de memoria configurable (512 MiB por defecto). La
evicción elimina primero las entradas menos usadas recientemente. Registra ratio de aciertos
para observabilidad.

**Precarga:** Las imágenes adyacentes en la carpeta se decodifican en segundo plano con
profundidad configurable, garantizando navegación instantánea.

**Generación de miniaturas:** Dimensiones acotadas (configurables), procesadas vía pool de hilos
acotado (3 trabajadores) con caché LRU independiente para miniaturas decodificadas.

### Características

- **Apertura instantánea** — imágenes se cargan en <200 ms (4K) con decodificación asíncrona
- **Zoom y pan** — zoom centrado, rueda del ratón, arrastrar, ajustar a ventana
- **Rotación** — 90° CW/CCW vía transformación mesh GPU (sin re-decodificar)
- **Navegación** — flechas entre imágenes de la carpeta, sidebar con miniaturas y scroll
- **Metadatos EXIF** — modelo de cámara, ISO, apertura, longitud focal, fecha, dimensiones
- **GIF animado** — reproducción en bucle con temporización de frames
- **Slideshow** — avance automático (intervalo configurable 1–60 s)
- **Pantalla completa** — F11, ventana sin bordes
- **Tema oscuro/claro** — persistente entre sesiones
- **Atajos configurables** — edición de keybindings vía diálogo en la app
- **Instalador MSI para Windows** — asocia PNG, JPEG, BMP, GIF, WebP, TIFF, AVIF

### Formatos soportados

| Formato | Codificación | Estado |
|---------|-------------|--------|
| PNG | 8/16-bit, RGBA | ✅ Soportado |
| JPEG | Baseline + progressive | ✅ Soportado |
| GIF | Estático + animado | ✅ Soportado |
| BMP | Sin compresión | ✅ Soportado |
| WebP | Lossy + lossless | ✅ Soportado |
| TIFF | Multi-página | ✅ Soportado |
| AVIF | Basado en AV1 | ✅ Soportado |

### Instalación

#### Windows (release)
[Descargá el MSI](https://github.com/Shoking01/Sh_Images/releases) de GitHub Releases y ejecutalo.
Las extensiones soportadas se asocian automáticamente.

Otras plataformas (macOS, Linux) — compilar desde código.

#### Desde código
```bash
# Requiere Rust 1.92+ (stable)
git clone https://github.com/Shoking01/Sh_Images.git
cd Sh_Images
cargo run --release
```

**Requisitos de compilación:**
- `rustc` 1.92+ (MSRV)
- `cargo` (toolchain estable)
- En Windows (MSI): `cargo-wix` o WiX Toolset v3
- Toolchain SVG para renderizado de icono: `resvg` (incluido vía build.rs)

### CLI
```bat
sh_images.exe "C:\ruta\a\imagen.png"
```

Pasá una ruta de imagen como primer argumento para abrirla directamente.

### Desarrollo
```bash
# Ejecutá todos los checks antes de commit
cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test

# Tests de integración (9 flujos: open, navigate, zoom, error, config, rotate, EXIF, GIF, slideshow)
cargo test --test integration

# Benchmarks de rendimiento (criterion)
cargo bench
```

**Calidad de código:** `clippy -- -D warnings` obligatorio. `cargo fmt` para formato. Snapshot
testing vía `insta` para estado de UI.

### Configuración

La configuración se guarda en `settings.toml` (directorio de configuración específico de plataforma):

```toml
cache_memory_limit_mb = 512
theme = "dark"
slideshow_interval_secs = 5
language = "en"
```

La primera ejecución crea valores por defecto. Escritura atómica (temp + rename) previene corrupción.

### Contribuir
1. Hacé fork del repositorio
2. Creá una rama (`git checkout -b feature/tu-feature`)
3. Escribí tests para la funcionalidad nueva
4. Ejecutá `cargo test`, `cargo fmt`, `cargo clippy -- -D warnings`
5. Abrí un Pull Request

Consultá [`CONTRIBUTING.md`](CONTRIBUTING.md) para la guía completa.

### Licencia
Licencia MIT — ver [LICENSE](LICENSE).

---

### Technical Details / Detalles Técnicos

**Dependencies** (`Cargo.toml`):
- `eframe` 0.35 + `egui` 0.35 — immediate-mode GUI framework with wgpu backend
- `image` 0.25 — image decoding (PNG, JPEG, GIF, BMP, WebP, TIFF, AVIF)
- `kamadak-exif` 0.6.1 — EXIF metadata extraction
- `rfd` 0.15 — native file dialog
- `serde` + `toml` — settings serialization
- `thiserror` — centralized error handling
- `tracing` — structured logging

**Release optimizations:** LTO enabled, single codegen unit, symbols stripped — targeting
binaries under 20 MB.

**Platform:** Windows (primary, MSI installer), macOS and Linux (build from source).

**Version:** 0.2.2 | **MSRV:** Rust 1.92+
