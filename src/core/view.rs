use std::ops::{Add, Mul, Sub};
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
pub const MAX_ZOOM: f32 = 8.0;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    pub zoom: f32,
    pub image_size: Vec2,
    pub viewport: Vec2,
    pub rotation: u8,
    pub pan: Vec2,
}

impl ViewTransform {
    pub fn new(image_size: Vec2, viewport: Vec2) -> Self {
        let mut t = Self {
            zoom: 1.0,
            image_size,
            viewport,
            rotation: 0,
            pan: Vec2::ZERO,
        };
        t.fit();
        t
    }
    pub fn effective_size(&self) -> Vec2 {
        if self.rotation % 2 == 1 {
            Vec2::new(self.image_size.y, self.image_size.x)
        } else {
            self.image_size
        }
    }
    pub fn fit_zoom(&self) -> f32 {
        let size = self.effective_size();
        if size.x <= 0.0 || size.y <= 0.0 || self.viewport.x <= 0.0 || self.viewport.y <= 0.0 {
            return 1.0;
        }
        let zx = self.viewport.x / size.x;
        let zy = self.viewport.y / size.y;
        zx.min(zy)
    }
    pub fn image_origin_screen(&self) -> Vec2 {
        let center = self.viewport.mul(0.5);
        let size = self.effective_size();
        let half = size.mul(self.zoom * 0.5);
        center.sub(half).add(self.pan)
    }
    pub fn fit(&mut self) {
        self.zoom = self.fit_zoom();
        self.pan = Vec2::ZERO;
    }
    pub fn apply_zoom_at(&mut self, screen_pos: Vec2, factor: f32) {
        let fit = self.fit_zoom();
        let new_zoom = (self.zoom * factor).clamp(fit, fit * MAX_ZOOM);
        if (new_zoom - self.zoom).abs() < f32::EPSILON {
            return;
        }
        let d = screen_pos.sub(self.viewport.mul(0.5));
        let ratio = new_zoom / self.zoom;
        let new_pan = self.pan.mul(ratio).add(d.mul(1.0 - ratio));
        self.zoom = new_zoom;
        self.pan = new_pan;
        self.clamp_pan();
    }
    pub fn pan_by(&mut self, delta: Vec2) {
        self.pan = self.pan.add(delta);
        self.clamp_pan();
    }
    pub fn clamp_pan(&mut self) {
        let size = self.effective_size();
        let img_w = size.x * self.zoom;
        let img_h = size.y * self.zoom;
        let vp_w = self.viewport.x;
        let vp_h = self.viewport.y;
        let max_x = ((img_w - vp_w) / 2.0).max(0.0);
        let max_y = ((img_h - vp_h) / 2.0).max(0.0);
        self.pan.x = self.pan.x.clamp(-max_x, max_x);
        self.pan.y = self.pan.y.clamp(-max_y, max_y);
    }
    pub fn screen_to_image(&self, screen_pos: Vec2) -> Vec2 {
        let origin = self.image_origin_screen();
        let within_viewport = screen_pos.sub(origin);
        Vec2::new(within_viewport.x / self.zoom, within_viewport.y / self.zoom)
    }
    pub fn image_to_screen(&self, img: Vec2) -> Vec2 {
        let origin = self.image_origin_screen();
        origin.add(Vec2::new(img.x * self.zoom, img.y * self.zoom))
    }
    pub fn set_viewport(&mut self, viewport: Vec2) {
        self.viewport = viewport;
        self.clamp_pan();
    }
    pub fn rotate_cw(&mut self) {
        self.rotation = (self.rotation + 1) % 4;
        self.fit();
    }
    pub fn rotate_ccw(&mut self) {
        self.rotation = (self.rotation + 3) % 4;
        self.fit();
    }
}

