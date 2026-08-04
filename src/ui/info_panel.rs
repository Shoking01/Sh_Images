//! Panel derecho con la información EXIF de la imagen actual.
//!
//! Formatea `ExifImage` en filas campo:valor y pinta un `Panel::right`
//! desplazable. La lectura (I/O) la orquesta `app.rs`; aquí solo se muestra.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use eframe::egui;

use crate::core::exif::{ExifImage, ExifRead, Rational};

/// Estado del panel: solo su visibilidad; el contenido se lee del cache.
#[derive(Debug)]
pub struct InfoPanelState {
    /// Si el panel debe mostrarse en pantalla.
    pub show: bool,
}

impl Default for InfoPanelState {
    fn default() -> Self {
        Self { show: true }
    }
}

/// Tipos de formateo numérico para los racionales EXIF.
#[derive(Debug, Clone, Copy)]
enum Fmt {
    /// Apertura: `f/2.8`.
    Aperture,
    /// Obturador: `1/125 s`.
    Shutter,
    /// Distancia focal: `50 mm`.
    Focal,
}

/// Formatea un `Rational` según el tipo de campo al que pertenece.
fn format_rational(r: Rational, fmt: Fmt) -> String {
    let d = r.to_f64();
    match fmt {
        Fmt::Aperture => format!("f/{d:.1}"),
        Fmt::Shutter => {
            if r.den != 0 && r.num != 0 {
                format!("1/{} s", (r.den as f64 / r.num as f64).round())
            } else {
                format!("{d} s")
            }
        }
        Fmt::Focal => format!("{d:.0} mm"),
    }
}

/// Convierte el código numérico de orientación EXIF en una etiqueta legible.
fn orientation_label(o: u16) -> String {
    match o {
        1 => "Horizontal".to_string(),
        3 | 6 | 8 => format!("Rotada ({o})"),
        other => format!("Vertical ({other})"),
    }
}

/// Convierte `ExifImage` en filas `(etiqueta, valor)`; omite los campos ausentes.
///
/// Se mantienen los campos ausentes fuera de la salida para que el panel no
/// muestre filas vacías.
pub fn field_rows(img: &ExifImage) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    if let Some(v) = &img.make {
        rows.push(("Fabricante".into(), v.clone()));
    }
    if let Some(v) = &img.model {
        rows.push(("Modelo".into(), v.clone()));
    }
    if let Some(v) = &img.fecha {
        rows.push(("Fecha".into(), v.clone()));
    }
    if let Some(v) = img.iso {
        rows.push(("ISO".into(), v.to_string()));
    }
    if let Some(v) = img.f_number {
        rows.push(("Apertura".into(), format_rational(v, Fmt::Aperture)));
    }
    if let Some(v) = img.shutter_speed {
        rows.push(("Obturador".into(), format_rational(v, Fmt::Shutter)));
    }
    if let Some(v) = img.focal_length {
        rows.push(("Focal".into(), format_rational(v, Fmt::Focal)));
    }
    if let Some(o) = img.orientacion {
        rows.push(("Orientación".into(), orientation_label(o)));
    }
    rows
}

/// Pinta el panel derecho. Lee del cache compartido; `None` → "Cargando…".
///
/// `current` es la imagen activa; si el cache aún no la tiene procesada se
/// muestra un texto provisional. Si la lectura falló se muestra un mensaje
/// suave (sin propagar el error al UI thread).
pub fn show(ui: &mut egui::Ui, cache: &Mutex<HashMap<PathBuf, ExifRead>>, current: &Path) {
    let entries = cache.lock().unwrap_or_else(|p| p.into_inner());
    egui::Panel::right("info")
        .default_size(220.0)
        .resizable(true)
        .show(ui, |ui| {
            ui.heading("Información");
            egui::ScrollArea::vertical().show(ui, |ui| match entries.get(current) {
                Some(ExifRead::Found(img)) => {
                    let rows = field_rows(img);
                    if rows.is_empty() {
                        ui.label("Sin datos");
                    } else {
                        egui::Grid::new("info_grid").num_columns(2).show(ui, |ui| {
                            for (k, v) in rows {
                                ui.label(k);
                                ui.label(v);
                                ui.end_row();
                            }
                        });
                    }
                }
                Some(ExifRead::None) => {
                    ui.label("Sin metadatos EXIF");
                }
                Some(ExifRead::Error(_)) => {
                    ui.label("No se pudo leer los metadatos");
                }
                None => {
                    ui.label("Cargando…");
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ExifImage {
        ExifImage {
            make: Some("Nikon".into()),
            model: Some("D750".into()),
            fecha: Some("2020:01:02 03:04:05".into()),
            iso: Some(400),
            f_number: Some(Rational::new(28, 10)),
            shutter_speed: Some(Rational::new(1, 125)),
            focal_length: Some(Rational::new(50, 1)),
            orientacion: Some(6),
        }
    }

    #[test]
    fn row_is_omitted_when_field_absent() {
        let rows = field_rows(&ExifImage::default());
        assert!(rows.is_empty());
    }

    #[test]
    fn present_fields_become_rows() {
        let rows = field_rows(&sample());
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[0], ("Fabricante".into(), "Nikon".into()));
    }

    #[test]
    fn rational_formatting_is_human_readable() {
        assert_eq!(format_rational(Rational::new(50, 1), Fmt::Focal), "50 mm");
        assert_eq!(
            format_rational(Rational::new(28, 10), Fmt::Aperture),
            "f/2.8"
        );
        assert_eq!(
            format_rational(Rational::new(1, 125), Fmt::Shutter),
            "1/125 s"
        );
    }

    #[test]
    fn snapshot_field_rows() {
        insta::assert_debug_snapshot!(field_rows(&sample()));
    }
}
