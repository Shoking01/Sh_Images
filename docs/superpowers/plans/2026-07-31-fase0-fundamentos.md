# Fase 0 — Fundamentos Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dejar Sh_Images compilando con una ventana `eframe` vacía, sistema de errores y configuración operativos, CI funcionando, y benchmark base de apertura de imagen.

**Architecture:** Crate con dos targets — `lib` (`src/lib.rs`, donde vive toda la lógica, evita `dead_code` en stubs y habilita tests/benchmarks) y `bin` (`src/main.rs`, wrapper fino). `ShImagesError` centralizado con `thiserror`. `Settings` TOML con load/save. `core::image_loader::load_image` decodifica con `image`. Stubs documentados en `core/` y `ui/`.

**Tech Stack:** Rust 2021, `eframe`/`egui` 0.35, `image` 0.25, `thiserror` 2, `serde` 1 + `toml` 1.x, `tracing`/`tracing-subscriber` 0.1/0.3. Dev: `criterion` 0.5, `tempfile` 3.

**Spec:** `docs/superpowers/specs/2026-07-31-fase0-fundamentos-design.md`

---

## File Structure

```
Cargo.toml                      # Create — deps, targets lib/bin/bench
src/lib.rs                      # Create — declara módulos (lib target)
src/main.rs                     # Create (Task 8) — init logging + eframe
src/app.rs                      # Create (Task 8) — ShImagesApp
src/core/mod.rs                 # Create (Task 1) — declara submódulos core
src/core/image_loader.rs        # Create (Task 5) — load_image real
src/core/image_cache.rs         # Create (Task 6) — stub
src/core/thumbnail_gen.rs       # Create (Task 6) — stub
src/core/navigation.rs          # Create (Task 6) — stub
src/core/exif.rs                # Create (Task 6) — stub
src/config/mod.rs               # Create (Task 1)
src/config/settings.rs          # Create (Task 4) — Settings real
src/ui/mod.rs                   # Create (Task 1)
src/ui/viewer.rs                # Create (Task 7) — stub
src/ui/toolbar.rs               # Create (Task 7) — stub
src/ui/sidebar.rs               # Create (Task 7) — stub
src/ui/theme.rs                 # Create (Task 7) — apply() real (trivial)
src/utils/mod.rs                # Create (Task 1)
src/utils/errors.rs             # Create (Task 2) — ShImagesError real
src/utils/paths.rs              # Create (Task 3) — config_dir/settings_path real
tests/fixtures/sample.png       # Create (Task 5) — PNG 1x1
benches/opening.rs              # Create (Task 9) — criterion benchmark
.github/workflows/ci.yml        # Create (Task 10) — CI
docs/ARCHITECTURE.md            # Create (Task 11) — ADRs
README.md                       # Create (Task 11) — nota VC++ runtime
```

---

## Task 1: Scaffold del crate y estructura de módulos

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/core/mod.rs`, `src/config/mod.rs`, `src/ui/mod.rs`, `src/utils/mod.rs`

- [ ] **Step 1: Crear `Cargo.toml`**

```toml
[package]
name = "sh_images"
version = "0.1.0"
edition = "2021"

[lib]
name = "sh_images"
path = "src/lib.rs"

[[bin]]
name = "sh_images"
path = "src/main.rs"

[[bench]]
name = "opening"
harness = false

[dependencies]
# GUI — eframe/egui: framework inmediato-mode nativo, fijado por Plan.md.
eframe = "0.35"
egui = "0.35"
# Decodificación de imágenes — justificación: benchmark base y Fase 1.
image = "0.25"
# Serialización de configuración TOML.
serde = { version = "1", features = ["derive"] }
toml = "1"
# Sistema de errores centralizado (AGENTS.md §3.3).
thiserror = "2"
# Logging estructurado (AGENTS.md §7.3).
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
# Benchmarks de rendimiento.
criterion = { version = "0.5", features = ["html_reports"] }
# Archivos temporales en tests de I/O (AGENTS.md §4.3).
tempfile = "3"
```

> Actualización (2026-07-31): durante la revisión de Task 1 se subió la versión de `eframe`/`egui` a **0.35** y `toml` a **1.x** (la última estable), y se commiteó `Cargo.lock`. El código de egui en Tasks 7-8 usa API estable desde 0.31 (inmediato-mode), por lo que el plan no cambia en contenido; cualquier ajuste de API se resuelve en el compile de esos tasks.

> Nota: `src/main.rs` aún no existe; cargo lo exige porque `[[bin]]` lo declara. Créalo en Task 8. Para que `cargo check` pase en este task, crea también un `src/main.rs` temporal vacío (lo sobreescribiremos en Task 8) — o comenta el `[[bin]]` hasta Task 8. Recomendado: comentar `[[bin]]` y `[[bench]]` ahora; se descomentan en sus tasks.

- [ ] **Step 2: Crear `src/lib.rs`**

```rust
//! Sh_Images — Visor de imágenes nativo en Rust.
//!
//! La librería contiene toda la lógica de negocio, separada del binario
//! (`main.rs`). Esto permite tests de integración y benchmarks sobre la lógica
//! sin acoplar a la UI.

