use eframe::egui;

use crate::core::edit_state::{EditState, Filter};
use crate::core::lang::Language;
const EDITOR_WIDTH: f32 = 220.0;
#[derive(Debug, Default)]
pub struct EditorResponse {
    pub start_crop: bool,
    pub apply_crop: bool,
    pub cancel_crop: bool,
    pub save: bool,
    pub save_as: bool,
    pub cancel: bool,
    pub reset: bool,
}
pub fn show(ctx: &egui::Context, state: &mut EditState) -> EditorResponse {
    let mut response = EditorResponse::default();

    let screen = ctx
        .input(|i| i.raw.screen_rect)
        .unwrap_or(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1280.0, 800.0),
        ));
    let pos = egui::pos2(screen.max.x - EDITOR_WIDTH - 4.0, screen.min.y + 4.0);
    let lang = ctx
        .data(|d| d.get_temp::<Language>("app_lang".into()))
        .unwrap_or_default();
    let t = lang.translations();

    egui::Window::new(t.editor_title)
        .fixed_pos(pos)
        .fixed_size(egui::vec2(EDITOR_WIDTH, screen.height() - 8.0))
        .title_bar(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_height(ui.available_height());
            ui.heading(t.editor_title);
            ui.separator();
            ui.label(t.crop);
            if state.is_crop_mode() {
                ui.label(t.crop_active_hint);
                ui.horizontal(|ui| {
                    if ui.button(t.apply).clicked() {
                        response.apply_crop = true;
                    }
                    if ui.button(t.cancel).clicked() {
                        response.cancel_crop = true;
                    }
                });
                if let Some(rect) = &state.crop_rect {
                    ui.label(format!(
                        "  {}: {}×{} @ ({}, {})",
                        t.crop_selection, rect.w, rect.h, rect.x, rect.y
                    ));
                }
            } else if state.has_crop() {
                let rect = state.crop_rect.unwrap();
                ui.label(format!(
                    "{}: {}×{} @ ({}, {})",
                    t.crop_dimensions, rect.w, rect.h, rect.x, rect.y
                ));
                ui.horizontal(|ui| {
                    if ui.button(t.apply).clicked() {
                        response.apply_crop = true;
                    }
                    if ui.button("✕").clicked() {
                        response.cancel_crop = true;
                    }
                });
            } else if ui.button(t.select_area).clicked() {
                response.start_crop = true;
            }
            ui.separator();
            ui.label(t.filters);
            ui.horizontal_wrapped(|ui| {
                for filter in [
                    Filter::Grayscale,
                    Filter::Sepia,
                    Filter::Invert,
                    Filter::BlackWhite,
                ] {
                    let selected = state.filter == Some(filter);
                    let text = filter.label(lang);
                    if selected {
                        if ui.selectable_label(true, text).clicked() {
                            state.filter = None;
                        }
                    } else if ui.selectable_label(false, text).clicked() {
                        state.filter = Some(filter);
                    }
                }
            });
            ui.separator();
            ui.label(t.adjustments);

            ui.horizontal(|ui| {
                ui.label(t.brightness);
                ui.add(egui::Slider::new(&mut state.brightness, -100..=100));
            });
            ui.horizontal(|ui| {
                ui.label(t.contrast);
                ui.add(egui::Slider::new(&mut state.contrast, -100..=100));
            });
            ui.horizontal(|ui| {
                ui.label(t.saturation);
                ui.add(egui::Slider::new(&mut state.saturation, -100..=100));
            });

            ui.separator();
            if ui.button(t.save_copy_btn).clicked() {
                response.save = true;
            }
            if ui.button(t.save_as_btn).clicked() {
                response.save_as = true;
            }
            if ui.button(t.reset_btn).clicked() {
                response.reset = true;
            }
            if ui.button(t.cancel_edit_btn).clicked() {
                response.cancel = true;
            }
        });

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_response_has_no_actions() {
        let resp = EditorResponse::default();
        assert!(!resp.apply_crop);
        assert!(!resp.save);
        assert!(!resp.cancel);
        assert!(!resp.start_crop);
    }
}
