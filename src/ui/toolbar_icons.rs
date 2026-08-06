//! Iconos vectoriales procedurales para la barra de herramientas.
//!
//! Dibujados con primitivas seguras de egui (rect_filled, rect_stroke,
//! line_segment, circle_filled, circle_stroke). Se evita convex_polygon
//! porque geometría degenerada puede romper el frame de renderizado.

use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke};

const TAU: f32 = std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarIcon {
    Prev,
    Next,
    RotateCw,
    Fit,
    Fullscreen,
    Sidebar,
    Info,
    Slideshow,
    Edit,
    Settings,
}

impl ToolbarIcon {
    pub fn paint(self, painter: &egui::Painter, rect: Rect, color: Color32) {
        let c = rect.center();
        let r = rect.width().min(rect.height()) * 0.5;
        match self {
            Self::Prev => chevron(painter, c, r, color, false),
            Self::Next => chevron(painter, c, r, color, true),
            Self::RotateCw => rotate_cw(painter, c, r, color),
            Self::Fit => fit_to_window(painter, c, r, color),
            Self::Fullscreen => fullscreen(painter, c, r, color),
            Self::Sidebar => sidebar(painter, c, r, color),
            Self::Info => info(painter, c, r, color),
            Self::Slideshow => play(painter, c, r, color),
            Self::Edit => pencil(painter, c, r, color),
            Self::Settings => gear(painter, c, r, color),
        }
    }
}

fn sw(r: f32) -> f32 {
    (r * 0.13).clamp(1.1, 2.2)
}

fn chevron(painter: &egui::Painter, c: Pos2, r: f32, color: Color32, left: bool) {
    let s = sw(r);
    let dir: f32 = if left { -1.0 } else { 1.0 };
    let stroke = Stroke::new(s, color);
    let a = Pos2::new(c.x + dir * r * 0.5, c.y - r * 0.7);
    let b = Pos2::new(c.x - dir * r * 0.4, c.y);
    let d = Pos2::new(c.x + dir * r * 0.5, c.y + r * 0.7);
    painter.line_segment([a, b], stroke);
    painter.line_segment([b, d], stroke);
}

fn rotate_cw(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let s = sw(r);
    let stroke = Stroke::new(s, color);
    let radius = r * 0.58;
    let segments = 28;
    let start = 215f32.to_radians();
    let end = 100f32.to_radians();
    let sweep = TAU - (start - end);
    let points: Vec<Pos2> = (0..=segments)
        .map(|i| {
            let t = i as f32 / segments as f32;
            let angle = start + t * sweep;
            Pos2::new(c.x + radius * angle.cos(), c.y + radius * angle.sin())
        })
        .collect();
    painter.add(egui::Shape::line(points.clone(), stroke));
    if let Some(tip) = points.last() {
        let dir = end - std::f32::consts::FRAC_PI_2;
        let back = Pos2::new(tip.x - s * 3.0 * dir.cos(), tip.y - s * 3.0 * dir.sin());
        let perp = Pos2::new(-dir.sin(), dir.cos());
        let w = s * 1.6;
        painter.line_segment([*tip, back], stroke);
        painter.line_segment([back, Pos2::new(back.x + perp.x * w, back.y + perp.y * w)], stroke);
        painter.line_segment([back, Pos2::new(back.x - perp.x * w, back.y - perp.y * w)], stroke);
    }
}

fn fit_to_window(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let s = sw(r);
    let stroke = Stroke::new(s, color);
    let outer = r * 0.75;
    let inner = r * 0.35;
    painter.rect_stroke(
        Rect::from_center_size(c, egui::vec2(outer * 2.0, outer * 2.0)),
        0.0,
        stroke,
        egui::StrokeKind::Inside,
    );
    painter.rect_filled(
        Rect::from_center_size(c, egui::vec2(inner * 2.0, inner * 2.0)),
        0.0,
        color,
    );
}

