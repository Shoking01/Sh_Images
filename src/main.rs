//! Punto de entrada: inicialización de logging y arranque de `eframe`.

use eframe::egui;
use sh_images::app::ShImagesApp;

/// Inicializa el logging estructurado (`tracing`).
///
/// Nivel `DEBUG` en builds de debug, `INFO` en release (AGENTS.md §7.3).
fn init_logging() {
    let level = if cfg!(debug_assertions) {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt().with_max_level(level).init();
}

/// Arranca la aplicación con una ventana de 1280x800.
fn main() -> eframe::Result<()> {
    init_logging();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Sh_Images",
        options,
        Box::new(|cc| Ok(Box::new(ShImagesApp::new(cc)))),
    )
}
