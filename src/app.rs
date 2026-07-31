//! Estado global de la aplicación y loop principal de `egui`.

use eframe::egui;

use crate::config::settings::Settings;
use crate::ui::theme;
use crate::utils::errors::Result;
use crate::utils::paths::settings_path;

/// Estado global de la aplicación, reconstruido en cada frame por `eframe`.
pub struct ShImagesApp {
    settings: Settings,
}

impl ShImagesApp {
    /// Crea el estado de la app cargando la configuración del usuario.
    ///
    /// Si la configuración no puede cargarse, se usan los defaults y se loguea
    /// un warning; la app nunca aborta el arranque por esto.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let settings = match settings_path().and_then(|path| Settings::load(&path)) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(error = %e, "failed to load settings; using defaults");
                Settings::default()
            }
        };
        Self { settings }
    }

    /// Carga la configuración y devuelve un error tipado si falla.
    ///
    /// Expuesta para que los tests de integración puedan verificar el ciclo
    /// de vida completo sin arrancar una ventana.
    pub fn load_settings() -> Result<Settings> {
        settings_path().and_then(|path| Settings::load(&path))
    }
}

impl eframe::App for ShImagesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::apply(ui.ctx(), &self.settings.theme);
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.heading("Sh_Images");
            });
        });
    }
}
