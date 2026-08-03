//! Barra de herramientas compacta superior.
//!
//! Solo presenta: pinta los botones y devuelve la `Action` clickeada. `app.rs`
//! decide qué ejecutar (único dispatch). Recibe los estados visuales
//! (`theme_name`, `is_fullscreen`) para resaltar el botón activo.

use eframe::egui;

use crate::core::actions::Action;
use crate::core::shortcuts::ShortcutMap;

/// Pinta la toolbar y devuelve la acción clickeada, o `None`.
///
/// # Arguments
/// * `ui` - Ui raíz del frame.
/// * `shortcuts` - Mapa de atajos (para mostrar el atajo en el tooltip).
/// * `theme_name` - Tema activo (`"dark"` | `"light"`); solo para tooltip.
/// * `is_fullscreen` - Si la app está en fullscreen (resalta el botón).
pub fn show(
    ui: &mut egui::Ui,
    shortcuts: &ShortcutMap,
    theme_name: &str,
    is_fullscreen: bool,
) -> Option<Action> {
    let mut clicked = None;
    egui::Panel::top("toolbar").exact_size(30.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            if toolbar_button(ui, "<", Action::Prev, shortcuts) {
                clicked = Some(Action::Prev);
            }
            if toolbar_button(ui, ">", Action::Next, shortcuts) {
                clicked = Some(Action::Next);
            }

            ui.separator();

            if toolbar_button(ui, "Rotar", Action::RotateCw, shortcuts) {
                clicked = Some(Action::RotateCw);
            }
            if toolbar_button(ui, "Fit", Action::Fit, shortcuts) {
                clicked = Some(Action::Fit);
            }

            ui.separator();

            if toolbar_button(ui, "Full", Action::Fullscreen, shortcuts) {
                clicked = Some(Action::Fullscreen);
            }
            if toolbar_button(ui, "Tema", Action::ToggleTheme, shortcuts) {
                clicked = Some(Action::ToggleTheme);
            }
            if toolbar_button(ui, "Barra", Action::ToggleSidebar, shortcuts) {
                clicked = Some(Action::ToggleSidebar);
            }
            if toolbar_button(ui, "Info", Action::ToggleInfo, shortcuts) {
                clicked = Some(Action::ToggleInfo);
            }
            if toolbar_button(ui, "Atajos", Action::EditShortcuts, shortcuts) {
                clicked = Some(Action::EditShortcuts);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Tema: {theme_name}"));
                if is_fullscreen {
                    ui.colored_label(egui::Color32::YELLOW, "[*] Fullscreen");
                }
            });
        });
    });
    clicked
}

/// Pinta un botón con `icon`, tooltip con `action.label()` + atajo, y devuelve
/// `true` si se clickeó.
fn toolbar_button(ui: &mut egui::Ui, icon: &str, action: Action, shortcuts: &ShortcutMap) -> bool {
    let shortcut = shortcuts
        .get(action)
        .map(|b| b.to_string())
        .unwrap_or_default();
    let tooltip = if shortcut.is_empty() {
        action.label().to_string()
    } else {
        format!("{} ({shortcut})", action.label())
    };
    ui.button(icon).on_hover_text(tooltip).clicked()
}
