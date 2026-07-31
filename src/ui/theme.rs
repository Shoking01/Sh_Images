//! Aplicación de temas visuales a `egui`.

use egui;

/// Aplica el tema `name` (`"dark"` o `"light"`) al contexto de `egui`.
///
/// Cualquier otro valor se ignora y se deja el tema por defecto de `egui`.
pub fn apply(ctx: &egui::Context, name: &str) {
    match name {
        "dark" => ctx.set_visuals(egui::Visuals::dark()),
        "light" => ctx.set_visuals(egui::Visuals::light()),
        _ => {}
    }
}
