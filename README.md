# Sh_Images

Visor de imágenes ligero, rápido y nativo en Rust (`egui` + `eframe`).

## Estado

En desarrollo — Fase 4 (metadatos avanzados) pendiente. Completas: Fases 0–3
(fundamentos, visor básico, cache y rendimiento, UI/UX polish). Ver `Plan.md`
para el roadmap.

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

## Desarrollo

```bash
# Suite de tests completa (unitarios + integración)
cargo test

# Tests de integración de los flujos críticos (AGENTS.md §8.1)
cargo test --test integration

# Benchmarks de rendimiento (AGENTS.md §6.2)
cargo bench

# Benchmarks sin ejecutar (solo compilar)
cargo bench --no-run

# QA completo pre-commit
cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test
```

Ver `AGENTS.md` para estándares de calidad y `Plan.md` para el roadmap.
