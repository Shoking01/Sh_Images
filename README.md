# Sh_Images

Visor de imágenes ligero, rápido y nativo en Rust (`egui` + `eframe`).

## Estado

En desarrollo — Fase 0 (fundamentos). Ventana base funcional; carga de imágenes
en la UI llega en Fase 1.

## Requisitos

- Rust 1.92+ (stable)
- Windows: `Visual C++ Redistributable` (requerido por `eframe`/`winit`)

## Uso

```bash
cargo run
```

## QA local (antes de commit)

```bash
cargo check
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
cargo test --release
```

Ver `AGENTS.md` para estándares de calidad y `Plan.md` para el roadmap.
