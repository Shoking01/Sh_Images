//! Build-script: genera `OUT_DIR/icon.ico` desde `assets/icon.svg` y
//! (en Windows) incrusta el icono en el `.exe`.
//!
//! Pipeline: SVG → `usvg::Tree` → `resvg::render` a RGBA8 multi-res →
//! empaquetado en `.ico` multi-imagen → `OUT_DIR/icon.ico`.
//! En Windows, `winres` incrusta el `.ico` como icono del exe.

extern crate ico;
extern crate resvg;

use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;

// resvg re-exporta usvg y tiny_skia.
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, Tree as UsvgTree};

/// Resoluciones del icono (en píxeles). La viewBox del SVG es 512×512.
const ICON_SIZES: &[u32] = &[16, 32, 48, 64, 128, 256];
const VIEWBOX_SIZE: f32 = 512.0;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.svg");
    println!("cargo:rerun-if-changed=build.rs");

    let svg_path = PathBuf::from("assets/icon.svg");
    let svg_data = match fs::read(&svg_path) {
        Ok(d) => d,
        Err(e) => {
            println!("cargo:warning=no se pudo leer assets/icon.svg: {e}");
            return;
        }
    };

    let tree = match UsvgTree::from_data(&svg_data, &usvg::Options::default()) {
        Ok(t) => t,
        Err(e) => {
            println!("cargo:warning=SVG inválido: {e}");
            return;
        }
    };

    let mut rgba_bufs: Vec<(u32, Vec<u8>)> = Vec::new();
    for &size in ICON_SIZES {
        let mut pixmap = Pixmap::new(size, size).expect("pixmap alloc");
        let scale = size as f32 / VIEWBOX_SIZE;
        let mut pm = pixmap.as_mut();
        resvg::render(&tree, Transform::from_scale(scale, scale), &mut pm);
        rgba_bufs.push((size, pixmap.data().to_vec()));
    }

    let ico_bytes = encode_ico(&rgba_bufs);
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("icon.ico"), &ico_bytes).expect("failed to write icon.ico");
    println!(
        "cargo:warning=icon.ico generado ({} bytes)",
        ico_bytes.len()
    );

    // Emit a Rust module with the 256x256 RGBA pixel data so main.rs can
    // include!() it and pass it to eframe::ViewportBuilder::with_icon().
    let (icon_size, icon_rgba) = rgba_bufs.last().expect("icon rgba");
    let mut rs = String::new();
    rs.push_str(&format!("pub const ICON_WIDTH: u32 = {};\n", icon_size));
    rs.push_str(&format!("pub const ICON_HEIGHT: u32 = {};\n", icon_size));
    rs.push_str("pub const ICON_RGBA: &[u8] = &[\n");
    for (i, byte) in icon_rgba.iter().enumerate() {
        if i % 32 == 0 {
            rs.push_str("    ");
        }
        rs.push_str(&format!("{},", byte));
        if i % 32 == 31 {
            rs.push('\n');
        }
    }
    if icon_rgba.len() % 32 != 0 {
        rs.push('\n');
    }
    rs.push_str("];\n");
    fs::write(out_dir.join("icon_data.rs"), rs).expect("failed to write icon_data.rs");

    embed_icon(&ico_bytes);
}

/// Codifica RGBA8 multi-resolución como un `.ico` multi-imagen estándar.
fn encode_ico(rgba_bufs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    let n = rgba_bufs.len();

    // ICONDIR (6 bytes)
    buf.write_all(&0u16.to_le_bytes()).unwrap();
    buf.write_all(&1u16.to_le_bytes()).unwrap(); // type = ICO
    buf.write_all(&(n as u16).to_le_bytes()).unwrap(); // count

    // Pre-calcular offsets
    let header_size = 6 + (n * 16);
    let mut offsets: Vec<u32> = Vec::with_capacity(n);
    let mut current_offset = header_size as u32;

    for &(w, ref rgba) in rgba_bufs {
        let h = w;
        let pixel_bytes = rgba.len();
        let and_mask_bytes = (w as usize).div_ceil(32) * 4 * h as usize;
        let image_size = 40 + pixel_bytes + and_mask_bytes;
        offsets.push(current_offset);
        current_offset += image_size as u32;
    }

    // ICONDIRENTRYs
    for (i, &(w, _)) in rgba_bufs.iter().enumerate() {
        let h = w;
        let (_, rgba) = &rgba_bufs[i];
        let pixel_bytes = rgba.len();
        let and_mask_bytes = (w as usize).div_ceil(32) * 4 * h as usize;
        let image_size = 40 + pixel_bytes + and_mask_bytes;

        buf.write_all(&[w.min(256) as u8]).unwrap();
        buf.write_all(&[h.min(256) as u8]).unwrap();
        buf.write_all(&[0u8]).unwrap();
        buf.write_all(&[0u8]).unwrap();
        buf.write_all(&1u16.to_le_bytes()).unwrap();
        buf.write_all(&32u16.to_le_bytes()).unwrap();
        buf.write_all(&(image_size as u32).to_le_bytes()).unwrap();
        buf.write_all(&offsets[i].to_le_bytes()).unwrap();
    }

    // Image data blocks (BITMAPINFOHEADER + BGRA + AND mask)
    for &(w, ref rgba) in rgba_bufs {
        let h = w;

        buf.write_all(&40u32.to_le_bytes()).unwrap();
        buf.write_all(&(w as i32).to_le_bytes()).unwrap();
        buf.write_all(&((h as i32) * 2).to_le_bytes()).unwrap();
        buf.write_all(&1u16.to_le_bytes()).unwrap();
        buf.write_all(&32u16.to_le_bytes()).unwrap();
        buf.write_all(&0u32.to_le_bytes()).unwrap(); // BI_RGB
        buf.write_all(&0u32.to_le_bytes()).unwrap(); // image size
        buf.write_all(&0u32.to_le_bytes()).unwrap();
        buf.write_all(&0u32.to_le_bytes()).unwrap();
        buf.write_all(&0u32.to_le_bytes()).unwrap();
        buf.write_all(&0u32.to_le_bytes()).unwrap();

        // BGRA pixel data (swizzle R↔B desde RGBA)
        let mut bgra = Vec::with_capacity(rgba.len());
        for px in rgba.chunks_exact(4) {
            let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
            bgra.extend_from_slice(&[b, g, r, a]);
        }
        buf.write_all(&bgra).unwrap();

        // AND mask: 1bpp, todos 0 = opacos
        let and_mask_len = (w as usize).div_ceil(32) * 4 * h as usize;
        buf.write_all(&vec![0u8; and_mask_len]).unwrap();
    }

    buf.into_inner()
}

/// En Windows: incrusta el icono en el .exe usando `winres`.
/// En Linux/macOS: no-op (el icono se usa solo en Windows).
fn embed_icon(ico_bytes: &[u8]) {
    #[cfg(windows)]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let ico_path = std::path::Path::new(&out_dir).join("embed.ico");
        std::fs::write(&ico_path, ico_bytes).expect("write embed.ico");

        let mut rc = winres::WindowsResource::new();
        rc.set_icon("embed.ico");
        rc.compile().expect("winres compile failed");
    }

    #[cfg(not(windows))]
    let _ = ico_bytes;
}