fn fullscreen(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let s = sw(r);
    let stroke = Stroke::new(s, color);
    let half = r * 0.72;
    let len = r * 0.32;
    let rect = Rect::from_center_size(c, egui::vec2(half * 2.0, half * 2.0));
    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
    let corners = [
        (rect.left_top(), 1.0_f32, 1.0_f32),
        (rect.right_top(), -1.0_f32, 1.0_f32),
        (rect.left_bottom(), 1.0_f32, -1.0_f32),
        (rect.right_bottom(), -1.0_f32, -1.0_f32),
    ];
    for (corner, dx, dy) in corners {
        painter.line_segment([corner, Pos2::new(corner.x + dx * len, corner.y)], stroke);
        painter.line_segment([corner, Pos2::new(corner.x, corner.y + dy * len)], stroke);
    }
}

fn sidebar(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let s = sw(r);
    let stroke = Stroke::new(s, color);
    let left = c.x - r * 0.6;
    let right = c.x + r * 0.6;
    let top = c.y - r * 0.65;
    let bottom = c.y + r * 0.65;
    painter.line_segment([Pos2::new(left, top), Pos2::new(left, bottom)], stroke);
    for i in 0..3 {
        let y = top + (bottom - top) * (i as f32 + 0.5) / 3.0;
        painter.line_segment([Pos2::new(left + r * 0.12, y), Pos2::new(right, y)], stroke);
    }
}

fn info(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let s = sw(r);
    let stroke = Stroke::new(s, color);
    let radius = r * 0.55;
    painter.circle_stroke(c, radius, stroke);
    painter.circle_filled(Pos2::new(c.x, c.y - radius * 0.42), s * 0.9, color);
    painter.line_segment(
        [
            Pos2::new(c.x, c.y - radius * 0.18),
            Pos2::new(c.x, c.y + radius * 0.45),
        ],
        stroke,
    );
}

fn play(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let s = sw(r);
    let stroke = Stroke::new(s, color);
    let size = r * 0.5;
    let half = r * 0.55;
    let a = Pos2::new(c.x - size, c.y - half);
    let b = Pos2::new(c.x - size, c.y + half);
    let d = Pos2::new(c.x + size, c.y);
    painter.line_segment([a, b], stroke);
    painter.line_segment([b, d], stroke);
    painter.line_segment([d, a], stroke);
    let inner = s * 0.6;
    painter.line_segment([Pos2::new(a.x + inner, a.y + inner), Pos2::new(b.x + inner, b.y - inner)], stroke);
    painter.line_segment([Pos2::new(b.x + inner, b.y - inner), Pos2::new(d.x - inner * 1.2, c.y)], stroke);
    painter.line_segment([Pos2::new(d.x - inner * 1.2, c.y), Pos2::new(a.x + inner, a.y + inner)], stroke);
}

fn pencil(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let half_w = r * 0.28;
    let top = c.y - r * 0.7;
    let mid = c.y + r * 0.2;
    let bot = c.y + r * 0.8;
    painter.rect_filled(
        Rect::from_center_size(Pos2::new(c.x, (top + mid) / 2.0), egui::vec2(half_w * 2.0, mid - top)),
        0.0,
        color,
    );
    let s = sw(r);
    let stroke = Stroke::new(s, color);
    painter.line_segment([Pos2::new(c.x - half_w, mid), Pos2::new(c.x, bot)], stroke);
    painter.line_segment([Pos2::new(c.x + half_w, mid), Pos2::new(c.x, bot)], stroke);
    painter.line_segment([Pos2::new(c.x - half_w, mid), Pos2::new(c.x + half_w, mid)], stroke);
}

fn gear(painter: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let s = sw(r);
    let stroke = Stroke::new(s, color);
    let outer = r * 0.7;
    let hub = r * 0.28;
    painter.circle_stroke(c, outer, stroke);
    painter.circle_filled(c, hub, color);
    let teeth = 8;
    let tooth_len = r * 0.12;
    for i in 0..teeth {
        let a = (i as f32) * TAU / teeth as f32;
        let p1 = Pos2::new(c.x + outer * a.cos(), c.y + outer * a.sin());
        let p2 = Pos2::new(c.x + (outer + tooth_len) * a.cos(), c.y + (outer + tooth_len) * a.sin());
        painter.line_segment([p1, p2], stroke);
    }
}
