//! Barra de estado inferior con la información de la imagen actual.
//!
//! Solo presenta: recibe un `StatusInfo` ya construido por `app.rs`. El único
//! formateo con lógica (`format_status`, `format_bytes`) es puro y testeable.

use eframe::egui;

/// Información de la imagen actual que muestra la status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusInfo {
    /// Nombre del archivo (sin ruta).
    pub name: String,
    /// Ancho de la imagen en píxeles (sin rotar).
    pub width: u32,
    /// Alto de la imagen en píxeles (sin rotar).
    pub height: u32,
    /// Tamaño en disco en bytes; `None` si no se pudo leer.
    pub size_bytes: Option<u64>,
    /// Índice 1-based de la imagen actual.
    pub index: usize,
    /// Total de imágenes de la carpeta.
    pub total: usize,
}

/// Formatea una cantidad de bytes como `"512 B"`, `"4.2 MB"`, etc.
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Construye la línea de la status bar: `"img.png · 3840×2160 · 4.2 MB · 3/50"`.
pub fn format_status(info: &StatusInfo) -> String {
    let size = info
        .size_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "—".to_string());
    format!(
        "{} · {}×{} · {} · {}/{}",
        info.name, info.width, info.height, size, info.index, info.total
    )
}

/// Pinta la barra inferior con `info`.
pub fn show(ui: &mut egui::Ui, info: &StatusInfo) {
    egui::Panel::bottom("statusbar")
        .exact_size(24.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format_status(info));
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, w: u32, h: u32, bytes: Option<u64>, idx: usize, total: usize) -> StatusInfo {
        StatusInfo {
            name: name.to_string(),
            width: w,
            height: h,
            size_bytes: bytes,
            index: idx,
            total,
        }
    }

    #[test]
    fn format_bytes_handles_all_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(4_404_019), "4.2 MB");
        assert_eq!(format_bytes(5_368_709_120), "5.0 GB");
    }

    #[test]
    fn format_status_shows_all_fields() {
        let s = format_status(&info("img_0042.png", 3840, 2160, Some(4_404_019), 3, 50));
        assert_eq!(s, "img_0042.png · 3840×2160 · 4.2 MB · 3/50");
    }

    #[test]
    fn format_status_missing_size_uses_dash() {
        let s = format_status(&info("a.jpg", 64, 64, None, 1, 1));
        assert_eq!(s, "a.jpg · 64×64 · — · 1/1");
    }

    /// Congela el formato de la status bar con una tabla de casos.
    #[test]
    fn snapshot_statusbar_format() {
        let cases = [
            info("img_0042.png", 3840, 2160, Some(4_404_019), 3, 50),
            info("tiny.jpg", 64, 64, Some(512), 1, 1),
            info("no-size.tiff", 100, 200, None, 2, 7),
            info("big_file_with_a_long_name.jpeg", 8, 8, Some(5_368_709_120), 9, 9),
        ];
        let lines: Vec<String> = cases.iter().map(format_status).collect();
        insta::assert_snapshot!(lines.join("\n"));
    }
}