pub mod config;
pub mod core;
pub mod ui;
pub mod utils;
```

- [ ] **Step 3: Crear los `mod.rs` vacíos**

Crear `src/core/mod.rs`, `src/config/mod.rs`, `src/ui/mod.rs`, `src/utils/mod.rs`, cada uno vacío (0 bytes o un comentario `//!`).

- [ ] **Step 4: Verificar compilación**

Run: `cargo check`
Expected: `Compiling sh_images` ... `Finished` sin errores. La primera vez tarda minutos (descarga de eframe).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/lib.rs src/core/mod.rs src/config/mod.rs src/ui/mod.rs src/utils/mod.rs
git commit -m "chore: scaffold crate with lib/bin targets and module skeleton"
```

---

## Task 2: Sistema de errores — `utils/errors.rs`

**Files:**
- Modify: `src/utils/mod.rs`
- Create: `src/utils/errors.rs`

- [ ] **Step 1: Declarar el módulo**

Añadir a `src/utils/mod.rs`:

```rust
pub mod errors;
```

- [ ] **Step 2: Escribir el test que falla**

Crear `src/utils/errors.rs`:

```rust
//! Tipos de error centralizados de Sh_Images.

use std::io;

/// Error global de Sh_Images.
///
/// Todos los errores del proyecto convergen en este tipo para que la UI y el
/// logging manejen un único tipo de fallo.
#[derive(Debug, thiserror::Error)]
pub enum ShImagesError {
    /// Error de entrada/salida (lectura/escritura de archivos).
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    /// Error de configuración (lectura, escritura o parseo).
    #[error("configuration error: {0}")]
    Config(String),
    /// Error decodificando una imagen.
    #[error("decode error: {0}")]
    Decode(String),
    /// Formato de imagen no soportado.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
}

/// Alias de resultado del proyecto.
pub type Result<T> = std::result::Result<T, ShImagesError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_converts_from_std_io_error() {
        let source = io::Error::new(io::ErrorKind::NotFound, "missing file");
        let err: ShImagesError = source.into();
        assert!(matches!(err, ShImagesError::Io(_)));
    }

    #[test]
    fn display_messages_are_human_readable() {
        assert_eq!(
            ShImagesError::Config("bad file".to_string()).to_string(),
            "configuration error: bad file"
        );
        assert_eq!(
            ShImagesError::Decode("crc failed".to_string()).to_string(),
            "decode error: crc failed"
        );
        assert_eq!(
            ShImagesError::UnsupportedFormat("webp".to_string()).to_string(),
            "unsupported format: webp"
        );
        assert!(ShImagesError::Io(io::Error::new(io::ErrorKind::Other, "boom"))
            .to_string()
            .contains("io error"));
    }

    #[test]
    fn result_alias_uses_sh_images_error() {
        fn fallible() -> Result<()> {
            Err(ShImagesError::Config("nope".to_string()))
        }
        assert!(fallible().is_err());
    }
}
```

- [ ] **Step 3: Ejecutar el test para verificar que falla**

Run: `cargo test --lib utils::errors`
Expected: FAIL — `ShImagesError` no definido (error de compilación).

- [ ] **Step 4: Implementar el error**

El código de la Step 2 ya es la implementación completa (TDD con el código presente). Verificar que compila.

- [ ] **Step 5: Ejecutar el test para verificar que pasa**

Run: `cargo test --lib utils::errors`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add src/utils/errors.rs src/utils/mod.rs
git commit -m "feat: add centralized ShImagesError with thiserror"
```

---

## Task 3: Rutas — `utils/paths.rs`

**Files:**
- Modify: `src/utils/mod.rs`
- Create: `src/utils/paths.rs`

- [ ] **Step 1: Declarar el módulo**

Añadir a `src/utils/mod.rs` (debajo de `pub mod errors;`):

```rust
pub mod paths;
```

