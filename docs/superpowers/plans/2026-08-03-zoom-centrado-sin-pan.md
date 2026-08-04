# Zoom centrado sin pan — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transformar el visor para que la imagen quede siempre fija y centrada en el canvas, con zoom de rueda anclado al centro y sin arrastre por mouse.

**Architecture:** `ViewTransform` deja de tener `pan`; la esquina TL de la imagen se calcula siempre como `center - half`, por lo que la imagen queda centrada por construcción bajo cualquier zoom. El zoom de rueda cambia de `apply_zoom_at(anchor)` (anclado al cursor) a `apply_center(factor)` (anclado al centro del canvas). En `ui::viewer` se elimina el bloque de arrastre y el sense pasa a `hover()`. En `app` solo se elimina la referencia a `resp.panned`.

**Tech Stack:** Rust, `egui`/`eframe`, `insta` (snapshot), test de integración existente en `tests/integration.rs`. Sin dependencias nuevas.

**Verificación base:** `cargo check --locked && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test`. Número de tests de base: 137 (unit) + 6 (integración) — ver spec §Verificación.

---

## File Map

| Archivo | Acción | Responsabilidad |
|---------|--------|------------------|
| `src/core/view.rs` | Modificar | Eliminar `pan`, `pan_by`, `apply_zoom_at`; añadir `apply_center`; simplificar `image_origin_screen`/`fit`; actualizar tests unitarios |
| `src/ui/viewer.rs` | Modificar | Quitar arrastre, `Sense::hover()`, usar `apply_center`, eliminar `panned` del `ViewResponse` |
| `src/app.rs` | Modificar | `if resp.zoomed || resp.panned` → `if resp.zoomed` |
| `tests/integration.rs` | Modificar | Reescribir `flujo_zoom_pan_fit`; quitar `assert_eq!(t.pan, ...)` en rotación |

---

### Task 1: Añadir `apply_center` a `core::view` (TDD)

**Files:**
- Modify: `src/core/view.rs` (estructura `ViewTransform`, después de `fit()`)
- Test: `src/core/view.rs` (módulo `tests`)

- [ ] **Step 1: Write the failing tests.** Añadir los 4 tests justo después del test `fit_zoom_returns_1_on_zero_dimension` (línea 203), dentro del bloque `mod tests`:

```rust
    #[test]
    fn apply_center_keeps_image_centered() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let center = Vec2::new(250.0, 250.0);
        for _ in 0..6 {
            let origin = t.image_origin_screen();
            let img_center = origin.add(Vec2::new(1000.0 * t.zoom * 0.5, 1000.0 * t.zoom * 0.5));
            assert!(approx(img_center.x, center.x), "centrada en x en zoom {}", t.zoom);
            assert!(approx(img_center.y, center.y), "centrada en y en zoom {}", t.zoom);
            t.apply_center(2.0);
        }
    }

    #[test]
    fn apply_center_clamps_to_max_zoom() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let fit = t.fit_zoom();
        t.apply_center(1000.0);
        assert!(approx(t.zoom, fit * MAX_ZOOM));
    }

    #[test]
    fn apply_center_clamps_to_min_fit() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let fit = t.fit_zoom();
        t.apply_center(0.0001);
        assert!(approx(t.zoom, fit));
    }

    #[test]
    fn image_origin_is_always_centered() {
        let mut t = ViewTransform::new(Vec2::new(2000.0, 1000.0), Vec2::new(500.0, 500.0));
        let expected = |zoom: f32| {
            Vec2::new(
                (500.0 - 2000.0 * zoom) / 2.0,
                (500.0 - 1000.0 * zoom) / 2.0,
            )
        };
        t.apply_center(1.0);
        for _ in 0..8 {
            let o = t.image_origin_screen();
            let e = expected(t.zoom);
            assert!(approx(o.x, e.x), "TL-x centrada en zoom {}", t.zoom);
            assert!(approx(o.y, e.y), "TL-y centrada en zoom {}", t.zoom);
            t.apply_center(1.5);
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p sh_images view::tests::apply_center_keeps_image_centered -- --nocapture`
Expected: FAIL — ERROR "no method named `apply_center` found". (Lo mismo para los 3 tests restantes: método inexistente.)

