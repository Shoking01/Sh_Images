//! Estado global de la aplicación y loop principal de `egui`.

use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui;
use image::{DynamicImage, GenericImageView};

use crate::config::settings::Settings;
use crate::core::image_loader::load_image;
use crate::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use crate::core::view::{Vec2, ViewTransform};
use crate::ui::{theme, toast::Toasts, viewer};
use crate::utils::errors::Result;
use crate::utils::paths::settings_path;

/// Evento enviado por el thread worker al UI thread.
struct LoadEvent {
    path: PathBuf,
    result: Result<DynamicImage>,
}

/// Estado global de la aplicación, creado una vez al arrancar.
///
/// `eframe` invoca [`eframe::App::ui`] en cada frame.
pub struct ShImagesApp {
    settings: Settings,
    /// Contexto de egui, clonado para `request_repaint` desde workers.
    ctx: egui::Context,
    navigation: Option<Navigation>,
    transform: ViewTransform,
    texture: Option<egui::TextureHandle>,
    rx: Option<mpsc::Receiver<LoadEvent>>,
    toasts: Toasts,
    /// `true` si el usuario ha hecho zoom/pan con la imagen actual.
    user_interacted: bool,
}

impl ShImagesApp {
    /// Crea el estado de la app cargando la configuración del usuario.
    ///
    /// Si la configuración no puede cargarse, se usan los defaults y se loguea
    /// un warning; la app nunca aborta el arranque por esto.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = match settings_path().and_then(|path| Settings::load(&path)) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::warn!(error = %e, "failed to load settings; using defaults");
                Settings::default()
            }
        };
        Self {
            settings,
            ctx: cc.egui_ctx.clone(),
            navigation: None,
            transform: ViewTransform::new(Vec2::ZERO, Vec2::ZERO),
            texture: None,
            rx: None,
            toasts: Toasts::new(),
            user_interacted: false,
        }
    }

    /// Carga la configuración y devuelve un error tipado si falla.
    ///
    /// Expuesta para que los tests de integración puedan verificar el ciclo
    /// de vida completo sin arrancar una ventana.
    pub fn load_settings() -> Result<Settings> {
        settings_path().and_then(|path| Settings::load(&path))
    }

    /// Abre el diálogo nativo y, si hay elección, carga la imagen.
    ///
    /// El tiempo de egui se re-lee tras el diálogo (que es bloqueante): usar
    /// el tiempo del frame en que se abrió haría que un toast emitido ahora
    /// expirara al instante si el diálogo estuvo abierto más de 3 segundos.
    fn open_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Imágenes", SUPPORTED_EXTENSIONS)
            .pick_file();
        if let Some(path) = picked {
            let t = self.ctx.input(|i| i.time);
            self.open_path(path, t);
        }
    }

    /// Abre `path`: construye la navegación de su carpeta y dispara la carga.
    fn open_path(&mut self, path: PathBuf, t: f64) {
        match Navigation::from_folder(&path, SUPPORTED_EXTENSIONS) {
            Ok(nav) => {
                tracing::info!(path = %path.display(), "opening image");
                self.navigation = Some(nav);
                self.start_load(path);
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "failed to scan folder");
                self.toasts
                    .push(format!("No se pudo leer la carpeta: {e}"), t);
            }
        }
    }

    /// Dispara un thread worker que carga `path` y envía el resultado por canal.
    fn start_load(&mut self, path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = load_image(&path);
            let event = LoadEvent { path, result };
            if tx.send(event).is_err() {
                tracing::debug!("load event dropped (receiver gone)");
            }
            ctx.request_repaint();
        });
    }

    /// Cada frame, recoge el resultado del worker si está listo.
    fn poll_loader(&mut self, _ui: &mut egui::Ui, t: f64) {
        let Some(rx) = &self.rx else { return };
        let Ok(event) = rx.try_recv() else { return };
        self.rx = None;

        // Descarta resultados de navegaciones obsoletas.
        let is_current = self
            .navigation
            .as_ref()
            .and_then(|n| n.current_path())
            .map(|p| p == &event.path)
            .unwrap_or(false);
        if !is_current {
            tracing::debug!(path = %event.path.display(), "ignoring stale load result");
            return;
        }

        match event.result {
            Ok(image) => {
                tracing::info!(path = %event.path.display(), "image decoded");
                let size = image.dimensions();
                self.texture = Some(make_texture(&self.ctx, &image));
                self.transform =
                    ViewTransform::new(Vec2::new(size.0 as f32, size.1 as f32), Vec2::ZERO);
                self.user_interacted = false;
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %event.path.display(), "failed to load image");
                self.toasts.push(format!("No se pudo abrir: {e}"), t);
            }
        }
    }

    /// Navega `dir` pasos (-1 prev, +1 next) y carga la nueva imagen.
    fn navigate(&mut self, dir: isize) {
        let Some(nav) = &mut self.navigation else {
            return;
        };
        if dir > 0 {
            nav.next();
        } else {
            nav.prev();
        }
        if let Some(path) = nav.current_path().cloned() {
            self.start_load(path);
        }
    }

    /// Atajos de teclado: Ctrl+O abre, ←→ navega, F re-ajusta a fit.
    fn handle_shortcuts(&mut self, ui: &mut egui::Ui) {
        let open = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O));
        if open {
            self.open_dialog();
        }
        let next = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight));
        if next {
            self.navigate(1);
        }
        let prev = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft));
        if prev {
            self.navigate(-1);
        }
        let fit = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F));
        if fit && self.texture.is_some() {
            self.transform.fit();
            self.user_interacted = false;
        }
    }
}

/// Convierte una imagen decodificada en textura de egui.
fn make_texture(ctx: &egui::Context, image: &DynamicImage) -> egui::TextureHandle {
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.to_rgba8();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    ctx.load_texture("image", color_image, egui::TextureOptions::LINEAR)
}

impl eframe::App for ShImagesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::apply(ui.ctx(), &self.settings.theme);
        let t = ui.input(|i| i.time);

        self.poll_loader(ui, t);

        let mut want_open = false;
        egui::CentralPanel::default().show(ui, |ui| {
            egui::menu::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Archivo", |ui| {
                    if ui.button("Abrir…").clicked() {
                        ui.close();
                        want_open = true;
                    }
                });
            });
            if want_open {
                self.open_dialog();
            }

            match &self.texture {
                Some(texture) => {
                    let resp = viewer::show(ui, texture, &mut self.transform);
                    if resp.zoomed || resp.panned {
                        self.user_interacted = true;
                    }
                }
                None => {
                    ui.centered_and_justified(|ui| {
                        ui.heading("Sh_Images");
                        ui.label("Archivo → Abrir… o Ctrl+O");
                    });
                }
            }
        });

        self.toasts.update(t);
        self.toasts.show(ui);

        self.handle_shortcuts(ui);
    }
}
