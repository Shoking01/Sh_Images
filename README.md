<div align="center">

# Sh_Images

**Visor de imágenes nativo · Ligero · Rápido · Sin Electron**

[English](#english) | [Español](#español)

</div>

---

## English

A native image viewer built in Rust using `egui` + `eframe`. No Electron, no WebView — just a fast,
responsive desktop app.

### Features

- **Instant open** — images load in <200 ms (4K) with async decoding
- **Zoom & pan** — centered zoom, mouse wheel, drag to pan
- **Rotate** — 90° CW/CCW with GPU mesh (no re-decode)
- **Navigation** — arrow keys through folder images, sidebar thumbnails
- **EXIF metadata** — camera model, ISO, aperture, focal length, date
- **Animated GIF** — looped playback
- **Slideshow** — auto-advance (configurable 1–60 s)
- **Fullscreen** — F11 toggle
- **Dark/light themes** — persisted between sessions
- **Configurable shortcuts** — edit keybindings
- **Windows MSI installer** — associates PNG, JPEG, BMP, GIF, WebP, TIFF

### Supported Formats
| Format | Status |
|--------|--------|
| PNG | |
| JPEG | |
| GIF (static + animated) | |
| BMP | |
| WebP | |
| TIFF | |
| AVIF | |

### Installation

#### Windows (release)
[Download the MSI] from GitHub Releases and run it. Supported images are associated
automatically.

Other platforms (macOS, Linux) — build from source.

#### From source
```bash
# Requires Rust 1.92+ (stable)
git clone https://github.com/Shoking01/Sh_Images.git
cd Sh_Images
cargo run
```

### CLI
```bat
sh_images.exe "C:\path\to\image.png"
```

### Development
```bash
# Run all checks before committing
cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test

# Integration tests (9 flows: open, navigate, zoom, error, config, rotate, EXIF, GIF, slideshow)
cargo test --test integration

# Performance benchmarks
cargo bench
```

### Contributing
1. Fork the repo
2. Create a feature branch (`git checkout -b feature/your-feature`)
3. Write tests for new functionality
4. Run `cargo test`, `cargo fmt`, `cargo clippy -- -D warnings`
5. Open a Pull Point

See [`CONTRIBUTING.md`](CONTRIBUTING.md) (es) for full guidelines.

### License
Project — see [LICENSE](LICENSE).

---

## Español

Un visor de imágenes ligero construido en Rust con `egui` + `eframe`. Sin Electron, sin WebView —
rendimiento nativo puro.

### 1. Características

- **Apertura instantánea** — < 15 ms para imágenes 4K con LRU async
- **Zoom y pan** — rueda del ratón, arrastrar, fit-to-window
- **Rotación** — 90° CW/CCW vía mesh GPU (sin re-decodificar)
- **Navegación** — flechas entre imágenes de la carpeta, sidebar con miniaturas
- **Metadatos EXIF** — panel derecho con cámara, ISO, apertura, focal, fecha
- **GIF animado** — reproducción en bucle
- **Slideshow** — avance automático (1–60 s configurable)
- **Pantalla completa** — F11
- **Tema oscuro/claro** — persistente entre sesiones
- **Atajos configurables** — editables desde la UI
- **Instalador MSI** para Windows — asocia PNG, JPG, BMP, GIF, WebP, TIFF

### 2. Formatos soportados
PNG, JPEG, GIF, BMP, WebP, TIFF, AVIF.

### 3. Instalación

#### Windows (MSI)
[Descargá el MSI] de GitHub Releases y ejecutalo. Las extensiones soportadas se asocian
automáticamente.

#### Desde código
```bash
# Requiere Rust 1.92+ (stable)
git clone https://github.com/Shoking01/Sh_Images.git
cd Sh_Images
cargo run
```

### 4. CLI
```bat
sh_images.exe "C:\ruta\a\imagen.jpg"
```

### 5. Desarrollo
```bash
# Tests completos antes de commit
cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test

# Tests de integración (9 flujos)
cargo test --test integration

# Benchmarks
cargo bench
```

### 6. Contribuir
1. Hacé fork del repositorio
2. Creá una rama (`git checkout -b feature/tu-feature`)
3. Escribí tests para la funcionalidad nueva
4. Ejecutá `cargo test`, `cargo fmt`, `cargo clippy -- -D warnings`
5. Abrí un Pull Request

Consultá [`CONTRIBUTING.md`](CONTRIBUTING.md) (es) para la guía completa.

### 7. Licencia
Ver [LICENSE](LICENSE).

---

### Project
Sh_Images is built in Rust. See [`Cargo.toml`](Cargo.toml) for dependencies.
All features rendered via `egui`/`eframe` (immediate-mode, wGPU backend).