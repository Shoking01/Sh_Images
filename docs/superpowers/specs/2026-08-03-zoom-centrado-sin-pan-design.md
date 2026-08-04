# Imagen fija centrada, zoom sin pan — Design spec

Fecha: 2026-08-03
Estado: Aprobado

## Contexto

El visor actual permite arrastrar la imagen con el mouse (pan) y hacer zoom
anclado al cursor. El usuario quiere:

1. Que la foto **no se arrastre** con el mouse.
2. Que quede **fija y centrada** siempre.
3. Que **el zoom con la rueda siga funcionando**.

Decisión clave: el zoom queda **anclado al centro** del canvas (no al cursor),
de modo que la imagen nunca se desplaza del centro bajo ningún nivel de zoom.

## Objetivo

`ViewTransform` deja de tener desplazamiento (pan). La imagen queda centrada por
construcción; el zoom se aplica alrededor del centro del canvas. Se elimina el
arrastre del mouse.

## Cambios

### `src/core/view.rs` (lógica pura)

- **Eliminar** el campo `pan: Vec2`.
- **Eliminar** el método `pan_by(delta)`.
- `image_origin_screen()`: `center - half` (sin `+ pan`).
- `fit()`: `zoom = fit_zoom()` (ya no hay pan que resetear).
- **Nuevo** `apply_center(factor)`: igual que el antiguo `apply_zoom_at` pero
  anclado al centro del canvas (`anchor = viewport / 2`). Mantiene el punto
  central de la imagen bajo el centro del canvas. Reutiliza el clamp
  `[fit, fit * MAX_ZOOM]`.
- **Eliminar** `apply_zoom_at(anchor, factor)` (anclado al cursor), o convertirlo
  en helper interno que `apply_center` usa con `anchor = viewport / 2`.

### `src/ui/viewer.rs` — interacción

- `ViewResponse`: solo queda `zoomed: bool` (eliminar `panned`).
- `show()`: eliminar el bloque de arrastre (`if response.dragged() { pan_by }`).
- Zoom: usar `apply_center(factor)` en vez de `apply_zoom_at(anchor, factor)`.
- `Sense::click_and_drag()` → `Sense::hover()` (no se arrastra).

### `src/app.rs`

- `if resp.zoomed || resp.panned` → `if resp.zoomed`.
- El autofit no cambia de lógica: al cargar / redimensionar sin interacción,
  `fit()` centra y ajusta a ventana. `user_interacted` solo se marca con zoom.

### Tests

**`src/core/view.rs` (TDD):**
- Eliminar tests que dependen de `pan` / `apply_zoom_at`:
  `pan_by_moves_image_by_delta`, `apply_zoom_at_keeps_anchor_point_fixed`,
  `apply_zoom_at_clamps_to_max_zoom`, `apply_zoom_at_clamps_to_min_fit`,
  y las partes de `fit_resets_to_centered_initial` / `rotating_resets_to_fit_and_centers`
  que usan `pan`/`apply_zoom_at`.
- Añadir:
  - `apply_center_keeps_image_centered`
  - `apply_center_clamps_to_max_zoom`
  - `apply_center_clamps_to_min_fit`
  - `image_origin_is_always_centered` (la esquina TL está centrada en todo zoom).

**`tests/integration.rs`:**
- Quitar `t.pan_by(...)` (línea 122).
- Quitar `assert_eq!(t.pan, ZERO, ...)` (línea 229).
- El flujo de zoom (`apply_zoom_at`, línea 111) pasa a `apply_center`.

**`src/ui/viewer.rs`:** sin tests de UI unitarios; verificación por
`cargo check` / `cargo clippy` y smoke manual.

## Verificación

- `cargo check --locked`
- `cargo clippy --all-targets -- -D warnings`
- `cargo fmt --check`
- `cargo test` y `cargo test --release` (137 + 6 de base, ajustados).
- Smoke manual: imagen centrada siempre, rueda hace zoom, sin arrastre.

## Alcance

- Sin API pública nueva (cambios internos a `core::view`, `ui::viewer`, `app`).
- No toca rotación / fullscreen / shortcuts / toolbar.
- No añade dependencias.