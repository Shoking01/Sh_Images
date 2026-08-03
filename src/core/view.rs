//! Matemática pura de zoom/pan/fit para el visor.
//!
//! `core/` no depende de `egui`; este módulo define un vector 2D mínimo propio.

use std::ops::{Add, Div, Mul, Sub};

/// Vector/posición 2D mínimo del módulo core.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    /// Componente horizontal.
    pub x: f32,
    /// Componente vertical.
    pub y: f32,
}

impl Vec2 {
    /// El vector (0, 0).
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    /// Crea un vector con los componentes dados.
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
    /// Factor de escala actual de la imagen (nunca por debajo de `fit_zoom`).
    pub zoom: f32,
    /// Desplazamiento de la imagen respecto al centro del canvas.
    pub pan: Vec2,
    /// Tamaño en píxeles de la imagen original.
    pub image_size: Vec2,
    /// Tamaño del canvas (área visible) en píxeles de pantalla.
    pub viewport: Vec2,
    /// Rotación de la imagen en cuartos de vuelta: 0=0°, 1=90°CW, 2=180°, 3=270°CW.
    pub rotation: u8,
}

impl ViewTransform {
    /// Crea una transformación en fit inicial.
    pub fn new(image_size: Vec2, viewport: Vec2) -> Self {
        let mut t = Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            image_size,
            viewport,
            rotation: 0,
        };
        t.fit();
        t
    }

    /// Tamaño efectivo de la imagen bajo la rotación actual (dims intercambiadas
    /// si la rotación es impar).
    pub fn effective_size(&self) -> Vec2 {
        if self.rotation % 2 == 1 {
            Vec2::new(self.image_size.y, self.image_size.x)
        } else {
            self.image_size
        }
    }

    /// Zoom que hace caber la imagen completa en el viewport.
    pub fn fit_zoom(&self) -> f32 {
        let size = self.effective_size();
        if size.x <= 0.0 || size.y <= 0.0 || self.viewport.x <= 0.0 || self.viewport.y <= 0.0 {
            return 1.0;
        }
        let zx = self.viewport.x / size.x;
        let zy = self.viewport.y / size.y;
        zx.min(zy)
    }

    /// Esquina superior izquierda de la imagen en coordenadas de pantalla.
    pub fn image_origin_screen(&self) -> Vec2 {
        let center = self.viewport.mul(0.5);
        let size = self.effective_size();
        let half = size.mul(self.zoom * 0.5);
        center.sub(half).add(self.pan)
    }

    /// Ajusta a fit completo: zoom = fit, pan = centrado.
    pub fn fit(&mut self) {
        self.zoom = self.fit_zoom();
        self.pan = Vec2::ZERO;
    }

    /// Aplica un factor de zoom anclado al centro del canvas, sin desplazamiento.
    ///
    /// Como la imagen queda siempre centrada (`image_origin_screen()` deriva la
    /// esquina de `zoom`), solo hay que escalar `self.zoom` y clamp lo entre
    /// `fit_zoom()` (mínimo, imagen completa) y `fit_zoom() * MAX_ZOOM`.
    pub fn apply_center(&mut self, factor: f32) {
        let fit = self.fit_zoom();
        let new_zoom = (self.zoom * factor).clamp(fit, fit * MAX_ZOOM);
        self.zoom = new_zoom;
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

    /// Rota 90° en sentido horario y re-aplica fit (pan a 0).
    pub fn rotate_cw(&mut self) {
        self.rotation = (self.rotation + 1) % 4;
        self.fit();
    }

    /// Rota 90° en sentido antihorario y re-aplica fit (pan a 0).
    pub fn rotate_ccw(&mut self) {
        self.rotation = (self.rotation + 3) % 4;
        self.fit();
    }
}

impl ViewTransform {
    /// UV (normalizado 0..1) del vértice `corner` (0=TL,1=TR,2=BR,3=BL) bajo la
    /// rotación `rotation` (cuartos de vuelta CW). Permite pintar el mesh rotado.
    pub fn rotated_uv(corner: u8, rotation: u8) -> (f32, f32) {
        const UVS: [(f32, f32); 4] = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let idx = ((corner as usize) + 4 - (rotation as usize % 4)) % 4;
        UVS[idx]
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
    fn apply_center_keeps_image_centered() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let center = Vec2::new(250.0, 250.0);
        for _ in 0..6 {
            let origin = t.image_origin_screen();
            let img_center = origin.add(Vec2::new(1000.0 * t.zoom * 0.5, 1000.0 * t.zoom * 0.5));
            assert!(
                approx(img_center.x, center.x),
                "centrada en x en zoom {}",
                t.zoom
            );
            assert!(
                approx(img_center.y, center.y),
                "centrada en y en zoom {}",
                t.zoom
            );
            t.apply_center(2.0);
        }
    }

