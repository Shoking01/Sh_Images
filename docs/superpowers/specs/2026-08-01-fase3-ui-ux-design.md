# Fase 3 — UI/UX Polish — Design

> Fecha: 2026-08-01 · Estado: Aprobado por usuario

## 1. Contexto

Sh_Images tiene completas las Fases 0–2 (fundamentos, visor básico, cache y rendimiento). El visor abre imágenes, navega con ←→, hace zoom/pan/fit, tiene sidebar de miniaturas, pre-carga N±1, benchmarks y tests de integración.

La Fase 3 (Plan.md §4) es el pulido de UI/UX. Estado actual:

- `theme.rs` ya tiene `apply()` para dark/light, y `settings.theme` existe (`"dark"`|`"light"`), pero **no hay toggle en la UI**.
- `toolbar.rs` es un stub vacío de 1 línea. Hoy la barra es solo un menú "Archivo" en el `CentralPanel`.
- No hay fullscreen, rotación, info de imagen persistente, ni atajos configurables.
- Los atajos viven hardcodeados en `app.rs::handle_shortcuts`.
- `insta` no está en dev-deps; los tests son unitarios + integración, sin snapshots.

Esta spec define los 7 ítems de la Fase 3: toolbar con iconos, fullscreen, rotación, info de imagen, tema, atajos configurables y snapshot testing con `insta`.

## 2. Decisiones de diseño

### 2.1 Todas las acciones pasan por un `Action` enum centralizado

**Contexto:** toolbar, menú y atajos configurables disparan las mismas acciones. Si cada vía implementa su propia llamada, toolbar y atajos se desincronizan y el editor de atajos no tendría nada que remapear.

**Decisión (enfoque A, elegido en brainstorming):** un enum `Action` en `core/actions.rs` enumera todas las acciones. `app.rs` tiene un único `dispatch(&mut self, action: Action)`. La toolbar devuelve `Option<Action>` al click, los atajos se resuelven a `Action` vía `ShortcutMap`, y el menú también. Un solo punto de ejecución.

**Consecuencias:**
- Los botones y los atajos nunca divergen.
- `Action::label()` y `Action::default_shortcut()` centralizan los metadatos.
- El dispatch es testeable como lógica pura (las acciones cuyo efecto es puro se testean directamente).

### 2.2 Atajos configurables con editor visual en la UI

**Decisión (nivel "completo", elegido en brainstorming):** un `ShortcutMap` (mapeo `Action → KeyBinding`) serializable en `settings.toml`, más un editor modal (`ui/shortcut_dialog.rs`) que captura teclas en vivo.

- `KeyBinding { key: KeyCode, modifiers: Modifiers }` con enums propios de core (sin egui) para mantener `core/` libre de UI (AGENTS.md §3.2).
- `defaults()` devuelve el mapa por defecto; migración transparente vía `#[serde(default)]` (los archivos de settings existentes siguen cargando).
- Validación de conflictos: dos acciones no pueden compartir binding; se reporta error tipado y no se asigna.

### 2.3 Rotación como transformación visual, no de bitmap

**Decisión (elegido en brainstorming):** la rotación es un campo `rotation: u8` (0/1/2/3 = 0°/90°CW/180°/270°CW) en `ViewTransform`. El viewer pinta el mesh con los vértices y UVs permutados. No se re-decodifica ni se toca el cache.

**Consecuencias:**
- Operación instantánea (no hay costo de re-muestreo).
- `fit_zoom()` y `image_origin_screen()` deben usar las dimensiones *efectivas* (rotadas): para rotación impar las dimensiones se intercambian.
- Rotar resetea el pan y re-aplica fit (comportamiento estándar en visores).
- La rotación no persiste en disco (no es un setting).

### 2.4 Fullscreen nativo

**Decisión:** `ViewportCommand::Fullscreen(bool)` vía `ctx.send_viewport_cmd`. F11 (por defecto) lo alterna; Esc lo sale (comportamiento del viewport de eframe). `is_fullscreen: bool` en `ShImagesApp` para pintar el estado en la toolbar.

### 2.5 Tema con toggle en toolbar + persistencia

