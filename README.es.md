<div align="center">

# Sh_Images

**Visor de imágenes nativo · Ligero · Rápido · Sin Electron**

<a href="README.md" style="display:inline-block; padding:8px 24px; background:#2ea44f; color:#ffffff; border-radius:6px; text-decoration:none; font-weight:600; margin-top:8px;">🌐 English</a>

</div>

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

**Precarga:** Las imágenes adyacentes en la carpeta se decodifican en segundo plano con profundidad
configurable, garantizando navegación instantánea.

**Generación de miniaturas:** Dimensiones acotadas (configurables), procesadas vía pool de hilos
acotado (3 trabajadores) con caché LRU independiente para miniaturas decodificadas.

### Características

- **Apertura instantánea** — las imágenes se cargan en <200 ms (4K) con decodificación asíncrona
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

Otras plataformas (macOS, Linux) — compilar desde código fuente.

#### Desde código fuente
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
- Toolchain SVG para renderizado de ícono: `resvg` (incluido vía build.rs)

### CLI
```bat
sh_images.exe "C:\ruta\a\imagen.png"
```

Pasá una ruta de imagen como primer argumento para abrirla directamente.

### Configuración

La configuración se guarda en `settings.toml` (directorio de configuración específico de plataforma):

```toml
cache_memory_limit_mb = 512
theme = "dark"
slideshow_interval_secs = 5
language = "en"
```

La primera ejecución crea valores por defecto. Escritura atómica (temp + rename) previene
corrupción.

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

## Detalles Técnicos

**Dependencias** (`Cargo.toml`):
- `eframe` 0.35 + `egui` 0.35 — framework de GUI immediate-mode con backend wgpu
- `image` 0.25 — decodificación de imágenes (PNG, JPEG, GIF, BMP, WebP, TIFF, AVIF)
- `kamadak-exif` 0.6.1 — extracción de metadatos EXIF
- `rfd` 0.15 — diálogo de archivo nativo
- `serde` + `toml` — serialización de configuración
- `thiserror` — manejo de errores centralizado
- `tracing` — logging estructurado

**Optimizaciones de release:** LTO activado, una sola codegen unit, symbols eliminados — apuntando
a binarios de menos de 20 MB.

**Plataforma:** Windows (principal, instalador MSI), macOS y Linux (compilación desde código fuente).

**Versión:** 0.2.2 | **MSRV:** Rust 1.92+