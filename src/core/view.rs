//! Matemática pura de zoom/pan/fit para el visor.
//!
//! `core/` no depende de `egui`; este módulo define un vector 2D mínimo propio.

use std::ops::{Add, Div, Mul, Sub};

/// Vector/posición 2D mínimo del módulo core.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x / rhs, self.y / rhs)
    }
}

/// Zoom máximo como múltiplo del tamaño fit (8x el fit).
pub const MAX_ZOOM: f32 = 8.0;

/// Transformación de vista: escala y desplazamiento de la imagen en el canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    pub zoom: f32,
    pub pan: Vec2,
    pub image_size: Vec2,
    pub viewport: Vec2,
}

impl ViewTransform {
    /// Crea una transformación en fit inicial.
    pub fn new(image_size: Vec2, viewport: Vec2) -> Self {
        let mut t = Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            image_size,
            viewport,
        };
        t.fit();
        t
    }

    /// Zoom que hace caber la imagen completa en el viewport.
    pub fn fit_zoom(&self) -> f32 {
        if self.image_size.x <= 0.0
            || self.image_size.y <= 0.0
            || self.viewport.x <= 0.0
            || self.viewport.y <= 0.0
        {
            return 1.0;
        }
        let zx = self.viewport.x / self.image_size.x;
        let zy = self.viewport.y / self.image_size.y;
        zx.min(zy)
    }

    /// Esquina superior izquierda de la imagen en coordenadas de pantalla.
    pub fn image_origin_screen(&self) -> Vec2 {
        let center = self.viewport.mul(0.5);
        let half = self.image_size.mul(self.zoom * 0.5);
        center.sub(half).add(self.pan)
    }

    /// Ajusta a fit completo: zoom = fit, pan = centrado.
    pub fn fit(&mut self) {
        self.zoom = self.fit_zoom();
        self.pan = Vec2::ZERO;
    }

    /// Cambia el zoom por `factor` manteniendo fijo el punto de la imagen bajo `anchor`.
    pub fn apply_zoom_at(&mut self, anchor: Vec2, factor: f32) {
        let fit = self.fit_zoom();
        let min = fit;
        let max = fit * MAX_ZOOM;
        let new_zoom = (self.zoom * factor).clamp(min, max);
        if (new_zoom - self.zoom).abs() < f32::EPSILON {
            return;
        }
        let origin = self.image_origin_screen();
        let image_point = anchor.sub(origin).div(self.zoom);
        let new_origin = anchor.sub(image_point.mul(new_zoom));
        let center = self.viewport.mul(0.5);
        self.pan = new_origin
            .sub(center)
            .add(self.image_size.mul(new_zoom * 0.5));
        self.zoom = new_zoom;
    }

    /// Desplaza el pan libremente (sin clamp).
    pub fn pan_by(&mut self, delta: Vec2) {
        self.pan = self.pan.add(delta);
    }

    /// Actualiza el tamaño del canvas.
    pub fn set_viewport(&mut self, viewport: Vec2) {
        self.viewport = viewport;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn fit_zoom_scales_to_fit_wide_image() {
        let t = ViewTransform::new(Vec2::new(2000.0, 1000.0), Vec2::new(500.0, 500.0));
        assert!(approx(t.fit_zoom(), 0.25)); // min(500/2000, 500/1000)
    }

    #[test]
    fn fit_zoom_scales_to_fit_tall_image() {
        let t = ViewTransform::new(Vec2::new(1000.0, 2000.0), Vec2::new(500.0, 500.0));
        assert!(approx(t.fit_zoom(), 0.25)); // min(500/1000, 500/2000)
    }

    #[test]
    fn fit_zoom_returns_1_on_zero_dimension() {
        let t = ViewTransform::new(Vec2::ZERO, Vec2::new(500.0, 500.0));
        assert_eq!(t.fit_zoom(), 1.0);
    }

    #[test]
    fn apply_zoom_at_keeps_anchor_point_fixed() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let anchor = Vec2::new(150.0, 200.0);
        let origin = t.image_origin_screen();
        let image_point = anchor.sub(origin).div(t.zoom);
        t.apply_zoom_at(anchor, 1.5);
        let new_origin = t.image_origin_screen();
        let new_screen = new_origin.add(image_point.mul(t.zoom));
        assert!(approx(new_screen.x, anchor.x));
        assert!(approx(new_screen.y, anchor.y));
    }

    #[test]
    fn apply_zoom_at_clamps_to_max_zoom() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let fit = t.fit_zoom();
        t.apply_zoom_at(Vec2::new(250.0, 250.0), 1000.0);
        assert!(approx(t.zoom, fit * MAX_ZOOM));
    }

    #[test]
    fn apply_zoom_at_clamps_to_min_fit() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let fit = t.fit_zoom();
        t.apply_zoom_at(Vec2::new(250.0, 250.0), 0.0001);
        assert!(approx(t.zoom, fit));
    }

    #[test]
    fn pan_by_moves_image_by_delta() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let before = t.image_origin_screen();
        t.pan_by(Vec2::new(10.0, -20.0));
        let after = t.image_origin_screen();
        assert!(approx(after.x - before.x, 10.0));
        assert!(approx(after.y - before.y, -20.0));
    }

    #[test]
    fn fit_resets_to_centered_initial() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        t.apply_zoom_at(Vec2::new(100.0, 100.0), 2.0);
        t.pan_by(Vec2::new(50.0, 50.0));
        t.fit();
        let expected = t.fit_zoom();
        assert!(approx(t.zoom, expected));
        // En fit con pan 0, la imagen queda centrada.
        let expected_origin = Vec2::new(
            (500.0 - 1000.0 * expected) / 2.0,
            (500.0 - 1000.0 * expected) / 2.0,
        );
        let origin = t.image_origin_screen();
        assert!(approx(origin.x, expected_origin.x));
        assert!(approx(origin.y, expected_origin.y));
    }
}