- [ ] **Step 3: Write the minimal implementation.** Insertar `apply_center` en `impl ViewTransform`, inmediatamente después de `fit()` (línea 119):

```rust
    /// Aplica un factor de zoom anclado al centro del canvas, sin desplazamiento.
    ///
    /// Como la imagen queda siempre centrada (`image_origin_screen()` deriva la
    /// esquina de `zoom`), solo hay que escalar `self.zoom` y clamp lo entre
    /// `fit_zoom()` (mínimo, imagen completa) y `fit_zoom() * MAX_ZOOM`.
    pub fn apply_center(&mut self, factor: f32) {
        let fit = self.fit_zoom();
        let new_zoom = (self.zoom * factor).clamp(fit, fit * MAX_ZOOM);
        self.zoom = new_zoom;
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sh_images core::view`
Expected: PASS — los 4 tests nuevos + los 137 de base siguen pasando (`apply_zoom_at`/`pan` aún existen en este punto).

- [ ] **Step 5: Commit**

```bash
git add src/core/view.rs
git commit -m "feat(core): add center-anchored zoom without pan"
```

---

## Task 2: Eliminar `pan`/`pan_by`/`apply_zoom_at` de `core::view`

**Files:**
- Modify: `src/core/view.rs` — struct `ViewTransform` (campo `pan`), `image_origin_screen`, `fit`, métodos `apply_zoom_at`/`pan_by`
- Test: `src/core/view.rs` (tests que usan `apply_zoom_at`/`pan`)

- [ ] **Step 1: Update the struct and the math.** 
  - Quitar `pub pan: Vec2,` del struct `ViewTransform` (líneas 62-63).
  - En `new()` (línea 77) quitar la línea `pan: Vec2::ZERO,`.
  - En `image_origin_screen()` (línea 112): `center.sub(half).add(self.pan)` → `center.sub(half)`.
  - En `fit()` (línea 118): eliminar `self.pan = Vec2::ZERO;` (queda solo `self.zoom = self.fit_zoom();`).
  - Eliminar por completo el método `apply_zoom_at` (líneas 121-138).
  - Eliminar por completo el método `pan_by` (líneas 140-143).

- [ ] **Step 2: Actualizar los tests unitarios.** En `src/core/view.rs`:
 1. Eliminar los tests `apply_zoom_at_keeps_anchor_point_fixed`, `apply_zoom_at_clamps_to_max_zoom` (líneas 199-226) y `pan_by_moves_image_by_delta` (líneas 228-236).
 2. Reescribir `fit_resets_to_centered_initial` (líneas 248-263): quitar `apply_zoom_at`/`pan_by`; queda:

```rust
    #[test]
    fn fit_resets_to_centered_initial() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        t.apply_center(3.0);
        t.fit();
        let expected = t.fit_zoom();
        assert!(approx(t.zoom, expected), "fit restaura zoom");
        let expected_origin = Vec2::new(
            (500.0 - 1000.0 * expected) / 2.0,
            (500.0 - 1000.0 * expected) / 2.0,
        );
        let origin = t.image_origin_screen();
        assert!(approx(origin.x, expected_origin.x));
        assert!(approx(origin.y, expected_origin.y));
    }
```

 3. Reescribir `rotating_resets_to_fit_and_centers` (líneas 305-313), quitando `apply_zoom_at`/`pan_by`/`assert_eq!(t.pan, ...)`:

```rust
    #[test]
    fn rotating_resets_to_fit_and_centers() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        t.apply_center(3.0);
        t.rotate_cw();
        assert!(approx(t.zoom, t.fit_zoom()), "rota → fit");
        let origin = t.image_origin_screen();
        assert!(approx(origin.x, (500.0 - 1000.0 * t.zoom) / 2.0), "quedó centrada en x");
        assert!(approx(origin.y, (500.0 - 1000.0 * t.zoom) / 2.0), "quedó centrada en y");
    }
```

- [ ] **Step 3: Run the tests to verify pass**

Run: `cargo test -p sh_images core::view`
Expected: PASS — sin referencias a `pan`/`apply_zoom_at`.

- [ ] **Step 4: Commit**

```bash
git add src/core/view.rs
git commit -m "refactor(core): remove pan and cursor-anchored zoom from ViewTransform"
```

---

## Task 3: Actualizar `ui::viewer` — sin arrastre, zoom centrado