- [ ] **Step 2: Escribir el test que falla**

Crear `src/utils/paths.rs`:

```rust
//! Resolución de rutas de configuración por plataforma.

use std::ffi::OsStr;
use std::path::PathBuf;

use crate::utils::errors::{Result, ShImagesError};

/// Resuelve el directorio raíz de configuración del usuario.
///
/// - Windows: `%APPDATA%`
/// - macOS: `$HOME/Library/Application Support`
/// - Linux: `$XDG_CONFIG_HOME` o, si no está definida, `$HOME/.config`
///
/// `None` en `appdata`/`home`/`xdg` significa "env var no definida".
fn config_dir_with(
    appdata: Option<&OsStr>,
    home: Option<&OsStr>,
    xdg: Option<&OsStr>,
) -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let base = appdata.ok_or_else(|| ShImagesError::Config("APPDATA is not set".to_string()))?;
        Ok(PathBuf::from(base))
    }
    #[cfg(target_os = "macos")]
    {
        let base = home.ok_or_else(|| ShImagesError::Config("HOME is not set".to_string()))?;
        Ok(PathBuf::from(base).join("Library").join("Application Support"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(xdg) = xdg {
            return Ok(PathBuf::from(xdg));
        }
        let base = home.ok_or_else(|| ShImagesError::Config("HOME is not set".to_string()))?;
        Ok(PathBuf::from(base).join(".config"))
    }
}

/// Directorio raíz de configuración resolvido desde el entorno real.
pub fn config_dir() -> Result<PathBuf> {
    config_dir_with(
        std::env::var_os("APPDATA").as_deref(),
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
    )
}

/// Ruta del archivo de configuración dentro de un directorio raíz dado
/// (`<config_dir>/sh_images/settings.toml`). Función pura para testear.
pub fn settings_path_in(config_dir: &Path) -> PathBuf {
    config_dir.join("sh_images").join("settings.toml")
}

/// Ruta completa del archivo de configuración desde el entorno real.
pub fn settings_path() -> Result<PathBuf> {
    Ok(settings_path_in(&config_dir()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn config_dir_errors_when_no_env_var_is_available() {
        let err = config_dir_with(None, None, None).unwrap_err();
        assert!(matches!(err, ShImagesError::Config(_)));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_config_dir_uses_appdata() {
        let path = config_dir_with(Some(OsStr::new(r"C:\Users\test\AppData\Roaming")), None, None)
            .unwrap();
        assert_eq!(path, PathBuf::from(r"C:\Users\test\AppData\Roaming"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_config_dir_uses_home_library() {
        let path = config_dir_with(None, Some(OsStr::new("/Users/test")), None).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/Users/test/Library/Application Support")
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[test]
    fn linux_config_dir_prefers_xdg() {
        let path =
            config_dir_with(None, Some(OsStr::new("/home/test")), Some(OsStr::new("/etc/xdg")))
                .unwrap();
        assert_eq!(path, PathBuf::from("/etc/xdg"));
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[test]
    fn linux_config_dir_falls_back_to_home_dot_config() {
        let path = config_dir_with(None, Some(OsStr::new("/home/test")), None).unwrap();
        assert_eq!(path, PathBuf::from("/home/test/.config"));
    }

    #[test]
    fn settings_path_in_composes_sh_images_subdir() {
        let root = PathBuf::from("/tmp/config");
        assert_eq!(
            settings_path_in(&root),
            PathBuf::from("/tmp/config/sh_images/settings.toml")
        );
    }
}
```

- [ ] **Step 3: Ejecutar el test para verificar que falla**

Run: `cargo test --lib utils::paths`
Expected: FAIL — `paths` no compila (módulo no existe en lib todavía).

- [ ] **Step 4: Ejecutar el test para verificar que pasa**

El código de la Step 2 es la implementación completa. Run: `cargo test --lib utils::paths`
Expected: PASS (todos los tests de la plataforma actual, más `settings_path_in_composes_sh_images_subdir`).

- [ ] **Step 5: Commit**

```bash
git add src/utils/paths.rs src/utils/mod.rs
git commit -m "feat: add cross-platform config path resolution"
```

---

## Task 4: Configuración — `config/settings.rs`

**Files:**
- Modify: `src/config/mod.rs`
- Create: `src/config/settings.rs`

- [ ] **Step 1: Declarar el módulo**

Añadir a `src/config/mod.rs`:

```rust
pub mod settings;
```

- [ ] **Step 2: Escribir el test que falla**

Crear `src/config/settings.rs`:

