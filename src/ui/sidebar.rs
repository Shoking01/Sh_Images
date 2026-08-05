use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use eframe::egui;
use egui::{Color32, Sense, Stroke, StrokeKind};

use crate::core::navigation::Navigation;
use crate::core::thumbnail_cache::ThumbnailCache;
const CELL_SIZE: f32 = 108.0;
pub struct SidebarState {
    pub show: bool,
    textures: HashMap<PathBuf, egui::TextureHandle>,
}

impl fmt::Debug for SidebarState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SidebarState")
            .field("show", &self.show)
            .field("textures", &self.textures.len())
            .finish()
    }
}

impl Default for SidebarState {
    fn default() -> Self {
        Self::new()
    }
}

impl SidebarState {
    pub fn new() -> Self {
        Self {
            show: true,
            textures: HashMap::new(),
        }
    }
    pub fn clear_textures(&mut self) {
        self.textures.clear();
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        nav: &Navigation,
        thumb_cache: &ThumbnailCache,
    ) -> Option<usize> {
        let mut selection = None;
        egui::Panel::left("sidebar")
            .min_size(CELL_SIZE + 20.0)
            .default_size(CELL_SIZE + 40.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (i, path) in nav.images.iter().enumerate() {
                            if !self.textures.contains_key(path) {
                                if let Some(thumb) = thumb_cache.get(path) {
                                    let tex = load_thumbnail(ui.ctx(), &thumb);
                                    self.textures.insert(path.clone(), tex);
                                }
                            }
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(CELL_SIZE, CELL_SIZE),
                                Sense::click(),
                            );
                            if response.clicked() {
                                selection = Some(i);
                            }
                            if i == nav.current {
                                ui.painter()
                                    .rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
                            }
                            match self.textures.get(path) {
                                Some(tex) => {
                                    let fit = fit_thumbnail(rect.size(), tex.size_vec2());
                                    let top_left = rect.center() - egui::vec2(fit.x, fit.y) * 0.5;
                                    let img_rect = egui::Rect::from_min_size(top_left, fit);
                                    ui.painter().image(
                                        tex.id(),
                                        img_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                }
                                None => {
                                    ui.painter().rect_filled(rect, 4.0, Color32::from_gray(60));
                                }
                            }
                            if response.hovered() {
                                ui.painter().rect_stroke(
                                    rect,
                                    4.0,
                                    Stroke::new(1.0, Color32::from_gray(180)),
                                    StrokeKind::Inside,
                                );
                            }
                        }
                    });
                });
            });
        selection
    }
}
fn load_thumbnail(ctx: &egui::Context, thumb: &image::DynamicImage) -> egui::TextureHandle {
    let size = [thumb.width() as usize, thumb.height() as usize];
    let rgba = thumb.to_rgba8();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    ctx.load_texture("thumbnail", color_image, egui::TextureOptions::LINEAR)
}
fn fit_thumbnail(avail: egui::Vec2, img: egui::Vec2) -> egui::Vec2 {
    if img.x <= 0.0 || img.y <= 0.0 {
        return avail;
    }
    let scale = (avail.x / img.x).min(avail.y / img.y).min(1.0);
    egui::vec2(img.x * scale, img.y * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_vec2(a: egui::Vec2, b: egui::Vec2) {
        assert!((a.x - b.x).abs() < 1e-3, "x: {} != {}", a.x, b.x);
        assert!((a.y - b.y).abs() < 1e-3, "y: {} != {}", a.y, b.y);
    }

    #[test]
    fn fit_square_image_into_square_cell_fills_cell() {
        assert_vec2(
            fit_thumbnail(egui::vec2(108.0, 108.0), egui::vec2(96.0, 96.0)),
            egui::vec2(96.0, 96.0),
        );
    }

    #[test]
    fn fit_wide_image_preserves_aspect_ratio() {
        assert_vec2(
            fit_thumbnail(egui::vec2(108.0, 108.0), egui::vec2(96.0, 54.0)),
            egui::vec2(96.0, 54.0),
        );
    }

    #[test]
    fn fit_tall_image_preserves_aspect_ratio() {
        assert_vec2(
            fit_thumbnail(egui::vec2(108.0, 108.0), egui::vec2(54.0, 96.0)),
            egui::vec2(54.0, 96.0),
        );
    }

    #[test]
    fn fit_never_upscales_beyond_natural_size() {
        assert_vec2(
            fit_thumbnail(egui::vec2(200.0, 200.0), egui::vec2(50.0, 50.0)),
            egui::vec2(50.0, 50.0),
        );
    }

    #[test]
    fn fit_scales_down_when_avail_is_smaller() {
        assert_vec2(
            fit_thumbnail(egui::vec2(40.0, 20.0), egui::vec2(96.0, 54.0)),
            egui::vec2(35.55556, 20.0),
        );
    }

    #[test]
    fn fit_zero_size_returns_avail() {
        assert_vec2(
            fit_thumbnail(egui::vec2(108.0, 108.0), egui::vec2(0.0, 0.0)),
            egui::vec2(108.0, 108.0),
        );
    }
}
