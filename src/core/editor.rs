//! Image editing operations (crop, filters, adjustments).

use image::{DynamicImage, GenericImageView, Rgba};

use crate::core::edit_state::{CropRect, Filter};

pub fn apply_crop(image: &DynamicImage, rect: CropRect) -> DynamicImage {
    let (img_w, img_h) = image.dimensions();
    let x = rect.x.min(img_w.saturating_sub(1));
    let y = rect.y.min(img_h.saturating_sub(1));
    let w = rect.w.min(img_w - x).max(1);
    let h = rect.h.min(img_h - y).max(1);
    image.crop_imm(x, y, w, h)
}

pub fn apply_filter(image: &DynamicImage, filter: Filter) -> DynamicImage {
    match filter {
        Filter::Grayscale => image.grayscale(),
        Filter::Sepia => apply_sepia(image),
        Filter::Invert => {
            let mut img = image.clone();
            img.invert();
            img
        }
        Filter::BlackWhite => apply_black_white(image),
    }
}

pub fn apply_adjustments(
    image: &DynamicImage,
    brightness: i32,
    contrast: i32,
    saturation: i32,
) -> DynamicImage {
    let mut img = image.clone();
    if brightness != 0 {
        img = img.brighten(brightness);
    }
    if contrast != 0 {
        let factor = (contrast as f32 + 100.0) / 100.0;
        img = img.adjust_contrast(factor);
    }
    if saturation != 0 {
        img = apply_saturation(&img, saturation);
    }
    img
}

pub fn apply_all(
    image: &DynamicImage,
    crop_rect: Option<CropRect>,
    filter: Option<Filter>,
    brightness: i32,
    contrast: i32,
    saturation: i32,
) -> DynamicImage {
    let mut result = match crop_rect {
        Some(rect) => apply_crop(image, rect),
        None => image.clone(),
    };
    if let Some(f) = filter {
        result = apply_filter(&result, f);
    }
    if brightness != 0 || contrast != 0 || saturation != 0 {
        result = apply_adjustments(&result, brightness, contrast, saturation);
    }
    result
}

fn apply_sepia(image: &DynamicImage) -> DynamicImage {
    let mut img = image.to_rgba8();
    for pixel in img.pixels_mut() {
        let Rgba([r, g, b, a]) = *pixel;
        let tr = (0.393 * r as f32 + 0.769 * g as f32 + 0.189 * b as f32).min(255.0) as u8;
        let tg = (0.349 * r as f32 + 0.686 * g as f32 + 0.168 * b as f32).min(255.0) as u8;
        let tb = (0.272 * r as f32 + 0.534 * g as f32 + 0.131 * b as f32).min(255.0) as u8;
        *pixel = Rgba([tr, tg, tb, a]);
    }
    DynamicImage::ImageRgba8(img)
}

fn apply_black_white(image: &DynamicImage) -> DynamicImage {
    let mut img = image.to_rgba8();
    for pixel in img.pixels_mut() {
        let Rgba([r, g, b, a]) = *pixel;
        let lum = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u8;
        let bw = if lum > 127 { 255 } else { 0 };
        *pixel = Rgba([bw, bw, bw, a]);
    }
    DynamicImage::ImageRgba8(img)
}

fn apply_saturation(image: &DynamicImage, amount: i32) -> DynamicImage {
    let factor = (amount as f32 + 100.0) / 100.0;
    let mut img = image.to_rgba8();
    for pixel in img.pixels_mut() {
        let Rgba([r, g, b, a]) = *pixel;
        let gray = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
        let nr = (gray + factor * (r as f32 - gray)).clamp(0.0, 255.0) as u8;
        let ng = (gray + factor * (g as f32 - gray)).clamp(0.0, 255.0) as u8;
        let nb = (gray + factor * (b as f32 - gray)).clamp(0.0, 255.0) as u8;
        *pixel = Rgba([nr, ng, nb, a]);
    }
    DynamicImage::ImageRgba8(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image() -> DynamicImage {
        let mut img = image::RgbaImage::new(4, 4);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = Rgba([(x * 60) as u8, (y * 60) as u8, 100, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn crop_returns_smaller_image() {
        let img = test_image();
        let cropped = apply_crop(
            &img,
            CropRect {
                x: 1,
                y: 1,
                w: 2,
                h: 2,
            },
        );
        assert_eq!(cropped.dimensions(), (2, 2));
    }

    #[test]
    fn crop_clamps_to_bounds() {
        let img = test_image();
        let cropped = apply_crop(
            &img,
            CropRect {
                x: 2,
                y: 2,
                w: 100,
                h: 100,
            },
        );
        assert_eq!(cropped.dimensions(), (2, 2));
    }

    #[test]
    fn grayscale_preserves_dimensions() {
        let img = test_image();
        let gray = apply_filter(&img, Filter::Grayscale);
        assert_eq!(gray.dimensions(), img.dimensions());
    }

    #[test]
    fn invert_changes_pixels() {
        let img = test_image();
        let inverted = apply_filter(&img, Filter::Invert);
        let orig_pixel = img.get_pixel(0, 0);
        let inv_pixel = inverted.get_pixel(0, 0);
        assert_eq!(inv_pixel.0[0], 255 - orig_pixel.0[0]);
    }

    #[test]
    fn brightness_increases_values() {
        let img = test_image();
        let bright = apply_adjustments(&img, 50, 0, 0);
        let orig = img.get_pixel(2, 2);
        let brighter = bright.get_pixel(2, 2);
        assert!(brighter.0[0] >= orig.0[0] || brighter.0[1] >= orig.0[1]);
    }

    #[test]
    fn apply_all_with_no_adjustments_returns_original_size() {
        let img = test_image();
        let result = apply_all(&img, None, None, 0, 0, 0);
        assert_eq!(result.dimensions(), img.dimensions());
    }

    #[test]
    fn apply_all_with_crop_returns_cropped_size() {
        let img = test_image();
        let result = apply_all(
            &img,
            Some(CropRect {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            }),
            None,
            0,
            0,
            0,
        );
        assert_eq!(result.dimensions(), (2, 2));
    }

    #[test]
    fn sepia_preserves_dimensions() {
        let img = test_image();
        let sepia = apply_filter(&img, Filter::Sepia);
        assert_eq!(sepia.dimensions(), img.dimensions());
    }

    #[test]
    fn black_white_only_black_or_white() {
        let img = test_image();
        let bw = apply_filter(&img, Filter::BlackWhite);
        let (w, h) = bw.dimensions();
        for y in 0..h {
            for x in 0..w {
                let pixel = bw.get_pixel(x, y);
                let Rgba([r, g, b, _]) = pixel;
                assert!(r == 0 || r == 255);
                assert_eq!(r, g);
                assert_eq!(g, b);
            }
        }
    }
}
