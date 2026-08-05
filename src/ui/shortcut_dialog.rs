use eframe::egui;

use crate::core::actions::Action;
use crate::core::lang::Language;
use crate::core::shortcuts::{KeyBinding, KeyCode, Modifiers, ShortcutError, ShortcutMap};
#[derive(Debug, Default)]
pub struct ShortcutDialog {
    pub open: bool,
    capture_for: Option<Action>,
    error: Option<String>,
}

impl ShortcutDialog {
    pub fn show(&mut self, ui: &mut egui::Ui, shortcuts: &mut ShortcutMap, lang: Language) -> bool {
        let mut changed = false;
        egui::Window::new("⌨")
            .open(&mut self.open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                let t = lang.translations();
                if let Some(action) = self.capture_for {
                    ui.label(format!(
                        "{} «{}» (Esc {})",
                        t.capture_prefix,
                        action.label(lang),
                        t.capture_suffix
                    ));
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.capture_for = None;
                        self.error = None;
                    } else if let Some(binding) = pressed_binding(ui) {
                        match shortcuts.assign(action, binding) {
                            Ok(()) => {
                                changed = true;
                                self.capture_for = None;
                                self.error = None;
                            }
                            Err(ShortcutError::Conflict(other)) => {
                                self.error = Some(format!(
                                    "{} «{}» {}",
                                    t.conflict_prefix,
                                    other.label(lang),
                                    t.conflict_suffix
                                ));
                                self.capture_for = None;
                            }
                            Err(ShortcutError::InvalidKey(_)) | Err(ShortcutError::Empty) => {
                                self.error = Some(t.invalid_combo.to_string());
                            }
                        }
                    }
                } else {
                    egui::Grid::new("shortcuts_grid")
                        .striped(true)
                        .num_columns(3)
                        .show(ui, |ui| {
                            for action in Action::all() {
                                let current = shortcuts
                                    .get(action)
                                    .map(|b| b.to_string())
                                    .unwrap_or_else(|| "—".to_string());
                                ui.label(action.label(lang));
                                ui.label(current);
                                if ui.button("Cambiar…").clicked() {
                                    self.capture_for = Some(action);
                                    self.error = None;
                                }
                                ui.end_row();
                            }
                        });
                    if ui.button(t.reset_all).clicked() {
                        shortcuts.reset();
                        changed = true;
                    }
                }
                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, err);
                }
            });
        changed
    }
}
fn pressed_binding(ui: &egui::Ui) -> Option<KeyBinding> {
    ui.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => keybinding_from_egui(*key, *modifiers),
            _ => None,
        })
    })
}
pub fn keybinding_from_egui(key: egui::Key, modifiers: egui::Modifiers) -> Option<KeyBinding> {
    let code = match key {
        egui::Key::ArrowLeft => KeyCode::ArrowLeft,
        egui::Key::ArrowRight => KeyCode::ArrowRight,
        egui::Key::F => KeyCode::KeyF,
        egui::Key::H => KeyCode::KeyH,
        egui::Key::I => KeyCode::KeyI,
        egui::Key::K => KeyCode::KeyK,
        egui::Key::O => KeyCode::KeyO,
        egui::Key::T => KeyCode::KeyT,
        egui::Key::OpenBracket => KeyCode::OpenBracket,
        egui::Key::CloseBracket => KeyCode::CloseBracket,
        egui::Key::F11 => KeyCode::F11,
        egui::Key::F5 => KeyCode::KeyF5,
        egui::Key::Comma => KeyCode::Comma,
        egui::Key::Period => KeyCode::Period,
        _ => return None,
    };
    let mods = if modifiers.ctrl {
        if modifiers.shift {
            Modifiers::CtrlShift
        } else {
            Modifiers::Ctrl
        }
    } else {
        Modifiers::None
    };
    Some(KeyBinding::new(code, mods))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybinding_from_egui_maps_plain_keys() {
        assert_eq!(
            keybinding_from_egui(egui::Key::ArrowRight, egui::Modifiers::NONE),
            Some(KeyBinding::new(KeyCode::ArrowRight, Modifiers::None))
        );
        assert_eq!(
            keybinding_from_egui(egui::Key::F11, egui::Modifiers::NONE),
            Some(KeyBinding::new(KeyCode::F11, Modifiers::None))
        );
    }

    #[test]
    fn keybinding_from_egui_maps_ctrl_and_ctrl_shift() {
        let ctrl = egui::Modifiers {
            ctrl: true,
            ..egui::Modifiers::NONE
        };
        assert_eq!(
            keybinding_from_egui(egui::Key::O, ctrl),
            Some(KeyBinding::new(KeyCode::KeyO, Modifiers::Ctrl))
        );
        let ctrl_shift = egui::Modifiers {
            ctrl: true,
            shift: true,
            ..egui::Modifiers::NONE
        };
        assert_eq!(
            keybinding_from_egui(egui::Key::T, ctrl_shift),
            Some(KeyBinding::new(KeyCode::KeyT, Modifiers::CtrlShift))
        );
    }

    #[test]
    fn keybinding_from_egui_ignores_unmapped_keys() {
        assert_eq!(
            keybinding_from_egui(egui::Key::A, egui::Modifiers::NONE),
            None
        );
        assert_eq!(
            keybinding_from_egui(egui::Key::Escape, egui::Modifiers::NONE),
            None
        );
    }
}
