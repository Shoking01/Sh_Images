use eframe::egui;
use egui::{Sense, Vec2};

use crate::core::actions::Action;
use crate::core::lang::Language;
use crate::core::shortcuts::ShortcutMap;
use crate::ui::toolbar_icons::ToolbarIcon;

const ICON_SIZE: f32 = 22.0;

pub fn show(
    ui: &mut egui::Ui,
    shortcuts: &ShortcutMap,
    _theme_name: &str,
    is_fullscreen: bool,
    slideshow_active: bool,
    lang: crate::core::lang::Language,
) -> Option<Action> {
    let mut clicked = None;
    let t = lang.translations();
    egui::Panel::top("toolbar").exact_size(30.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            if toolbar_button(ui, ToolbarIcon::Prev, Action::Prev, shortcuts, lang) {
                clicked = Some(Action::Prev);
            }
            if toolbar_button(ui, ToolbarIcon::Next, Action::Next, shortcuts, lang) {
                clicked = Some(Action::Next);
            }

            ui.separator();

            if toolbar_button(ui, ToolbarIcon::RotateCw, Action::RotateCw, shortcuts, lang) {
                clicked = Some(Action::RotateCw);
            }
            if toolbar_button(ui, ToolbarIcon::Fit, Action::Fit, shortcuts, lang) {
                clicked = Some(Action::Fit);
            }

            ui.separator();

            if toolbar_button(
                ui,
                ToolbarIcon::Fullscreen,
                Action::Fullscreen,
                shortcuts,
                lang,
            ) {
                clicked = Some(Action::Fullscreen);
            }
            if toolbar_button(
                ui,
                ToolbarIcon::Sidebar,
                Action::ToggleSidebar,
                shortcuts,
                lang,
            ) {
                clicked = Some(Action::ToggleSidebar);
            }
            if toolbar_button(ui, ToolbarIcon::Info, Action::ToggleInfo, shortcuts, lang) {
                clicked = Some(Action::ToggleInfo);
            }
            if toolbar_button(
                ui,
                ToolbarIcon::Slideshow,
                Action::ToggleSlideshow,
                shortcuts,
                lang,
            ) {
                clicked = Some(Action::ToggleSlideshow);
            }
            if toolbar_button(ui, ToolbarIcon::Edit, Action::Edit, shortcuts, lang) {
                clicked = Some(Action::Edit);
            }
            ui.menu_button("⚙", |ui| {
                ui.menu_button(t.language, |ui| {
                    if ui.button(Language::Es.native_name()).clicked() {
                        ui.close();
                        clicked = Some(Action::SetLangEs);
                    }
                    if ui.button(Language::En.native_name()).clicked() {
                        ui.close();
                        clicked = Some(Action::SetLangEn);
                    }
                });
                ui.separator();
                if ui.button(Action::ToggleTheme.label(lang)).clicked() {
                    ui.close();
                    clicked = Some(Action::ToggleTheme);
                }
                if ui.button(Action::EditShortcuts.label(lang)).clicked() {
                    ui.close();
                    clicked = Some(Action::EditShortcuts);
                }
                if ui.button(Action::SetDefaultViewer.label(lang)).clicked() {
                    ui.close();
                    clicked = Some(Action::SetDefaultViewer);
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if is_fullscreen {
                    ui.colored_label(egui::Color32::YELLOW, "[*] Fullscreen");
                }
                if slideshow_active {
                    ui.colored_label(egui::Color32::GREEN, "[▶] Slideshow");
                }
            });
        });
    });
    clicked
}

fn toolbar_button(
    ui: &mut egui::Ui,
    icon: ToolbarIcon,
    action: Action,
    shortcuts: &ShortcutMap,
    lang: Language,
) -> bool {
    let shortcut = shortcuts
        .get(action)
        .map(|b| b.to_string())
        .unwrap_or_default();
    let tooltip = if shortcut.is_empty() {
        action.label(lang).to_string()
    } else {
        format!("{} ({shortcut})", action.label(lang))
    };
    let size = Vec2::splat(ICON_SIZE);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, visuals.corner_radius, visuals.bg_fill);
    }
    let icon_rect = rect.shrink(4.0);
    icon.paint(painter, icon_rect, visuals.text_color());
    response.clone().on_hover_text(tooltip);
    response.clicked()
}