**Decisión:** `Action::ToggleTheme` alterna `settings.theme` entre `"dark"`/`"light"` y persiste con `settings.save()`. `theme.rs::apply` ya existe y no cambia. Se añade un helper puro `theme::toggle(name) -> &'static str` testeable.

### 2.6 Info de imagen en status bar inferior

**Decisión:** `ui/statusbar.rs` pinta un `egui::Panel::bottom` con `nombre · dims · tamaño · índice`. La UI recibe un struct `StatusInfo` ya construido por `app.rs`; el único formateo con lógica (`format_status`) es puro y testeable con insta.

- Dimensiones: de `ViewTransform.image_size` (ya disponible, sin I/O).
- Tamaño en disco: `fs::metadata(path).len()`, cacheado en `Option<u64>` refrescado al cambiar de imagen.
- Índice: `nav.current + 1` / `nav.images.len()`.

### 2.7 Snapshot testing con insta sobre estado/lógica UI

**Decisión (elegido en brainstorming):** egui es immediate-mode y el render es no determinista; no se hace snapshot del árbol renderizado (eso exigiría `egui_kittest`, dependencia pesada). En su lugar, `insta` congela estructuras deterministas:

- `core/shortcuts.rs::defaults()` serializado.
- `KeyBinding::to_string()` de todos los defaults (representación mostrada en UI).
- `ui/statusbar.rs::format_status` con tabla de casos.
- `core/view.rs` rotación: secuencia de rotaciones y sus `fit_zoom`/`image_origin_screen` resultantes.

Snapshots se revisan en el primer run y se commitean (AGENTS.md §4.4).

### 2.8 Sin dependencias nuevas salvo `insta`

Todo lo demás usa `std` + `egui`/`eframe` + `image` existentes. `insta` se añade a `dev-dependencies` (justificación en Cargo.toml: AGENTS.md §7.2, requerido por Plan.md Fase 3 y AGENTS.md §4.4).

## 3. Componentes

### 3.1 `core/actions.rs` (nuevo)

Enum puro, sin egui:

```rust
pub enum Action {
    Open, Prev, Next, RotateCW, RotateCCW, Fit, Fullscreen,
    ToggleTheme, ToggleSidebar, EditShortcuts,
}
```

- `label(&self) -> &'static str` — texto humano para la UI.
- `default_shortcut(&self) -> Option<KeyBinding>` — atajo por defecto de la acción (todas las del mapa tienen uno; el `Option` es solo para extensión futura).
- `variants() -> &'static [Action]` — para iterar en el editor de atajos.
- Serialización a string (`serde rename`: `next`, `rotate_cw`, …) para `settings.toml`.
- `ShImagesError` no cambia; no hay errores nuevos en este módulo.

### 3.2 `core/shortcuts.rs` (nuevo)

- `enum KeyCode { ArrowLeft, ArrowRight, KeyF, KeyO, KeyH, KeyK, KeyT, BracketLeft, BracketRight, F11 }` — subconjunto mínimo serializable (Rust no puede derivar serde sobre `egui::Key` fácilmente en core sin egui; enums propios).
- `enum Modifiers { NONE, CTRL, CTRL_SHIFT }` (extensible).
- `struct KeyBinding { key: KeyCode, modifiers: Modifiers }`.
  - `to_string(&self) -> String` → `"Ctrl+O"`, `"→"`, `"F11"` (snapshot-eado).
  - `parse(&str) -> Result<KeyBinding, ShImagesError>` — para el editor (se acepta la representación de `to_string` o un formato teclado simple).
- `struct ShortcutMap` (internamente `BTreeMap<Action, KeyBinding>` para determinismo de serialización).
  - `defaults() -> ShortcutMap`.
  - `get(&Action) -> Option<&KeyBinding>`.
  - `assign(&mut self, action: Action, binding: KeyBinding) -> Result<(), ShortcutError>` — rechaza conflictos (otra acción ya usa el binding).
  - `reset(&mut self)` — vuelve a defaults.