    #[test]
    fn apply_center_clamps_to_max_zoom() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let fit = t.fit_zoom();
        t.apply_center(1000.0);
        assert!(approx(t.zoom, fit * MAX_ZOOM));
    }

    #[test]
    fn apply_center_clamps_to_min_fit() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let fit = t.fit_zoom();
        t.apply_center(0.0001);
        assert!(approx(t.zoom, fit));
    }

    #[test]
    fn image_origin_is_always_centered() {
        let mut t = ViewTransform::new(Vec2::new(2000.0, 1000.0), Vec2::new(500.0, 500.0));
        let expected =
            |zoom: f32| Vec2::new((500.0 - 2000.0 * zoom) / 2.0, (500.0 - 1000.0 * zoom) / 2.0);
        t.apply_center(1.0);
        for _ in 0..8 {
            let o = t.image_origin_screen();
            let e = expected(t.zoom);
            assert!(approx(o.x, e.x), "TL-x centrada en zoom {}", t.zoom);
            assert!(approx(o.y, e.y), "TL-y centrada en zoom {}", t.zoom);
            t.apply_center(1.5);
        }
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
    fn set_viewport_updates_canvas_size() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let fit_before = t.fit_zoom();
        t.set_viewport(Vec2::new(250.0, 250.0));
        assert_eq!(t.viewport, Vec2::new(250.0, 250.0));
        assert!(approx(t.fit_zoom(), fit_before * 0.5));
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

    #[test]
    fn rotate_cw_cycles_0_to_3_and_back_to_0() {
        let mut t = ViewTransform::new(Vec2::new(100.0, 200.0), Vec2::new(500.0, 500.0));
        assert_eq!(t.rotation, 0);
        t.rotate_cw();
        assert_eq!(t.rotation, 1);
        t.rotate_cw();
        assert_eq!(t.rotation, 2);
        t.rotate_cw();
        assert_eq!(t.rotation, 3);
        t.rotate_cw();
        assert_eq!(t.rotation, 0);
    }

    #[test]
    fn rotate_ccw_is_inverse_of_cw() {
        let mut t = ViewTransform::new(Vec2::new(100.0, 200.0), Vec2::new(500.0, 500.0));
        t.rotate_ccw();
        assert_eq!(t.rotation, 3);
        t.rotate_ccw();
        t.rotate_ccw();
        assert_eq!(t.rotation, 1);
        t.rotate_ccw();
        assert_eq!(t.rotation, 0);
    }

    #[test]
    fn fit_zoom_swaps_dimensions_on_odd_rotation() {
        // Viewport NO cuadrado (2:1): al rotar la imagen (también 2:1) el lado
        // que limita el fit cambia y el fit se reduce.
        let mut t = ViewTransform::new(Vec2::new(1000.0, 500.0), Vec2::new(1000.0, 500.0));
        let fit0 = t.fit_zoom(); // min(1000/1000, 500/500) = 1.0
        t.rotate_cw();
        let fit90 = t.fit_zoom(); // min(1000/500, 500/1000) = 0.5
        assert!(approx(fit90, 0.5));
        assert!(approx(fit0, 1.0));
        t.rotate_cw();
        assert!(approx(t.fit_zoom(), fit0), "180° vuelve a las mismas dims");
    }

    #[test]
    fn rotating_resets_to_fit_and_centers() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        t.apply_zoom_at(Vec2::new(250.0, 250.0), 3.0);
        t.pan_by(Vec2::new(40.0, -20.0));
        t.rotate_cw();
        assert!(approx(t.zoom, t.fit_zoom()), "rota → fit");
        assert_eq!(t.pan, Vec2::ZERO, "rota → pan 0");
    }

    #[test]
    fn rotated_uv_permutes_corners() {
        // corner 0 (top-left) en rotación 1 (90°CW) debe usar el uv del corner 3.
        assert_eq!(ViewTransform::rotated_uv(0, 0), (0.0, 0.0));
        assert_eq!(ViewTransform::rotated_uv(1, 0), (1.0, 0.0));
        assert_eq!(ViewTransform::rotated_uv(0, 1), (0.0, 1.0));
        assert_eq!(ViewTransform::rotated_uv(1, 1), (0.0, 0.0));
        assert_eq!(ViewTransform::rotated_uv(2, 2), (0.0, 0.0));
        assert_eq!(ViewTransform::rotated_uv(0, 3), (1.0, 0.0));
    }

    /// Congela la matemática de rotación (fit_zoom/origin por cuarto de vuelta).
    #[test]
    fn snapshot_rotation_math() {
        // Viewport NO cuadrado (2:1) para que el fit varíe con la rotación.
        let mut lines = Vec::new();
        let mut t = ViewTransform::new(Vec2::new(1000.0, 500.0), Vec2::new(1000.0, 500.0));
        for _ in 0..4 {
            let o = t.image_origin_screen();
            lines.push(format!(
                "rot={} fit={:.4} origin=({:.4},{:.4})",
                t.rotation,
                t.fit_zoom(),
                o.x,
                o.y
            ));
            t.rotate_cw();
        }
        insta::assert_snapshot!(lines.join("\n"));
    }
}
