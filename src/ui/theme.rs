use egui;
pub fn apply(ctx: &egui::Context, name: &str) {
    match name {
        "dark" => ctx.set_visuals(egui::Visuals::dark()),
        "light" => ctx.set_visuals(egui::Visuals::light()),
        _ => {
            tracing::warn!(theme = %name, "unknown theme; keeping current visuals");
        }
    }
}
pub fn toggle(name: &str) -> &'static str {
    match name {
        "dark" => "light",
        "light" => "dark",
        other => {
            tracing::warn!(theme = %other, "unknown theme on toggle; falling back to dark");
            "dark"
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

    #[test]
    fn toggle_dark_to_light() {
        assert_eq!(toggle("dark"), "light");
    }

    #[test]
    fn toggle_light_to_dark() {
        assert_eq!(toggle("light"), "dark");
    }

    #[test]
    fn toggle_unknown_falls_back_to_dark() {
        assert_eq!(toggle("solarized"), "dark");
    }
}