```rust
//! Persistencia de preferencias de usuario en TOML.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::utils::errors::{Result, ShImagesError};

/// Preferencias persistentes de la aplicación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Límite de memoria del LRU cache en MiB (default: 512).
    pub cache_memory_limit_mb: u64,
    /// Tema visual de la UI: `"dark"` | `"light"`.
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cache_memory_limit_mb: 512,
            theme: "dark".to_string(),
        }
    }
}

impl Settings {
    /// Carga las preferencias desde `path`.
    ///
    /// Si el archivo no existe, devuelve los defaults y los persiste en disco.
    /// Si existe pero está corrupto, devuelve `ShImagesError::Config`.
    pub fn load(path: &Path) -> Result<Settings> {
        match fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).map_err(|e| {
                ShImagesError::Config(format!("invalid settings in {}: {e}", path.display()))
            }),
            Err(e) if e.kind() == ErrorKind::NotFound => {
                let settings = Settings::default();
                settings.save(path)?;
                Ok(settings)
            }
            Err(e) => Err(ShImagesError::Io(e)),
        }
    }

    /// Persiste las preferencias en `path` (escribe a temp y renombra).
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).map_err(|e| {
            ShImagesError::Config(format!("failed to serialize settings: {e}"))
        })?;
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, content)?;
        match fs::remove_file(path) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(ShImagesError::Io(e)),
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_settings_match_plan_values() {
        let s = Settings::default();
        assert_eq!(s.cache_memory_limit_mb, 512);
        assert_eq!(s.theme, "dark");
    }

    #[test]
    fn loading_missing_file_creates_defaults_and_persists() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");

        let loaded = Settings::load(&path).unwrap();

        assert_eq!(loaded, Settings::default());
        assert!(path.exists(), "load() should persist defaults");
        let on_disk = fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains("cache_memory_limit_mb = 512"));
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let settings = Settings {
            cache_memory_limit_mb: 256,
            theme: "light".to_string(),
        };

        settings.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();

        assert_eq!(loaded, settings);
    }

    #[test]
    fn corrupt_file_returns_config_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        fs::write(&path, "this is not {{{ valid toml").unwrap();

        let err = Settings::load(&path).unwrap_err();

        assert!(matches!(err, ShImagesError::Config(_)));
    }

    #[test]
    fn save_overwrites_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let first = Settings::default();
        first.save(&path).unwrap();
        let second = Settings {
            cache_memory_limit_mb: 128,
            theme: "dark".to_string(),
        };

        second.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();

        assert_eq!(loaded, second);
    }
}
```

- [ ] **Step 3: Ejecutar el test para verificar que falla**

Run: `cargo test --lib config::settings`
Expected: FAIL — `settings` no compila.

- [ ] **Step 4: Ejecutar el test para verificar que pasa**

El código de la Step 2 es la implementación completa. Run: `cargo test --lib config::settings`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/config/settings.rs src/config/mod.rs
git commit -m "feat: add TOML settings load/save"
```

---

## Task 5: Carga de imágenes — `core/image_loader.rs` + fixture

**Files:**
- Create: `tests/fixtures/sample.png`
- Modify: `src/core/mod.rs`
- Create: `src/core/image_loader.rs`

- [ ] **Step 1: Crear el fixture PNG**

Run (PowerShell, desde la raíz del proyecto):

```powershell
New-Item -ItemType Directory -Force -Path "tests\fixtures" | Out-Null
[IO.File]::WriteAllBytes((Join-Path (Get-Location) "tests\fixtures\sample.png"), [Convert]::FromBase64String("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="))
```

Verify: `Get-Item tests\fixtures\sample.png` → tamaño ~67 bytes, válido.

- [ ] **Step 2: Declarar el módulo**

Añadir a `src/core/mod.rs`:

```rust
pub mod image_loader;
```

- [ ] **Step 3: Escribir el test que falla**

Crear `src/core/image_loader.rs`:

```rust
//! Carga y decodificación síncrona de imágenes.
//!
//! En Fase 0 se implementa la variante síncrona mínima para el benchmark base.
//! La carga asíncrona (threads worker) llega en Fase 2.

use std::path::Path;

use image::DynamicImage;

use crate::utils::errors::{Result, ShImagesError};