**Files:**
- Modify: `src/ui/viewer.rs`

- [ ] **Step 1: Cambiar `ViewResponse`, sense y zoom.** Aplicar estos 3 cambios en `src/ui/viewer.rs`:
 1. En `ViewResponse` (líneas 15-17) eliminar el campo `panned`:

```rust
    /// El usuario hizo arrastrando (pan).
    pub panned: bool,
```
eliminar y dejar solo:
```rust
    /// El usuario hizo zoom con la rueda.
    pub zoomed: bool,
```

 2. En `show()` línea 31: `egui::Sense::click_and_drag()` → `egui::Sense::hover()`.
 3. Bloque de zoom (líneas 81-91): reemplazar usando `apply_center`; eliminar cálculo de `anchor`:

```rust
    // Zoom con la rueda, anclado al centro del canvas.
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = (scroll * 0.001).exp();
            transform.apply_center(factor);
            result.zoomed = true;
            ui.ctx().request_repaint();
        }
    }
```
 4. Eliminar todo el bloque de pan (líneas 93-99):

```rust
    // Pan con arrastre.
    if response.dragged() {
        let delta = response.drag_delta();
        transform.pan_by(Vec2::new(delta.x, delta.y));
        result.panned = true;
        ui.ctx().request_repaint();
    }
```

- [ ] **Step 2: Verify compile**

Run: `cargo check --locked`
Expected: PASS — no debe quedar ninguna referencia a `panned`, `pan_by` ni `dragged`.

- [ ] **Step 3: Verify clippy & fmt**

Run: `cargo clippy --all-targets -- -D warnings; cargo fmt --check`
Expected: PASS sin warnings; PASS format.

- [ ] **Step 4: Commit**

```bash
git add src/ui/viewer.rs
git commit -m "feat(ui): center-anchored scroll zoom and remove mouse pan in viewer"
```

---

## Task 4: Actualizar `app.rs` — quitar `resp.panned`

**Files:**
- Modify: `src/app.rs` (línea 599)

- [ ] **Step 1: Editar la condición de interacción.**
 En `src/app.rs`, dentro del bloque `Some(texture) =>` (línea 599):

```rust
                    if resp.zoomed || resp.panned {
                        self.user_interacted = true;
                    }
```
→
```rust
                    if resp.zoomed {
                        self.user_interacted = true;
                    }
```

- [ ] **Step 2: Verify compile**
Run: `cargo check --locked`
Expected: PASS.

- [ ] **Step 3: Commit**
```bash
git add src/app.rs
git commit -m "refactor(app): track interaction on zoom only after pan removal"
```

---

## Task 5: Actualizar tests de integración

**Files:**
- Modify: `tests/integration.rs` (`flujo_zoom_pan_fit`, `flujo_rotacion_visual`)

- [ ] **Step 1: Reescribir `flujo_zoom_pan_fit`.** Reemplazar el cuerpo completo del test (líneas 99-134) por:

```rust
/// Flujo 3 — Zoom/Fit: zoom in centrado → fit restaura.
#[test]
fn flujo_zoom_pan_fit() {
    let mut t = ViewTransform::new(Vec2::new(2000.0, 1000.0), Vec2::new(500.0, 500.0));
    let fit = t.fit_zoom();
    let center = Vec2::new(250.0, 250.0);

    // El centro de la imagen está bajo el centro del canvas (ancla fija).
    let origin = t.image_origin_screen();
    let img_center = origin.add(Vec2::new(2000.0 * t.zoom * 0.5, 1000.0 * t.zoom * 0.5));
    assert!((img_center.x - center.x).abs() < 1e-3, "centrada en x");
    assert!((img_center.y - center.y).abs() < 1e-3, "centrada en y");

    // Zoom in con rueda: sigue centrada.
    t.apply_center(2.0);
    assert!(t.zoom > fit, "zoom in supera el fit");
    let origin2 = t.image_origin_screen();
    let img_center2 = origin2.add(Vec2::new(2000.0 * t.zoom * 0.5, 1000.0 * t.zoom * 0.5));
    assert!((img_center2.x - center.x).abs() < 1e-3, "sigue centrada en x");
    assert!((img_center2.y - center.y).abs() < 1e-3, "sigue centrada en y");

    // Fit restaura zoom y entrena.
    t.fit();
    let origin_fit = t.image_origin_screen();
    let expected = Vec2::new((500.0 - 2000.0 * fit) / 2.0, (500.0 - 1000.0 * fit) / 2.0);
    assert!((t.zoom - fit).abs() < 1e-3, "fit restaura zoom");
    assert!((origin_fit.x - expected.x).abs() < 1e-3, "fit centra en x");
    assert!((origin_fit.y - expected.y).abs() < 1e-3, "fit centra en y");
}
```

