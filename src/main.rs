//! Punto de entrada: inicialización de logging y arranque de `eframe`.

use std::path::PathBuf;

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

/// Extrae el primer argumento de línea de comandos como path de imagen.
///
/// - `"sh_images.exe"` → `None` (abre el diálogo como siempre).
/// - `"sh_images.exe C:\foto.png"` → `Some("C:\foto.png")`.
/// - Args vacías o whitespace → `None`.
pub fn parse_cli_path() -> Option<PathBuf> {
    std::env::args()
        .nth(1)
        .filter(|s| !s.is_empty())
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
}

/// Arranca la aplicación con una ventana de 1280x800.
fn main() -> eframe::Result<()> {
    init_logging();
    let initial_path = parse_cli_path();
    if let Some(ref p) = initial_path {
        tracing::info!(path = %p.display(), "opening image from CLI");
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Sh_Images",
        options,
        Box::new(|cc| Ok(Box::new(ShImagesApp::new(cc, initial_path)))),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn parse_path_from_argv() {
        // Simula el contracto de parse_cli_path: con un path válido en args[1].
        let sample = vec!["sh_images.exe".to_string(), "C:\\foto.png".to_string()];
        let parsed = sample
            .into_iter()
            .nth(1)
            .filter(|s| !s.is_empty())
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);
        assert_eq!(parsed.unwrap().to_string_lossy(), "C:\\foto.png");
    }

    #[test]
    fn empty_string_arg_is_filtered() {
        let sample = vec!["sh_images.exe".to_string(), "".to_string()];
        let parsed = sample
            .into_iter()
            .nth(1)
            .filter(|s| !s.is_empty())
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);
        assert!(parsed.is_none());
    }

    #[test]
    fn whitespace_arg_is_filtered() {
        let sample = vec!["sh_images.exe".to_string(), "   ".to_string()];
        let parsed = sample
            .into_iter()
            .nth(1)
            .filter(|s| !s.is_empty())
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);
        assert!(parsed.is_none());
    }
}