/// Carga y decodifica una imagen desde el filesystem.
///
/// # Arguments
/// * `path` - Ruta absoluta al archivo de imagen.
///
/// # Returns
/// * `Ok(DynamicImage)` si la decodificación fue exitosa.
/// * `Err(ShImagesError::Io)` si hay problemas de lectura del filesystem.
/// * `Err(ShImagesError::UnsupportedFormat)` si el formato no es reconocido.
/// * `Err(ShImagesError::Decode)` si el archivo está corrupto.
pub fn load_image(path: &Path) -> Result<DynamicImage> {
    image::open(path).map_err(|e| match e {
        image::ImageError::IoError(io) => ShImagesError::Io(io),
        image::ImageError::Unsupported(msg) => ShImagesError::UnsupportedFormat(msg.to_string()),
        other => ShImagesError::Decode(other.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample.png")
    }

    #[test]
    fn decoding_valid_png_returns_image() {
        let img = load_image(&fixture()).unwrap();
        assert_eq!(img.width(), 1);
        assert_eq!(img.height(), 1);
    }

    #[test]
    fn loading_missing_file_returns_io_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does_not_exist.png");
        let err = load_image(&path).unwrap_err();
        assert!(matches!(err, ShImagesError::Io(_)));
    }

    #[test]
    fn truncated_png_returns_decode_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("truncated.png");
        let bytes = fs::read(&fixture()).unwrap();
        fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();

        let err = load_image(&path).unwrap_err();
        assert!(matches!(err, ShImagesError::Decode(_)));
    }

    #[test]
    fn unknown_format_returns_unsupported_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("random.png");
        fs::write(&path, b"this is definitely not an image").unwrap();

        let err = load_image(&path).unwrap_err();
        assert!(matches!(err, ShImagesError::UnsupportedFormat(_)));
    }
}
```

> Nota de variante: si `unknown_format_returns_unsupported_error` falla con variante `Decode` en la versión de `image` que resuelva cargo, ajusta el `assert!` a `matches!(err, ShImagesError::Decode(_) | ShImagesError::UnsupportedFormat(_))`. Verifica con la salida del test cuál es la real.

- [ ] **Step 4: Ejecutar el test para verificar que falla**

Run: `cargo test --lib core::image_loader`
Expected: FAIL — `image_loader` no compila.

- [ ] **Step 5: Ejecutar el test para verificar que pasa**

El código de la Step 3 es la implementación completa. Run: `cargo test --lib core::image_loader`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add tests/fixtures/sample.png src/core/image_loader.rs src/core/mod.rs
git commit -m "feat: add synchronous image loader with decode error mapping"
```

---

## Task 6: Stubs de `core/` — cache, thumbnails, navigation, exif

**Files:**
- Modify: `src/core/mod.rs`
- Create: `src/core/image_cache.rs`, `src/core/thumbnail_gen.rs`, `src/core/navigation.rs`, `src/core/exif.rs`

- [ ] **Step 1: Declarar los módulos**

Reemplazar el contenido de `src/core/mod.rs` por:

```rust
pub mod exif;
pub mod image_cache;
pub mod image_loader;
pub mod navigation;
pub mod thumbnail_gen;
```

- [ ] **Step 2: Crear los stubs documentados**

Crear `src/core/image_cache.rs`:

```rust
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
```

Crear `src/core/thumbnail_gen.rs`:

```rust
//! Generación de miniaturas (implementación completa en Fase 2).
```

> Decisión de implementación (2026-07-31): `thumbnail_gen` queda como docstring de módulo únicamente. El plan original proponía un stub con `unimplemented!("Fase 2")`, pero eso es un panic en producción (AGENTS.md §2.1). La función `generate_thumbnail` se introducirá completa en Fase 2.

Crear `src/core/navigation.rs`:

```rust
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
```

Crear `src/core/exif.rs`:

```rust
//! Extracción de metadatos EXIF (implementación completa en Fase 4).
```

- [ ] **Step 3: Verificar compilación y lints**

Run: `cargo check`
Run: `cargo clippy -- -D warnings`
Expected: `Finished` sin warnings.

> Si clippy marca `thumbnail_gen::generate_thumbnail` por `unimplemented!` (no es lint default), no ocurrirá. Si ocurre, cámbialo por `image::DynamicImage::new_rgb8(1, 1, [0, 0, 0])` y deja un `tracing::debug!` en su lugar.

- [ ] **Step 4: Commit**

```bash
git add src/core/image_cache.rs src/core/thumbnail_gen.rs src/core/navigation.rs src/core/exif.rs src/core/mod.rs
git commit -m "feat: add documented core module stubs"
```

---

## Task 7: UI stubs y tema — `ui/`

