//! Estado global de la aplicación y loop principal de `egui`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};

use eframe::egui;
use image::{DynamicImage, GenericImageView};

use crate::config::settings::Settings;
use crate::core::image_cache::ImageCache;
use crate::core::image_loader::load_image;
use crate::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use crate::core::preload::{preload_targets, PRELOAD_DEPTH};
use crate::core::thumbnail_cache::ThumbnailCache;
use crate::core::thumbnail_gen::{generate_thumbnail, THUMB_MAX};
use crate::core::view::{Vec2, ViewTransform};
use crate::ui::{sidebar::SidebarState, theme, toast::Toasts, viewer};
use crate::utils::errors::Result;
use crate::utils::paths::settings_path;

/// Evento enviado por un thread worker al UI thread.
///
/// La imagen decodificada NO viaja por el canal: el worker la inserta en el
/// cache y la UI la lee de ahí. El evento solo notifica el resultado del path.
struct LoadEvent {
    path: PathBuf,
    result: Result<()>,
}

/// Número de workers del pool de miniaturas (acotado, nunca un thread por imagen).
const THUMB_POOL_SIZE: usize = 3;

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
    /// Cache LRU de imágenes decodificadas, compartido con los workers.
    cache: Arc<ImageCache>,
    /// Paths con una carga en curso (deduplicación de workers).
    in_flight: Arc<Mutex<HashSet<PathBuf>>>,
    /// Emisor del canal único (clonado a cada worker).
    tx: mpsc::Sender<LoadEvent>,
    /// Receptor persistente del canal único.
    rx: Option<mpsc::Receiver<LoadEvent>>,
    toasts: Toasts,
    /// `true` si el usuario ha hecho zoom/pan con la imagen actual.
    user_interacted: bool,
    /// Último tamaño del canvas; se usa para re-fitear al redimensionar.
    last_viewport: Option<Vec2>,
    /// Último path aplicado a la textura; evita re-aplicar un evento duplicado.
    last_applied: Option<PathBuf>,
    /// Cache en memoria de miniaturas, compartido con el pool de workers.
    thumb_cache: Arc<ThumbnailCache>,
    /// Emisor del canal de paths a miniaturizar (la UI encola, los workers consumen).
    thumb_tx: mpsc::Sender<PathBuf>,
    /// Receptor de notificaciones de "miniatura lista" (solo dispara repaint).
    thumb_events_rx: Option<mpsc::Receiver<()>>,
    /// Estado del sidebar (visible + texturas GPU).
    sidebar: SidebarState,
    /// Generación de la carpeta abierta; los workers descartan miniaturas de
    /// generaciones anteriores (para no rellenar el cache tras `clear`).
    thumb_epoch: Arc<AtomicU64>,
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
        let cache = Arc::new(ImageCache::new(settings.cache_memory_limit_mb));
        let (tx, rx) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        let thumb_cache = Arc::new(ThumbnailCache::new());
        let (thumb_tx, thumb_rx) = mpsc::channel::<PathBuf>();
        let thumb_rx = Arc::new(Mutex::new(thumb_rx));
        let thumb_epoch = Arc::new(AtomicU64::new(0));
        let (thumb_events_tx, thumb_events_rx) = mpsc::channel::<()>();
        for _ in 0..THUMB_POOL_SIZE {
            let rx = thumb_rx.clone();
            let cache = thumb_cache.clone();
            let events_tx = thumb_events_tx.clone();
            let epoch = thumb_epoch.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || loop {
                let path = rx.lock().unwrap_or_else(|p| p.into_inner()).recv();
                let Ok(path) = path else { break };
                let start_epoch = epoch.load(Ordering::Relaxed);
                let image = load_image(&path);
                if epoch.load(Ordering::Relaxed) != start_epoch {
                    continue;
                }
                match image {
                    Ok(image) => {
                        let thumb = generate_thumbnail(&image, THUMB_MAX);
                        cache.insert(path.clone(), thumb);
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, path = %path.display(), "thumbnail failed");
                    }
                }
                if events_tx.send(()).is_err() {
                    tracing::debug!("thumbnail event dropped (receiver gone)");
                }
                ctx.request_repaint();
            });
        }
        Self {
            settings,
            ctx,
            navigation: None,
            transform: ViewTransform::new(Vec2::ZERO, Vec2::ZERO),
            texture: None,
            cache,
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            tx,
            rx: Some(rx),
            toasts: Toasts::new(),
            user_interacted: false,
            last_viewport: None,
            last_applied: None,
            thumb_cache,
            thumb_tx,
            thumb_events_rx: Some(thumb_events_rx),
            sidebar: SidebarState::new(),
            thumb_epoch,
        }
    }

    /// Carga la configuración y devuelve un error tipado si falla.
    ///
    /// Expuesta para que los tests de integración puedan verificar el ciclo
    /// de vida completo sin arrancar una ventana.
    pub fn load_settings() -> Result<Settings> {
        settings_path().and_then(|path| Settings::load(&path))
    }

    /// Guard del set de paths en carga, recuperándose de un lock envenenado.
    fn in_flight_guard(&self) -> MutexGuard<'_, HashSet<PathBuf>> {
        self.in_flight.lock().unwrap_or_else(|p| p.into_inner())
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
                self.thumb_epoch.fetch_add(1, Ordering::Relaxed);
                self.thumb_cache.clear();
                self.sidebar.clear_textures();
                for image_path in &nav.images {
                    if self.thumb_tx.send(image_path.clone()).is_err() {
                        tracing::debug!("thumbnail queue closed; workers gone");
                        break;
                    }
                }
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

    /// Carga `path` de la forma más rápida posible:
    ///
    /// 1. Cache hit → textura inmediata + pre-carga (sin thread).
    /// 2. In-flight → no-op (un worker ya lo está cargando).
    /// 3. Miss → spawn worker que decodifica, cachea y notifica.
    fn start_load(&mut self, path: PathBuf) {
        if let Some((texture, image_size)) = self.texture_from_cache(&path) {
            tracing::info!(path = %path.display(), "image loaded from cache");
            self.apply_decoded(&path, texture, image_size);
            return;
        }
        if self.in_flight_guard().contains(&path) {
            tracing::debug!(path = %path.display(), "load already in flight");
            return;
        }
        self.spawn_load(path, false);
    }

    /// Construye la textura desde el cache si `path` está presente.
    ///
    /// Devuelve `(textura, tamaño de imagen)` con el guard del cache ya soltado
    /// (la `CacheEntryRef` se cae al final de la llamada), para que el caller
    /// pueda mutar `self` libremente después.
    fn texture_from_cache(&self, path: &std::path::Path) -> Option<(egui::TextureHandle, Vec2)> {
        let entry = self.cache.get(path)?;
        let texture = make_texture(&self.ctx, &entry);
        let size = entry.dimensions();
        Some((texture, Vec2::new(size.0 as f32, size.1 as f32)))
    }

    /// Aplica una imagen decodificada al estado: textura, transform en fit y
    /// dispara la pre-carga de N±1.
    ///
    /// Marca `path` como el último aplicado para que `poll_loader` pueda
    /// descartar un evento Ok duplicado (e.g. pre-carga de N+1 que llegó tarde
    /// cuando N+1 ya es la imagen actual) sin pisar el estado del usuario.
    fn apply_decoded(
        &mut self,
        path: &std::path::Path,
        texture: egui::TextureHandle,
        image_size: Vec2,
    ) {
        self.last_applied = Some(path.to_path_buf());
        self.texture = Some(texture);
        self.transform = ViewTransform::new(image_size, Vec2::ZERO);
        self.user_interacted = false;
        self.last_viewport = None;
        self.preload_neighbors();
    }

    /// Spawnea un worker que decodifica `path`, lo inserta en el cache y envía
    /// un evento ligero por el canal único.
    ///
    /// `is_preload` solo cambia el nivel de log (DEBUG vs INFO): la lógica del
    /// worker es idéntica. El flag no genera toasts de error — eso lo decide el
    /// check `is_current` en `poll_loader`.
    fn spawn_load(&self, path: PathBuf, is_preload: bool) {
        if is_preload {
            tracing::debug!(path = %path.display(), "preloading image");
        } else {
            tracing::info!(path = %path.display(), "loading image");
        }
        self.in_flight_guard().insert(path.clone());
        let tx = self.tx.clone();
        let cache = self.cache.clone();
        let in_flight = self.in_flight.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = load_image(&path).map(|image| {
                cache.insert(path.clone(), image);
            });
            in_flight
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&path);
            if tx.send(LoadEvent { path, result }).is_err() {
                tracing::debug!("load event dropped (receiver gone)");
            }
            ctx.request_repaint();
        });
    }

    /// Drena el canal único; solo actúa sobre el path actual.
    ///
    /// Los eventos de pre-carga obsoletos (path distinto del actual) se ignoran
    /// silenciosamente: el único efecto que tenían era poblar el cache.
    fn poll_loader(&mut self, t: f64) {
        let Some(rx) = self.rx.take() else { return };
        while let Ok(event) = rx.try_recv() {
            let is_current = self
                .navigation
                .as_ref()
                .and_then(|n| n.current_path())
                .map(|p| p == &event.path)
                .unwrap_or(false);
            if !is_current {
                tracing::debug!(path = %event.path.display(), "ignoring non-current load result");
                continue;
            }
            match event.result {
                Ok(()) => {
                    if self.last_applied.as_ref() == Some(&event.path) {
                        tracing::debug!(path = %event.path.display(), "event already applied; skipping");
                        continue;
                    }
                    tracing::info!(path = %event.path.display(), "image decoded");
                    if let Some((texture, image_size)) = self.texture_from_cache(&event.path) {
                        self.apply_decoded(&event.path, texture, image_size);
                    } else {
                        tracing::warn!(
                            path = %event.path.display(),
                            limit_mb = self.cache.memory_limit_mb(),
                            "decoded image not in cache (exceeds limit or evicted)"
                        );
                        self.toasts.push(
                            format!(
                                "La imagen excede el límite de memoria del cache ({} MiB) y no se pudo mostrar",
                                self.cache.memory_limit_mb()
                            ),
                            t,
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, path = %event.path.display(), "failed to load image");
                    self.toasts.push(format!("No se pudo abrir: {e}"), t);
                }
            }
        }
        self.rx = Some(rx);
    }

    /// Drena las notificaciones de miniaturas y dispara un repaint si hubo.
    ///
    /// La UI no necesita el contenido del evento: lee `thumb_cache` directamente
    /// en el frame siguiente.
    fn poll_thumbnails(&mut self) {
        let Some(rx) = self.thumb_events_rx.take() else {
            return;
        };
        let mut repaint = false;
        while rx.try_recv().is_ok() {
            repaint = true;
        }
        self.thumb_events_rx = Some(rx);
        if repaint {
            self.ctx.request_repaint();
        }
    }

    /// Salta a la imagen `index` de la carpeta (click en una miniatura).
    fn navigate_to(&mut self, index: usize) {
        let Some(nav) = &mut self.navigation else {
            return;
        };
        if index >= nav.images.len() {
            return;
        }
        nav.current = index;
        if let Some(path) = nav.current_path().cloned() {
            self.start_load(path);
        }
    }

    /// Alterna la visibilidad del sidebar.
    fn toggle_sidebar(&mut self) {
        self.sidebar.show = !self.sidebar.show;
    }

    /// Dispara la pre-carga de N±1 usando `preload_targets`.
    fn preload_neighbors(&self) {
        let Some(nav) = &self.navigation else { return };
        let targets = preload_targets(
            nav,
            PRELOAD_DEPTH,
            |p| self.cache.contains(p),
            |p| self.in_flight_guard().contains(p),
        );
        for path in targets {
            self.spawn_load(path, true);
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
        let toggle_side = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::H));
        if toggle_side {
            self.toggle_sidebar();
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

        self.poll_loader(t);
        self.poll_thumbnails();

        if self.sidebar.show {
            if let Some(nav) = &self.navigation {
                let selected = self.sidebar.show(ui, nav, &self.thumb_cache);
                if let Some(index) = selected {
                    self.navigate_to(index);
                }
            }
        }

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
                    // Auto-fit: al cargar (viewport recién conocido) y al
                    // redimensionar mientras el usuario no haya interactuado.
                    let viewport = self.transform.viewport;
                    if !self.user_interacted && self.last_viewport != Some(viewport) {
                        self.transform.fit();
                        self.last_viewport = Some(viewport);
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
