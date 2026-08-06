use image::{DynamicImage, GenericImageView};
pub const THUMB_MAX: u32 = 96;
pub fn thumbnail_size(w: u32, h: u32, max: u32) -> (u32, u32) {
    if max == 0 || w == 0 || h == 0 {
        return (0, 0);
    }
    let (w, h) = (w as f64, h as f64);
    let max = max as f64;
    if w <= max && h <= max {
        return (w as u32, h as u32);
    }
    let scale = max / w.max(h);
    let nw = (w * scale).round() as u32;
    let nh = (h * scale).round() as u32;
    (nw.max(1), nh.max(1))
}
pub fn generate_thumbnail(image: &DynamicImage, max: u32) -> DynamicImage {
    let (w, h) = image.dimensions();
    let (nw, nh) = thumbnail_size(w, h, max);
    if nw == 0 || (nw == w && nh == h) {
        return image.clone();
    }
    image.thumbnail(nw, nh)
}

pub fn generate_thumbnail_capped(image: &DynamicImage, max: u32) -> DynamicImage {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return image.clone();
    }
    let (nw, nh) = thumbnail_size(w, h, max);
    if nw == w && nh == h {
        return image.thumbnail(nw, nh);
    }
    image.thumbnail(nw, nh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, RgbaImage};

    fn rgba(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(w, h))
    }

    #[test]
    fn thumbnail_size_keeps_aspect_ratio_wide() {
        assert_eq!(thumbnail_size(1920, 1080, THUMB_MAX), (96, 54));
    }

    #[test]
    fn thumbnail_size_keeps_aspect_ratio_tall() {
        assert_eq!(thumbnail_size(1080, 1920, THUMB_MAX), (54, 96));
    }

    #[test]
    fn thumbnail_size_keeps_aspect_ratio_square() {
        assert_eq!(thumbnail_size(100, 100, THUMB_MAX), (96, 96));
    }

    #[test]
    fn thumbnail_size_does_not_upscale() {
        assert_eq!(thumbnail_size(50, 30, THUMB_MAX), (50, 30));
    }

    #[test]
    fn thumbnail_size_zero_max_returns_zero() {
        assert_eq!(thumbnail_size(1920, 1080, 0), (0, 0));
    }

    #[test]
    fn thumbnail_size_zero_dimension_returns_zero() {
        assert_eq!(thumbnail_size(0, 1080, THUMB_MAX), (0, 0));
        assert_eq!(thumbnail_size(1920, 0, THUMB_MAX), (0, 0));
    }

    #[test]
    fn thumbnail_size_never_returns_zero_for_small_dimensions() {
        assert_eq!(thumbnail_size(1, 1, THUMB_MAX), (1, 1));
        assert_eq!(thumbnail_size(1920, 1, THUMB_MAX), (96, 1));
    }

    #[test]
    fn generate_thumbnail_downscales_to_max() {
        let img = generate_thumbnail(&rgba(1920, 1080), THUMB_MAX);
        assert_eq!(img.dimensions(), (96, 54));
    }

    #[test]
    fn generate_thumbnail_small_image_unchanged() {
        let img = generate_thumbnail(&rgba(50, 30), THUMB_MAX);
        assert_eq!(img.dimensions(), (50, 30));
    }

    #[test]
    fn generate_thumbnail_zero_max_returns_original() {
        let img = generate_thumbnail(&rgba(1920, 1080), 0);
        assert_eq!(img.dimensions(), (1920, 1080));
    }
}