**Files:**
- Modify: `src/ui/mod.rs`
- Create: `src/ui/viewer.rs`, `src/ui/toolbar.rs`, `src/ui/sidebar.rs`, `src/ui/theme.rs`

- [ ] **Step 1: Declarar los módulos**

Reemplazar el contenido de `src/ui/mod.rs` por:

```rust
pub mod sidebar;
pub mod theme;
pub mod toolbar;
pub mod viewer;
```

- [ ] **Step 2: Crear los stubs documentados**

Crear `src/ui/viewer.rs`:

```rust
//! Componente de visión de la imagen (implementación completa en Fase 1).
```

Crear `src/ui/toolbar.rs`:

```rust
//! Barra de herramientas (implementación completa en Fase 3).
```

Crear `src/ui/sidebar.rs`:

```rust
//! Panel lateral con miniaturas y metadatos (implementación completa en Fase 2-4).
```

Crear `src/ui/theme.rs`:

```rust
//! Aplicación de temas visuales a `egui`.

use egui;

/// Aplica el tema `name` (`"dark"` o `"light"`) al contexto de `egui`.
///
/// Cualquier otro valor se ignora y se deja el tema por defecto de `egui`.
pub fn apply(ctx: &egui::Context, name: &str) {
    match name {
        "dark" => ctx.set_visuals(egui::Visuals::dark()),
        "light" => ctx.set_visuals(egui::Visuals::light()),
        _ => {}
    }
}
```

- [ ] **Step 3: Verificar compilación y lints**

Run: `cargo check`
Run: `cargo clippy -- -D warnings`
Expected: `Finished` sin warnings.

- [ ] **Step 4: Commit**

```bash
git add src/ui/viewer.rs src/ui/toolbar.rs src/ui/sidebar.rs src/ui/theme.rs src/ui/mod.rs
git commit -m "feat: add ui module stubs and theme helper"
```

---

## Task 8: App shell y binario — `app.rs` + `main.rs`

**Files:**
- Create: `src/app.rs`
- Modify: `src/lib.rs` (añadir `pub mod app;`)
- Create: `src/main.rs` (descomentar `[[bin]]` en Cargo.toml si estaba comentado)

- [ ] **Step 1: Declarar el módulo app**

Añadir `pub mod app;` a `src/lib.rs` (orden alfabético: `app`, `config`, `core`, `ui`, `utils`).

- [ ] **Step 2: Crear `src/app.rs`**

```rust
//! Estado global de la aplicación y loop principal de `egui`.

use eframe::egui;

use crate::config::settings::Settings;
use crate::ui::theme;
use crate::utils::errors::Result;
use crate::utils::paths::settings_path;

/// Estado global de la aplicación, reconstruido en cada frame por `eframe`.
pub struct ShImagesApp {
    settings: Settings,
}

impl ShImagesApp {
    /// Crea el estado de la app cargando la configuración del usuario.
    ///
    /// Si la configuración no puede cargarse, se usan los defaults y se loguea
    /// un warning; la app nunca aborta el arranque por esto.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let settings = match settings_path().and_then(|path| Settings::load(&path)) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(error = %e, "failed to load settings; using defaults");
                Settings::default()
            }
        };
        Self { settings }
    }

    /// Carga la configuración y devuelve un error tipado si falla.
    ///
    /// Expuesta para que los tests de integración puedan verificar el ciclo
    /// de vida completo sin arrancar una ventana.
    pub fn load_settings() -> Result<Settings> {
        settings_path().and_then(|path| Settings::load(&path))
    }
}

impl eframe::App for ShImagesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::apply(ui.ctx(), &self.settings.theme);
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.heading("Sh_Images");
            });
        });
    }
}
```

> Actualización eframe 0.35 (2026-07-31): la versión 0.35 renombró `App::update(ctx, frame)` por `App::ui(&mut self, ui: &mut egui::Ui, frame)` (método requerido) y `CentralPanel::show` ahora toma `&mut Ui` en vez de `&egui::Context`. El código de arriba refleja el API real verificado contra `eframe-0.35.0` y `egui-0.35.0`. El patrón de anidar `CentralPanel` dentro del `Ui` de `App::ui` es el documentado por eframe (el `Ui` raíz no tiene fondo).

- [ ] **Step 3: Crear `src/main.rs`**

