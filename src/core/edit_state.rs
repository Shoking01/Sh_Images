//! Image editing session state.

use image::DynamicImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    Grayscale,
    Sepia,
    Invert,
    BlackWhite,
}

impl Filter {
    pub fn label(self, lang: crate::core::lang::Language) -> &'static str {
        let t = lang.translations();
        match self {
            Filter::Grayscale => t.filter_grayscale,
            Filter::Sepia => t.filter_sepia,
            Filter::Invert => t.filter_invert,
            Filter::BlackWhite => t.filter_black_white,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl CropRect {
    pub fn from_points(x1: f32, y1: f32, x2: f32, y2: f32, img_w: u32, img_h: u32) -> Self {
        let min_x = x1.min(x2).max(0.0) as u32;
        let min_y = y1.min(y2).max(0.0) as u32;
        let max_x = x1.max(x2).min(img_w as f32) as u32;
        let max_y = y1.max(y2).min(img_h as f32) as u32;
        let w = (max_x - min_x).max(1);
        let h = (max_y - min_y).max(1);
        Self {
            x: min_x,
            y: min_y,
            w,
            h,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditState {
    pub original: DynamicImage,
    pub working: DynamicImage,
    pub crop_rect: Option<CropRect>,
    pub crop_mode: bool,
    pub filter: Option<Filter>,
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
}

impl EditState {
    pub fn new(image: DynamicImage) -> Self {
        Self {
            original: image.clone(),
            working: image,
            crop_rect: None,
            crop_mode: false,
            filter: None,
            brightness: 0,
            contrast: 0,
            saturation: 0,
        }
    }

    pub fn has_crop(&self) -> bool {
        self.crop_rect.is_some()
    }

    pub fn is_crop_mode(&self) -> bool {
        self.crop_mode
    }

    pub fn has_changes(&self) -> bool {
        self.filter.is_some()
            || self.brightness != 0
            || self.contrast != 0
            || self.saturation != 0
            || self.crop_rect.is_some()
    }

    pub fn reset(&mut self) {
        self.working = self.original.clone();
        self.crop_rect = None;
        self.crop_mode = false;
        self.filter = None;
        self.brightness = 0;
        self.contrast = 0;
        self.saturation = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lang::Language;

    fn test_image() -> DynamicImage {
        DynamicImage::ImageRgba8(image::RgbaImage::new(10, 10))
    }

    #[test]
    fn new_state_has_no_changes() {
        let state = EditState::new(test_image());
        assert!(!state.has_changes());
        assert!(!state.has_crop());
    }

    #[test]
    fn reset_restores_original() {
        let mut state = EditState::new(test_image());
        state.brightness = 50;
        state.filter = Some(Filter::Grayscale);
        state.crop_rect = Some(CropRect {
            x: 0,
            y: 0,
            w: 5,
            h: 5,
        });
        state.crop_mode = true;
        assert!(state.has_changes());
        state.reset();
        assert!(!state.has_changes());
        assert_eq!(state.brightness, 0);
        assert!(state.filter.is_none());
        assert!(!state.crop_mode);
    }

    #[test]
    fn filter_label_is_readable() {
        assert_eq!(Filter::Grayscale.label(Language::Es), "Grises");
        assert_eq!(Filter::Sepia.label(Language::Es), "Sepia");
        assert_eq!(Filter::Invert.label(Language::Es), "Invertir");
        assert_eq!(Filter::BlackWhite.label(Language::Es), "B/N");
    }

    #[test]
    fn crop_rect_is_detected() {
        let mut state = EditState::new(test_image());
        assert!(!state.has_crop());
        state.crop_rect = Some(CropRect {
            x: 1,
            y: 1,
            w: 5,
            h: 5,
        });
        assert!(state.has_crop());
    }

    #[test]
    fn crop_rect_from_points_normalizes() {
        let rect1 = CropRect::from_points(10.0, 10.0, 50.0, 50.0, 100, 100);
        let rect2 = CropRect::from_points(50.0, 50.0, 10.0, 10.0, 100, 100);
        assert_eq!(rect1, rect2);
        assert_eq!(rect1.x, 10);
        assert_eq!(rect1.y, 10);
        assert_eq!(rect1.w, 40);
        assert_eq!(rect1.h, 40);
    }

    #[test]
    fn crop_rect_clamps_to_bounds() {
        let rect = CropRect::from_points(-10.0, -10.0, 999.0, 999.0, 100, 100);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.w, 100);
        assert_eq!(rect.h, 100);
    }

    #[test]
    fn crop_rect_minimum_size() {
        let rect = CropRect::from_points(5.0, 5.0, 5.0, 5.0, 100, 100);
        assert_eq!(rect.w, 1);
        assert_eq!(rect.h, 1);
    }
}