impl ViewTransform {
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
    fn apply_zoom_at_center_keeps_image_centered() {
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
            t.apply_zoom_at(center, 2.0);
        }
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
    fn apply_zoom_at_cursor_keeps_point_fixed() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        let cursor = Vec2::new(300.0, 200.0);
        let img_before = t.screen_to_image(cursor);
        t.apply_zoom_at(cursor, 2.0);
        let img_after = t.screen_to_image(cursor);
        assert!(
            approx(img_before.x, img_after.x),
            "x fija bajo cursor: {} vs {}",
            img_before.x,
            img_after.x
        );
        assert!(
            approx(img_before.y, img_after.y),
            "y fija bajo cursor: {} vs {}",
            img_before.y,
            img_after.y
        );
    }

    #[test]
    fn image_origin_is_always_centered_at_fit() {
        let mut t = ViewTransform::new(Vec2::new(2000.0, 1000.0), Vec2::new(500.0, 500.0));
        let expected =
            |zoom: f32| Vec2::new((500.0 - 2000.0 * zoom) / 2.0, (500.0 - 1000.0 * zoom) / 2.0);
        t.apply_zoom_at(Vec2::new(250.0, 250.0), 1.0);
        for _ in 0..8 {
            let o = t.image_origin_screen();
            let e = expected(t.zoom);
            assert!(approx(o.x, e.x), "TL-x centrada en zoom {}", t.zoom);
            assert!(approx(o.y, e.y), "TL-y centrada en zoom {}", t.zoom);
            t.apply_zoom_at(Vec2::new(250.0, 250.0), 1.5);
        }
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
        t.apply_zoom_at(Vec2::new(300.0, 200.0), 3.0);
        t.pan_by(Vec2::new(50.0, -30.0));
        t.fit();
        let expected = t.fit_zoom();
        assert!(approx(t.zoom, expected), "fit restaura zoom");
        assert_eq!(t.pan, Vec2::ZERO, "fit resetea pan");
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
        t.apply_zoom_at(Vec2::new(300.0, 200.0), 3.0);
        t.pan_by(Vec2::new(50.0, -30.0));
        t.rotate_cw();
        assert!(approx(t.zoom, t.fit_zoom()), "rota → fit");
        assert_eq!(t.pan, Vec2::ZERO, "rota resetea pan");
        let origin = t.image_origin_screen();
        assert!(
            approx(origin.x, (500.0 - 1000.0 * t.zoom) / 2.0),
            "quedó centrada en x"
        );
        assert!(
            approx(origin.y, (500.0 - 1000.0 * t.zoom) / 2.0),
            "quedó centrada en y"
        );
    }

    #[test]
    fn rotated_uv_permutes_corners() {
        assert_eq!(ViewTransform::rotated_uv(0, 0), (0.0, 0.0));
        assert_eq!(ViewTransform::rotated_uv(1, 0), (1.0, 0.0));
        assert_eq!(ViewTransform::rotated_uv(0, 1), (0.0, 1.0));
        assert_eq!(ViewTransform::rotated_uv(1, 1), (0.0, 0.0));
        assert_eq!(ViewTransform::rotated_uv(2, 2), (0.0, 0.0));
        assert_eq!(ViewTransform::rotated_uv(0, 3), (1.0, 0.0));
    }

    #[test]
    fn pan_by_shifts_image_origin() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        t.apply_zoom_at(Vec2::new(250.0, 250.0), 4.0); // zoom in para poder pan
        let origin_before = t.image_origin_screen();
        t.pan_by(Vec2::new(10.0, -20.0));
        let origin_after = t.image_origin_screen();
        assert!(approx(origin_after.x, origin_before.x + 10.0));
        assert!(approx(origin_after.y, origin_before.y - 20.0));
        assert!(approx(t.pan.x, 10.0));
        assert!(approx(t.pan.y, -20.0));
    }

    #[test]
    fn clamp_pan_limits_when_image_larger_than_viewport() {
        let mut t = ViewTransform::new(Vec2::new(2000.0, 2000.0), Vec2::new(500.0, 500.0));
        t.apply_zoom_at(Vec2::new(250.0, 250.0), 2.0);
        t.pan_by(Vec2::new(10000.0, -10000.0));
        assert!(t.pan.x <= 1750.0 + 1e-3, "pan.x = {}", t.pan.x);
        assert!(t.pan.y >= -(1750.0 + 1e-3), "pan.y = {}", t.pan.y);
    }

    #[test]
    fn clamp_pan_zero_when_image_fits_viewport() {
        let mut t = ViewTransform::new(Vec2::new(100.0, 100.0), Vec2::new(500.0, 500.0));
        t.pan_by(Vec2::new(50.0, 50.0));
        assert_eq!(t.pan, Vec2::ZERO);
    }

    #[test]
    fn screen_to_image_roundtrip() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 800.0), Vec2::new(600.0, 400.0));
        t.apply_zoom_at(Vec2::new(100.0, 100.0), 3.0);
        t.pan_by(Vec2::new(15.0, -10.0));
        let img_pt = Vec2::new(250.0, 200.0);
        let screen = t.image_to_screen(img_pt);
        let back = t.screen_to_image(screen);
        assert!(
            approx(back.x, img_pt.x),
            "roundtrip x: {} vs {}",
            back.x,
            img_pt.x
        );
        assert!(
            approx(back.y, img_pt.y),
            "roundtrip y: {} vs {}",
            back.y,
            img_pt.y
        );
    }

    #[test]
    fn zoom_at_extreme_cursor_keeps_clamped_pan() {
        let mut t = ViewTransform::new(Vec2::new(1000.0, 1000.0), Vec2::new(500.0, 500.0));
        t.apply_zoom_at(Vec2::new(0.0, 0.0), 8.0); // esquina sup-izq, zoom máximo
        let effective = t.effective_size();
        let max_x = (effective.x * t.zoom - t.viewport.x) / 2.0;
        let max_y = (effective.y * t.zoom - t.viewport.y) / 2.0;
        assert!(
            t.pan.x.abs() <= max_x + 1e-3,
            "pan.x={} max={}",
            t.pan.x,
            max_x
        );
        assert!(
            t.pan.y.abs() <= max_y + 1e-3,
            "pan.y={} max={}",
            t.pan.y,
            max_y
        );
    }
    #[test]
    fn snapshot_rotation_math() {
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