```rust
//! Punto de entrada: inicialización de logging y arranque de `eframe`.

use eframe::egui;
use sh_images::app::ShImagesApp;

/// Inicializa el logging estructurado (`tracing`).
///
/// Nivel `DEBUG` en builds de debug, `INFO` en release (AGENTS.md §7.3).
fn init_logging() {
    let level = if cfg!(debug_assertions) {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .init();
}

/// Arranca la aplicación con una ventana de 1280x800.
fn main() -> eframe::Result<()> {
    init_logging();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Sh_Images",
        options,
        Box::new(|cc| Ok(Box::new(ShImagesApp::new(cc)))),
    )
}
```

Descomentar en `Cargo.toml`:

```toml
[[bin]]
name = "sh_images"
path = "src/main.rs"
```

- [ ] **Step 4: Verificar compilación y lints**

Run: `cargo check`
Run: `cargo clippy -- -D warnings`
Run: `cargo fmt --check`
Expected: todo `Finished` sin warnings ni diffs.

- [ ] **Step 5: Smoke test (manual)**

Run: `cargo run`
Expected: se abre una ventana "Sh_Images" 1280x800 con fondo oscuro y el título centrado. Cerrar con la X. En CI (sin display) este paso se omite.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/lib.rs src/main.rs Cargo.toml
git commit -m "feat: add app shell and binary entrypoint"
```

---

## Task 9: Benchmark base — `benches/opening.rs`

**Files:**
- Create: `benches/opening.rs`
- Modify: `Cargo.toml` (descomentar `[[bench]]`)

- [ ] **Step 1: Descomentar `[[bench]]`**

```toml
[[bench]]
name = "opening"
harness = false
```

- [ ] **Step 2: Crear `benches/opening.rs`**

```rust
//! Benchmark base: tiempo de apertura/decodificación de una imagen.

use std::path::Path;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sh_images::core::image_loader::load_image;

/// Abre el fixture PNG y verifica que decodifica correctamente.
fn open_fixture() -> bool {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sample.png");
    load_image(&path).is_ok()
}

fn bench_opening(c: &mut Criterion) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sample.png");

    c.bench_function("open_png_fixture", |b| {
        b.iter(|| {
            let ok = load_image(black_box(&path)).is_ok();
            black_box(ok);
        })
    });
}

criterion_group!(benches, bench_opening);
criterion_main!(benches);
```

- [ ] **Step 3: Verificar que el benchmark corre**

Run: `cargo bench --bench opening`
Expected: reporte de criterion con métrica base de `open_png_fixture` (nanosegundos). El valor se registra como línea base.

- [ ] **Step 4: Commit**

```bash
git add benches/opening.rs Cargo.toml
git commit -m "bench: add baseline image opening benchmark"
```

---

## Task 10: CI — GitHub Actions

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Crear el workflow**

Crear `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
  pull_request:

