use std::ops::Mul;

use eframe::egui;

use crate::core::view::{Vec2, ViewTransform};
#[derive(Debug, Default)]
pub struct ViewResponse {
    pub zoomed: bool,
    pub panned: bool,
}
pub fn show(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    transform: &mut ViewTransform,
) -> ViewResponse {
    show_inner(ui, texture, transform, None)
}
pub fn show_editable(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    transform: &mut ViewTransform,
    crop_mode: bool,
    crop_rect_screen: Option<egui::Rect>,
) -> ViewResponse {
    show_inner(
        ui,
        texture,
        transform,
        if crop_mode {
            Some(crop_rect_screen)
        } else {
            None
        },
    )
}

fn show_inner(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    transform: &mut ViewTransform,
    crop_overlay: Option<Option<egui::Rect>>,
) -> ViewResponse {
    let size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

    transform.set_viewport(Vec2::new(rect.width(), rect.height()));
    ui.painter()
        .rect_filled(rect, 0.0, egui::Color32::from_gray(24));
    let origin = transform.image_origin_screen();
    let effective = transform.effective_size();
    let w = effective.x * transform.zoom;
    let h = effective.y * transform.zoom;
    let image_rect = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + origin.x, rect.min.y + origin.y),
        egui::vec2(w, h),
    );
    let clip = rect;

    if transform.rotation == 0 {
        ui.painter().with_clip_rect(clip).image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        let corners = [
            image_rect.left_top(),
            image_rect.right_top(),
            image_rect.right_bottom(),
            image_rect.left_bottom(),
        ];
        let mut mesh = egui::Mesh::with_texture(texture.id());
        for (i, pos) in corners.iter().enumerate() {
            let (u, v) = ViewTransform::rotated_uv(i as u8, transform.rotation);
            mesh.vertices.push(egui::epaint::Vertex {
                pos: *pos,
                uv: egui::pos2(u, v),
                color: egui::Color32::WHITE,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        ui.painter()
            .with_clip_rect(clip)
            .add(egui::Shape::mesh(mesh));
    }
    if let Some(Some(sel_rect)) = crop_overlay {
        draw_crop_overlay(ui, image_rect, sel_rect);
    }

    let mut result = ViewResponse::default();
    let crop_mode = crop_overlay.is_some();

    if crop_mode {
        handle_crop_drag(ui, &response, rect, transform, &mut result);
    } else if response.dragged() {
        let delta = response.drag_delta();
        if delta.x != 0.0 || delta.y != 0.0 {
            transform.pan_by(Vec2::new(delta.x, delta.y));
            result.panned = true;
            ui.ctx().request_repaint();
        }
    }
    if !crop_mode && response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = (scroll * 0.001).exp();
            let cursor_local = response
                .hover_pos()
                .map(|p| Vec2::new(p.x - rect.min.x, p.y - rect.min.y))
                .unwrap_or_else(|| transform.viewport.mul(0.5));
            transform.apply_zoom_at(cursor_local, factor);
            result.zoomed = true;
            ui.ctx().request_repaint();
        }
    }
    if response.hovered() {
        if crop_mode {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        } else {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
    if response.is_pointer_button_down_on() && !crop_mode {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }

    result
}
fn handle_crop_drag(
    ui: &egui::Ui,
    response: &egui::Response,
    canvas_rect: egui::Rect,
    transform: &ViewTransform,
    result: &mut ViewResponse,
) {
    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            if canvas_rect.contains(pos) {
                ui.ctx().data_mut(|d| {
                    d.insert_temp("crop_start".into(), pos);
                });
            }
        }
    }

    if response.dragged() {
        let start = ui
            .ctx()
            .data(|d| d.get_temp::<egui::Pos2>("crop_start".into()));
        let current = response.interact_pointer_pos();
        if let (Some(start), Some(current)) = (start, current) {
            let sel_rect = egui::Rect::from_two_pos(start, current).intersect(canvas_rect);
            ui.ctx().data_mut(|d| {
                d.insert_temp("crop_selection".into(), sel_rect);
            });
            result.panned = true; // Reutilizamos para indicar actividad.
            ui.ctx().request_repaint();
        }
    }

    if response.drag_stopped() {
        let sel = ui
            .ctx()
            .data(|d| d.get_temp::<egui::Rect>("crop_selection".into()));
        if let Some(sel_rect) = sel {
            let origin = transform.image_origin_screen();
            let zoom = transform.zoom;
            let img_x = ((sel_rect.min.x - canvas_rect.min.x - origin.x) / zoom) as i32;
            let img_y = ((sel_rect.min.y - canvas_rect.min.y - origin.y) / zoom) as i32;
            let img_x2 = ((sel_rect.max.x - canvas_rect.min.x - origin.x) / zoom) as i32;
            let img_y2 = ((sel_rect.max.y - canvas_rect.min.y - origin.y) / zoom) as i32;
            let img_w = (img_x2 - img_x).max(1) as u32;
            let img_h = (img_y2 - img_y).max(1) as u32;
            let img_x = img_x.max(0) as u32;
            let img_y = img_y.max(0) as u32;
            ui.ctx().data_mut(|d| {
                d.insert_temp("crop_result".into(), (img_x, img_y, img_w, img_h));
            });
        }
        ui.ctx().data_mut(|d| {
            d.remove::<egui::Pos2>("crop_start".into());
        });
    }
}
pub fn get_crop_result(ctx: &egui::Context) -> Option<(u32, u32, u32, u32)> {
    ctx.data(|d| d.get_temp::<(u32, u32, u32, u32)>("crop_result".into()))
}
pub fn get_crop_selection(ctx: &egui::Context) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp::<egui::Rect>("crop_selection".into()))
}
fn draw_crop_overlay(ui: &egui::Ui, image_rect: egui::Rect, sel_rect: egui::Rect) {
    let painter = ui.painter();
    let overlay_color = egui::Color32::from_rgba_premultiplied(0, 0, 0, 128);
    let top =
        egui::Rect::from_min_max(image_rect.min, egui::pos2(image_rect.max.x, sel_rect.min.y));
    painter.rect_filled(top, 0.0, overlay_color);
    let bottom =
        egui::Rect::from_min_max(egui::pos2(image_rect.min.x, sel_rect.max.y), image_rect.max);
    painter.rect_filled(bottom, 0.0, overlay_color);
    let left = egui::Rect::from_min_max(
        egui::pos2(image_rect.min.x, sel_rect.min.y),
        egui::pos2(sel_rect.min.x, sel_rect.max.y),
    );
    painter.rect_filled(left, 0.0, overlay_color);
    let right = egui::Rect::from_min_max(
        egui::pos2(sel_rect.max.x, sel_rect.min.y),
        egui::pos2(image_rect.max.x, sel_rect.max.y),
    );
    painter.rect_filled(right, 0.0, overlay_color);
    painter.rect_stroke(
        sel_rect,
        0.0,
        egui::Stroke::new(2.0, egui::Color32::WHITE),
        egui::StrokeKind::Outside,
    );
    let handle_size = 4.0;
    for corner in [
        sel_rect.left_top(),
        sel_rect.right_top(),
        sel_rect.left_bottom(),
        sel_rect.right_bottom(),
    ] {
        painter.circle_filled(corner, handle_size, egui::Color32::WHITE);
    }
}
