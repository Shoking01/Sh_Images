//! Entry point: logging initialization and `eframe` startup.
//!
//! On Windows, `windows_subsystem = "windows"` removes the console from the .exe.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;

use eframe::egui;
use sh_images::app::ShImagesApp;

/// Initializes structured logging (`tracing`).
fn init_logging() {
    let level = if cfg!(debug_assertions) {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt().with_max_level(level).init();
}

/// Extracts the first CLI argument as an image path.
pub fn parse_cli_path() -> Option<PathBuf> {
    std::env::args()
        .nth(1)
        .filter(|s| !s.is_empty())
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
}

/// Starts the app with a 1280x800 window.
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