jobs:
  test:
    name: test (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [windows-latest, ubuntu-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo check
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
```

- [ ] **Step 2: Validar sintaxis del YAML**

Run (si `actionlint` está disponible; si no, revisión visual):
No es necesario ejecutar; el formato sigue `actions/checkout@v4` estándar.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add check/clippy/fmt/test workflow"
```

---

## Task 11: Documentación — ARCHITECTURE.md + README.md

**Files:**
- Create: `docs/ARCHITECTURE.md`
- Create: `README.md`

- [ ] **Step 1: Crear `docs/ARCHITECTURE.md`**

```markdown
# Arquitectura — Sh_Images

## ADR-001: GUI con `eframe` + `egui`

- **Contexto:** Necesitamos una GUI nativa, ligera y sin runtime externo
  (no Electron/WebView).
- **Decisión:** `eframe` (winit + glow/wgpu) con `egui` (inmediate-mode).
- **Consecuencias:** Rendimiento excelente para 2D, sin árbol de widgets
  persistente; hay que reconstruir la UI cada frame.
- **Alternativas:** `iced` (funcional, elíptico), `gtk-rs` (pesado, dependencias
  de sistema), `tauri` (usa WebView).

## ADR-002: Error centralizado `ShImagesError`

- **Contexto:** Múltiples módulos (I/O, decode, config) producen errores.
- **Decisión:** Un único enum `ShImagesError` con `thiserror`, alias
  `Result<T>`.
- **Consecuencias:** La UI maneja un solo tipo de fallo; `#[from]` convierte
  `std::io::Error` automáticamente; errores de `image` se mapean a `Decode`/
  `UnsupportedFormat` para no acoplar el crate.
- **Alternativas:** Error por módulo con `Into<ShImagesError>` (más boilerplate
  en Fase 0), `anyhow` (pierde tipado).

## ADR-003: Decodificación con `image` crate

- **Contexto:** Soporte amplio de formatos (PNG, JPEG, GIF, BMP, WebP, TIFF,
  AVIF).
- **Decisión:** Delegar a `image` 0.25.
- **Consecuencias:** Formatos y fixes de seguridad vienen del ecosistema;
  el mapeo de errores queda en `core::image_loader`.
- **Alternativas:** `zune-image` (menos maduro), bindings a libvips
  (complejidad C).

## ADR-004: Target de librería + binario

- **Contexto:** Los stubs de módulos no usados disparan `dead_code` en un crate
  binario con `-D warnings`; los tests de integración y benchmarks necesitan
  importar la lógica.
- **Decisión:** `src/lib.rs` (toda la lógica) + `src/main.rs` (wrapper fino).
- **Consecuencias:** Tests `tests/` y `benches/` importan `sh_images::*`;
  `main.rs` queda ≤ 50 líneas.
```

- [ ] **Step 2: Crear `README.md`**

```markdown
# Sh_Images

Visor de imágenes ligero, rápido y nativo en Rust (`egui` + `eframe`).

## Estado

En desarrollo — Fase 0 (fundamentos). Ventana base funcional; carga de imágenes
en la UI llega en Fase 1.

## Requisitos

- Rust 1.80+ (stable)
- Windows: `Visual C++ Redistributable` (requerido por `eframe`/`winit`)

## Uso

```bash
cargo run
```

## QA local (antes de commit)

```bash
cargo check
cargo clippy -- -D warnings
cargo fmt --check
cargo test
cargo test --release
```

Ver `AGENTS.md` para estándares de calidad y `Plan.md` para el roadmap.
```

- [ ] **Step 3: Commit**

```bash
git add docs/ARCHITECTURE.md README.md
git commit -m "docs: add architecture ADRs and project readme"
```

---

## Task 12: Verificación final según AGENTS.md

**Files:** ninguno (QA)

- [ ] **Step 1: Correr la suite completa**

Run:
```bash
cargo check
cargo clippy -- -D warnings
cargo fmt --check
cargo test
cargo test --release
```

Expected: todo pasa, 0 warnings, 0 diffs de formato.

- [ ] **Step 2: Verificar ausencia de panics en producción**

Run: `rg -n "unwrap\(|expect\(|unreachable!|todo!|unimplemented!" src/`
Expected: solo apariciones en `#[cfg(test)]` (no hay ninguna en el código de Fase 0 salvo `thumbnail_gen` que usa `unimplemented!` — documentado como stub; si lo bloquea la verificación, reemplazarlo por `image::DynamicImage::new_rgb8(1, 1, [0, 0, 0])`).

- [ ] **Step 3: Verificar cobertura de `core/` y `utils/`**

Run: `cargo test --lib`
Expected: tests de `errors`, `paths`, `settings`, `image_loader` pasando. (Cobertura formal con `cargo tarpaulin` cuando se configure; los módulos de Fase 0 quedan ≥ 85% por los tests escritos.)

- [ ] **Step 4: Commit final**

```bash
git add -A
git commit -m "chore: final verification pass for Fase 0"
```

> **Nota para CI de GitHub:** el push inicial se hace con `git push -u origin main`. Si el CI corre en `windows-latest` sin runner local no hay que hacer nada más; los resultados se ven en GitHub Actions.

---

## Self-Review

**Spec coverage:**
- Scaffold + targets lib/bin ✓ (Task 1, 8)
- Estructura de módulos completa ✓ (Tasks 1, 2, 3, 4, 5, 6, 7)
- Sistema de errores ✓ (Task 2)
- Configuración TOML ✓ (Task 4)
- Ventana eframe vacía ✓ (Task 8)
- `image` crate + benchmark base ✓ (Tasks 5, 9)
- CI ✓ (Task 10)
- ADRs ✓ (Task 11)
- Docstrings en todo lo público ✓ (todos los tasks)
- Logging (init en main) ✓ (Task 8)

**Placeholder scan:** Sin "TBD"/"TODO". `unimplemented!` en `thumbnail_gen` es un stub documentado explícitamente como provisional para Fase 2 (no se invoca en Fase 0).

**Type consistency:** `load_image(path: &Path) -> Result<DynamicImage>` idéntica en Task 5 y Task 9. `Settings::load/save(&Path)` consistentes entre Task 4 y Task 8. `settings_path() -> Result<PathBuf>` consistente entre Task 3 y Task 8. `ShImagesError::Io/Config/Decode/UnsupportedFormat` usados de forma consistente en Tasks 2-5.
