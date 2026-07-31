//! Aplicación de temas visuales a `egui`.

use egui;

/// Aplica el tema `name` (`"dark"` o `"light"`) al contexto de `egui`.
///
/// Cualquier otro valor no modifica el tema actual.
pub fn apply(ctx: &egui::Context, name: &str) {
    match name {
        "dark" => ctx.set_visuals(egui::Visuals::dark()),
        "light" => ctx.set_visuals(egui::Visuals::light()),
        _ => {
            tracing::warn!(theme = %name, "unknown theme; keeping current visuals");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_dark_sets_dark_mode() {
        let ctx = egui::Context::default();
        apply(&ctx, "dark");
        assert!(ctx.global_style().visuals.dark_mode);
    }

    #[test]
    fn apply_light_sets_light_mode() {
        let ctx = egui::Context::default();
        apply(&ctx, "light");
        assert!(!ctx.global_style().visuals.dark_mode);
    }

    #[test]
    fn apply_unknown_name_leaves_visuals_unchanged() {
        let ctx = egui::Context::default();
        apply(&ctx, "unknown");
        assert!(ctx.global_style().visuals.dark_mode);
    }
}