- `enum ShortcutError { Conflict { action: Action } }` — el binding ya está asignado a `action`. Implementa `From<ShortcutError> for ShImagesError` para propagarlo a la UI.
- Defaults:
  | Action | Binding |
  |--------|---------|
  | Open | Ctrl+O |
  | Prev | ← |
  | Next | → |
  | RotateCW | Ctrl+] |
  | RotateCCW | Ctrl+[ |
  | Fit | F |
  | Fullscreen | F11 |
  | ToggleTheme | Ctrl+T |
  | ToggleSidebar | H |
  | EditShortcuts | Ctrl+K |

### 3.3 `config/settings.rs` (modificar)

- Añadir `#[serde(default)] pub shortcuts: ShortcutMap`.
- `Default` incluye `ShortcutMap::defaults()`.
- Sin migración de versión explícita: `#[serde(default)]` cubre archivos viejos. (El campo `theme` ya sigue este patrón.)

### 3.4 `core/view.rs` (modificar)

- Añadir `pub rotation: u8` a `ViewTransform` (0/1/2/3).
- `rotate_cw()` / `rotate_ccw()` — suma/resta módulo 4 y re-aplica fit con pan 0.
- Helpers privados `effective_size()` (intercambia dimensiones si rotación impar) y uso en `fit_zoom()` e `image_origin_screen()`.
- `new()` inicializa `rotation: 0`. `apply_zoom_at`/`pan_by` no cambian (operan en pantalla).

### 3.5 `ui/viewer.rs` (modificar)

- Pintar la textura con el mesh rotado: construir `egui::Mesh` con 4 vértices en las posiciones del rect rotado y los UVs permutados según `transform.rotation`.
- Para `rotation == 0` se mantiene el camino actual (`painter.image`) para no regresar en rendimiento; para rotado se usa el mesh.

### 3.6 `ui/toolbar.rs` (implementar stub)

- `pub fn show(ui: &mut egui::Ui, shortcuts: &ShortcutMap, theme_name: &str, is_fullscreen: bool) -> Option<Action>` — pinta la toolbar compacta bajo el menú: Prev, Next | RotateCW, Fit | Fullscreen, ToggleTheme, ToggleSidebar, EditShortcuts. Devuelve la acción clickeada. `theme_name` y `is_fullscreen` solo determinan el estado visual (icono activo).
- Los iconos son caracteres Unicode (egui no trae icon font en este setup): `← → ↻ ⤢ ⛶ ◐ ☰`. Tooltip con `label()` + atajo actual (`ui.on_hover_text`).
- Estado visual: botón de tema y de fullscreen reflejan el estado activo.

### 3.7 `ui/statusbar.rs` (nuevo)

- `struct StatusInfo { name: String, width: u32, height: u32, size_bytes: Option<u64>, index: usize, total: usize }`.
- `fn format_status(info: &StatusInfo) -> String` — puro, testeable con insta.
- `pub fn show(ui: &mut egui::Ui, info: &StatusInfo)` — pinta el panel inferior.

### 3.8 `ui/shortcut_dialog.rs` (nuevo)

- `struct ShortcutDialog { open: bool, capture_for: Option<Action>, error: Option<String> }`.
- `show(&mut self, ui, shortcuts: &mut ShortcutMap) -> bool` — devuelve `true` si el mapa cambió (para persistir). Modal `egui::Window`.
- Fila por acción: label, binding actual, botón "Cambiar…" (entra en modo captura), botón "Reset".
- Captura: en modo captura, el siguiente `key_pressed` + modificadores se asignan a la acción; si entra en conflicto, se muestra `error` y no se asigna.
- Botón "Restablecer todos" → `shortcuts.reset()`.

### 3.9 `app.rs` (modificar)

- Añadir campos: `shortcuts: ShortcutMap`, `is_fullscreen: bool`, `status_info_cache: Option<StatusInfo>` (o construir on-the-fly), `shortcut_dialog: ShortcutDialog`.
- `dispatch(&mut self, action: Action)` — único punto de ejecución:
  - `Open` → `open_dialog()`
  - `Prev`/`Next` → `navigate(-1/1)`
  - `RotateCW`/`RotateCCW` → `transform.rotate_cw()/rotate_ccw()` + `user_interacted=false`
  - `Fit` → `transform.fit()`
  - `Fullscreen` → toggle viewport + `is_fullscreen`
  - `ToggleTheme` → `theme::toggle` + `settings.save()`
  - `ToggleSidebar` → `toggle_sidebar()`
  - `EditShortcuts` → abrir el dialog
- `handle_shortcuts` se reescribe: lee `input` una vez, construye el `KeyBinding` de la tecla pulsada y lo busca en `shortcuts` (en lugar de `consume_key` por cada acción).
- `ui()`: toolbar en el panel superior, status bar inferior, y el dialog de shortcuts.
- El menú "Archivo" usa `dispatch`.

### 3.10 `ui/theme.rs` (modificar)

- Añadir `pub fn toggle(name: &str) -> &'static str` — `"dark"`→`"light"`, `"light"`→`"dark"`, otro→`"dark"` (con warning).

## 4. Testing

### 4.1 Unitarios (nuevos, estilo descriptivo existente)

- `core/actions.rs`: `label` para cada variante; `default_shortcut` (None y Some); roundtrip de serde de `Action` por string.
- `core/shortcuts.rs`: `defaults` completas (10 acciones, bindings esperados); `get`; `assign` reemplaza sin conflicto; `assign` con conflicto devuelve error y no cambia; `reset` vuelve a defaults; `to_string`/`parse` roundtrip.
- `core/view.rs`: `rotate_cw` 0→1→2→3→0; `rotate_ccw` inverso; `fit_zoom` con rotación impar intercambia dimensiones; tras rotar el pan es 0 y está en fit.
- `ui/statusbar.rs`: `format_status` (bytes 0, KB, MB, GB; índices; nombre largo).
- `ui/theme.rs`: `toggle` dark→light, light→dark, unknown→dark.

### 4.2 Snapshots con insta

- `shortcuts_defaults` — `defaults()` serializado (YAML).
- `keybinding_to_string` — `to_string()` de todos los defaults.
- `statusbar_format` — tabla de casos de `format_status`.
- `rotation_math` — secuencia de rotaciones con `fit_zoom`/`image_origin_screen`.

### 4.3 Integración

- Ampliar `tests/integration.rs` con flujo de rotación puro: abrir → `rotate_cw` → verificar transform (dimensiones efectivas y fit), sin GUI.

### 4.4 Cobertura

- `core/` sigue ≥90% (actions/shortcuts/view suman tests puros).
- `ui/` ≥70% (statusbar format + theme toggle son puros; toolbar/dialog quedan como QA manual en su parte visual).

## 5. Files afectados

```
src/core/actions.rs           # CREAR
src/core/shortcuts.rs         # CREAR
src/core/mod.rs               # MODIFICAR — registrar actions, shortcuts
src/core/view.rs              # MODIFICAR — rotation + effective_size
src/config/settings.rs        # MODIFICAR — shortcuts en Settings
src/ui/toolbar.rs             # IMPLEMENTAR stub
src/ui/statusbar.rs           # CREAR
src/ui/shortcut_dialog.rs     # CREAR
src/ui/mod.rs                 # MODIFICAR — registrar statusbar, shortcut_dialog
src/ui/viewer.rs              # MODIFICAR — mesh rotado
src/ui/theme.rs               # MODIFICAR — toggle()
src/app.rs                    # MODIFICAR — dispatch, fullscreen, status bar, dialog
tests/integration.rs          # MODIFICAR — flujo de rotación
Cargo.toml                    # MODIFICAR — insta en dev-dependencies
```

## 6. Riesgos

| Riesgo | Mitigación |
|--------|-----------|
| Mesh rotado con UVs mal permutados (image upside down) | Tests de rotación de math en core; QA manual visual documentado en el PR |
| Editor de atajos captura teclas reservadas por egui (Esc, Tab) | En modo captura se ignoran modificadores solos y teclas de sistema; QA manual |
| `BTreeMap<Action, _>` necesita `Action: Ord` | Derivar `Ord`/`PartialOrd` con orden de variante explícito |
| El dispatch en `app.rs` toca UI thread (open_dialog bloqueante ya existente) | Sin cambios: es el comportamiento actual aceptado |