- [ ] **Step 2: Quitar la aserción de `pan` en rotación.** En `flujo_rotacion_visual` (línea 229) eliminar:

```rust
    assert_eq!(t.pan, Vec2::ZERO, "rota → pan 0");
```

- [ ] **Step 3: Run the tests to verify pass**

Run: `cargo test`
Expected: PASS — 143 tests (137 unit + 6 integración, ajustados).

- [ ] **Step 4: Run full QA gate**

Run: `cargo check --locked && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test && cargo test --release`
Expected: PASS en todos.

- [ ] **Step 5: Commit**

```bash
git add tests/integration.rs
git commit -m "test: update integration flows for centered zoom without pan"
```

---

## Task 6: Smoke test manual

Verificar de forma manual que la app compila y el comportamiento visual resulta como la spec pide.

- [ ] **Step 1: Compilar en release**
Run: `cargo build --release`
Expected: exe generado en `target/release/sh_images`.

- [ ] **Step 2: Smoke manual**
Abrir la app → Archivo → Abrir… → seleccionar una imagen. Verificar:
 1. La imagen aparece **fija y centrada** al cargar.
 2. La rueda del mouse hace **zoom anclado al centro** (el centro de la imagen no se aparta del centro del canvas).
 3. **No se puede arrastrar** la imagen (el cursor no la mueve).
 4. Redimensionar la ventana: en fit inicial se re-centra; con zoom activo (`user_interacted`) no se fuercea el fit hasta pulsar "Fit".
 5. Rotación (Ctrl+] / Ctrl+[) vuelve a centrar.
 6. No hay `panic` ni lag perceptible.

- [ ] **Step 3: Benchmarks sin regresión** (si existen). Si `criterion` está en la CI, correr:
Run: `cargo bench`
Expected: ≤5% de degradación vs baseline (esta feature no añade trabajo de renderizado; es eliminación de código).

---

## Self-Review

**Spec coverage:**
- §`core/view.rs`: `pan` removido (Task 2), `pan_by` removido (Task 2), `image_origin_screen` sin `+ pan` (Task 2), `fit()` sin reset de pan (Task 2), nuevo `apply_center` (Task 1), `apply_zoom_at` removido (Task 2). ✔
- §`ui/viewer.rs`: `panned` eliminado (Task 3), bloque de arrastre eliminado (Task 3), `use Sensile::hover()` (Task 3), zoom vía `apply_center` (Task 3). ✔
- §`app.rs`: `if resp.zoomed` (Task 4). ✔
- §Tests `core::view`: tests nuevos (Task 1), tests de pan/apply_zoom_at eliminados (Task 2). ✔
- §`tests/integration.rs`: `pan_by` quitado en `flujo_zoom_pan_fit` reescrito (Task 5), aserción de `pan` en rotación quitada (Task 5), zoom usa `apply_center` (Task 5). ✔

**Placeholder scan:** No hay TBD/TODO; todos los pasos de código muestran el código real (los únicos escapes de `cargo check`/`smoke` de la Task 6 son verificaciones, no pasos de código).

**Type consistency:** `apply_center(f32)` definido en Task 1 se usa de forma idéntica en Tasks 2, 3 y 5. `ViewResponse` no tiene `panned` desde Task 3, y `app.rs` (Task 4) ya no lo referencia. `ViewTransform::fit()` (Task 2) se mantiene firma estable usado en Task 5.

**Nota de coherencia:** La Task 2 elimina `apply_center` y sus tests quedan en Task 1; el orden de las tareas (1→2) instala que `apply_center` se añade antes de borrar su gemelo `apply_zoom_at`. Los tests de Task 1 conviven con `apply_zoom_at`/`pan` hasta que Task 2 los borra; el repo pasa `cargo test` en cada paso.