# Arquitectura — Sh_Images

## ADR-001: GUI con `eframe` + `egui`

- **Contexto:** Necesitamos una GUI nativa, ligera y sin runtime externo
  (no Electron/WebView).
- **Decisión:** `eframe` (winit + wgpu; glow opcional) con `egui` (immediate-mode).
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
  `UnsupportedFormat` para no acoplar el crate. Las variantes `Config`/`Decode`/
  `UnsupportedFormat` llevan `String` como mensaje de contexto (patrón thiserror
  convencional); la identidad de la variante es lo que el código hace `match`,
  no la cadena.
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
- **Consecuencias:** Los benchmarks (`benches/`) importan `sh_images::*`;
  los tests de integración futuros (`tests/`) podrán importar la lógica
  sin acoplar a la UI; `main.rs` queda ≤ 50 líneas.

## ADR-005: Dependencias en sus versiones estables actuales

- **Contexto:** El plan original fijaba `eframe`/`egui` 0.31 y `toml` 0.8
  (línea conocida de 2025).
- **Decisión:** Usar `eframe`/`egui` 0.35 y `toml` 1.x (líneas estables
  actuales, 2026), y commitear `Cargo.lock`.
- **Consecuencias:** La API de eframe 0.35 cambió (`App::ui` en vez de
  `App::update`; `CentralPanel::show` con `&mut Ui`); MSRV sube a Rust 1.92.
  El código del proyecto quedó adaptado a esa API.
- **Alternativas:** Mantener 0.31 (acumular deuda de migración), subir más
  tarde (migración más costosa con más código).
