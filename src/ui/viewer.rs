//! Componente que pinta la imagen con la transformación de vista.
//!
//! Solo presenta: dibuja la textura con `ViewTransform` y reporta los inputs.
//! Toda la lógica de transformación vive en `core::view`.

use eframe::egui;

use crate::core::view::{Vec2, ViewTransform};

/// Resultado de interacción del visor en un frame.
#[derive(Debug, Default)]
pub struct ViewResponse {
    /// El usuario hizo zoom con la rueda.
    pub zoomed: bool,
    /// El usuario arrastró (pan).
    pub panned: bool,
}

/// Pinta la textura en todo el espacio disponible y captura zoom/pan.
///
/// # Arguments
/// * `ui` - UI de egui donde se dibuja el canvas.
/// * `texture` - Textura de la imagen a mostrar.
/// * `transform` - Transformación de vista (se muta con zoom/pan).
pub fn show(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    transform: &mut ViewTransform,
) -> ViewResponse {
    let size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

    transform.set_viewport(Vec2::new(rect.width(), rect.height()));

    // Fondo oscuro del canvas.
    ui.painter()
        .rect_filled(rect, 0.0, egui::Color32::from_gray(24));

    // Rectángulo de la imagen en pantalla (origin es relativo al canvas).
    let origin = transform.image_origin_screen();
    let effective = transform.effective_size();
    let w = effective.x * transform.zoom;
    let h = effective.y * transform.zoom;
    let image_rect = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + origin.x, rect.min.y + origin.y),
        egui::vec2(w, h),
    );

    if transform.rotation == 0 {
        // Camino sin rotación: un solo `painter.image` (más barato que un mesh).
        ui.painter().image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        // Mesh rotado: 4 vértices en las esquinas del rect con UVs permutados.
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
        ui.painter().add(egui::Shape::mesh(mesh));
    }

    let mut result = ViewResponse::default();

    // Zoom con la rueda, anclado al cursor.
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let anchor_screen = response.hover_pos().unwrap_or_else(|| rect.center());
            let anchor = Vec2::new(anchor_screen.x - rect.min.x, anchor_screen.y - rect.min.y);
            let factor = (scroll * 0.001).exp();
            transform.apply_zoom_at(anchor, factor);
            result.zoomed = true;
            ui.ctx().request_repaint();
        }
    }

    // Pan con arrastre.
    if response.dragged() {
        let delta = response.drag_delta();
        transform.pan_by(Vec2::new(delta.x, delta.y));
        result.panned = true;
        ui.ctx().request_repaint();
    }

    result
}
