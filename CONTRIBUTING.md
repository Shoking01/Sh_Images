<div align="center">

# Contributing to Sh_Images

[English](#english) | [Español](#español)

</div>

---

## English

### Code Philosophy
- **No `unsafe`** without documented justification (`// SAFETY: ...`)
- **No `.unwrap()` / `.expect()`** in production code — use `match`, `if let`, or `?`
- **No `println!`** — use `tracing` for structured logging
- **`cargo fmt` + `cargo clippy -- -D warnings`** before every commit

### Testing
- Unit tests for all `core/` modules (≥ 90% coverage target)
- Integration tests for critical flows (opening, navigation, zoom, error, config)
- Run `cargo test --release` — tests pass in release mode too

### Pull Request Checklist
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test` passes (all 181+ tests)
- [ ] `cargo test --release` passes
- [ ] New public functions have docstrings (`///`)
- [ ] New modules are documented if architecturally significant
- [ ] No regressions in existing benchmarks (`cargo bench`)

### Release Process
1. Verify all changes pass checks above
2. Tag release: `git tag v0.X.0 && git push origin v0.X.0`
3. CI builds the MSI and singles it as artifact
4. Download MSI from GitHub Actions → upload to GitHub Releases

---

## Español

### Filosofía de codigo
- **No uses `unsafe` o `unwrap()`** en código de producción
- **Usa `tracing`** en lugar de `println!`
- **Ejecutá `cargo fmt` y `cargo clippy`** antes de cada commit

### Tests
- Tests unitarios para todo `core/` (objetivo ≥ 90% cobertura)
- Tests de integración para los 9 flujos críticos
- `cargo test --release` también debe pasar

### Checklist de PR
- [ ] `cargo fmt --check` pasa
- [ ] `cargo clippy -- -D warnings` pasa
- [ ] `cargo test` pasa (todos los tests)
- [ ] Nuevas funciones públicas documentadas
- [ ] Sin regresiones en benchmarks

### Proceso de release
1. Mergear el PR
2. `git tag v0.X.0 && git push origin v0.X.0`
3. CI genera el MSI automático
4. Descargar artefacto → subir a GitHub Releases