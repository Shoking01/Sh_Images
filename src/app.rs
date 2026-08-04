//! Estado global de la aplicación y loop principal de `egui`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::Instant;

use crate::core::exif::{read_exif, ExifRead};

use eframe::egui;
use image::DynamicImage;

use crate::config::settings::Settings;
use crate::core::actions::Action;
use crate::core::image_cache::ImageCache;
use crate::core::image_loader::load_image;
use crate::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use crate::core::preload::{preload_targets, PRELOAD_DEPTH};
use crate::core::shortcuts::ShortcutMap;
use crate::core::thumb_queue::ThumbQueue;
use crate::core::thumbnail_cache::ThumbnailCache;
use crate::core::thumbnail_gen::{generate_thumbnail, THUMB_MAX};
use crate::core::view::{Vec2, ViewTransform};
use crate::ui::{
    info_panel::{self, InfoPanelState},
    shortcut_dialog::ShortcutDialog,
    sidebar::SidebarState,
    statusbar,
    statusbar::StatusInfo,
    theme,
    toast::Toasts,
    toolbar, viewer,
};
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

/// Estado de reproducción del GIF actual (None si la imagen es estática).
struct AnimState {
    started: Instant,
    current_frame: usize,
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
    /// `true` si el usuario ha hecho zoom con la imagen actual.
    user_interacted: bool,
    /// Último tamaño del canvas; se usa para re-fitear al redimensionar.
    last_viewport: Option<Vec2>,
    /// Último path aplicado a la textura; evita re-aplicar un evento duplicado.
    last_applied: Option<PathBuf>,
    /// Cache en memoria de miniaturas, compartido con el pool de workers.
    thumb_cache: Arc<ThumbnailCache>,
    /// Cola FIFO de paths a miniaturizar (la UI encola, los workers consumen).
    thumb_queue: ThumbQueue,
    /// Receptor de notificaciones de "miniatura lista" (solo dispara repaint).
    thumb_events_rx: Option<mpsc::Receiver<()>>,
    /// Estado del sidebar (visible + texturas GPU).
    sidebar: SidebarState,
    /// Generación de la carpeta abierta; los workers descartan miniaturas de
    /// generaciones anteriores (para no rellenar el cache tras `clear`).
    thumb_epoch: Arc<AtomicU64>,
    /// Atajos de teclado configurables (desde settings, editables en UI).
    shortcuts: ShortcutMap,
    /// Si la ventana está en pantalla completa.
    is_fullscreen: bool,
    /// Dialog de configuración de atajos.
    shortcut_dialog: ShortcutDialog,
    /// Tamaño en disco cacheado por path (evita `fs::metadata` por frame).
    size_for: Option<(PathBuf, u64)>,
    /// Cache de EXIF por path (se limpia al abrir una carpeta).
    exif_cache: Arc<Mutex<HashMap<PathBuf, ExifRead>>>,
    /// Emisor de peticiones de EXIF (UI → worker).
    exif_tx: mpsc::Sender<PathBuf>,
    /// Receptor de avisos "EXIF listo" (solo dispara repaint).
    exif_rx: Option<mpsc::Receiver<()>>,
    /// Estado del panel derecho de info.
    info_panel: InfoPanelState,
    /// Estado de reproducción de la animación del GIF actual.
    anim: Option<AnimState>,
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
        let thumb_queue = ThumbQueue::new();
        let thumb_epoch = Arc::new(AtomicU64::new(0));
        let (thumb_events_tx, thumb_events_rx) = mpsc::channel::<()>();
        for _ in 0..THUMB_POOL_SIZE {
            let queue = thumb_queue.clone();
            let cache = thumb_cache.clone();
            let events_tx = thumb_events_tx.clone();
            let epoch = thumb_epoch.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                while let Some(path) = queue.pop() {
                    let start_epoch = epoch.load(Ordering::Relaxed);
                    let image = load_image(&path);
                    if epoch.load(Ordering::Relaxed) != start_epoch {
                        continue;
                    }
                    match image {
                        Ok(image) => {
                            let thumb = generate_thumbnail(image.first_frame(), THUMB_MAX);
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
                }
            });
        }
        let (exif_tx, exif_rx) = mpsc::channel::<PathBuf>();
        let (exif_events_tx, exif_events_rx) = mpsc::channel::<()>();
        let exif_cache = Arc::new(Mutex::new(HashMap::new()));
        {
            let cache = exif_cache.clone();
            let events_tx = exif_events_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                while let Ok(path) = exif_rx.recv() {
                    let result = match read_exif(&path) {
                        Ok(Some(img)) => ExifRead::Found(img),
                        Ok(None) => ExifRead::None,
                        Err(e) => ExifRead::Error(e),
                    };
                    if let Ok(mut m) = cache.lock() {
                        m.insert(path, result);
                    }
                    if events_tx.send(()).is_err() {
                        tracing::debug!("exif event dropped (receiver gone)");
                    }
                    ctx.request_repaint();
                }
            });
        }
        Self {
            shortcuts: settings.shortcuts.clone(),
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
            thumb_queue,
            thumb_events_rx: Some(thumb_events_rx),
            sidebar: SidebarState::new(),
            thumb_epoch,
            is_fullscreen: false,
            shortcut_dialog: ShortcutDialog::default(),
            size_for: None,
            exif_cache,
            exif_tx,
            exif_rx: Some(exif_events_rx),
            info_panel: InfoPanelState::default(),
            anim: None,
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
                // Descarta los paths de la carpeta anterior aún en cola: el epoch
                // cubre a los workers que decodifican en vuelo, pero sin este
                // drenado cada worker consumiría (y descartaría) un path obsoleto
                // antes de llegar a los nuevos.
                self.thumb_queue.drain();
                self.exif_cache
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clear();
                for image_path in &nav.images {
                    self.thumb_queue.push(image_path.clone());
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
        let texture = make_texture(&self.ctx, entry.first_frame());
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
        self.request_exif(path);
        let animated = self
            .cache
            .get(path)
            .map(|e| e.is_animated())
            .unwrap_or(false);
        self.anim = if animated {
            Some(AnimState {
                started: Instant::now(),
                current_frame: 0,
            })
        } else {
            None
        };
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
                cache.insert_loaded(path.clone(), image);
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

    /// Encolar una petición de EXIF para `path` si aún no está cacheado.
    fn request_exif(&self, path: &Path) {
        let present = self
            .exif_cache
            .lock()
            .map(|m| m.contains_key(path))
            .unwrap_or(false);
        if present {
            return;
        }
        let _ = self.exif_tx.send(path.to_path_buf());
    }

    /// Drena las señales "EXIF listo" y hace repaint para repintar el panel.
    fn poll_exif(&mut self) {
        let Some(rx) = self.exif_rx.take() else {
            return;
        };
        let mut repaint = false;
        while rx.try_recv().is_ok() {
            repaint = true;
        }
        self.exif_rx = Some(rx);
        if repaint {
            self.ctx.request_repaint();
        }
    }

    /// Avanza el GIF actual: reconstruye la textura cuando cambia el frame
    /// activo y programa el repaint para el próximo cambio.
    fn tick_animation(&mut self) {
        let Some(anim) = self.anim.as_mut() else {
            return;
        };
        let Some(path) = self
            .navigation
            .as_ref()
            .and_then(|n| n.current_path())
            .cloned()
        else {
            return;
        };
        let Some(entry) = self.cache.get(&path) else {
            return;
        };
        if !entry.is_animated() {
            return;
        }
        let elapsed = anim.started.elapsed();
        let idx = entry.frame_index_at(elapsed);
        if idx != anim.current_frame {
            self.texture = Some(make_texture(&self.ctx, entry.frame_at(elapsed)));
            anim.current_frame = idx;
        }
        let wait = entry.time_to_next_frame(elapsed);
        self.ctx.request_repaint_after(wait);
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

    /// Atajos de teclado configurables vía `ShortcutMap`.
    ///
    /// Esc siempre sale del fullscreen (comportamiento de sistema, no remapeable).
    /// Si el dialog de atajos está abierto o hay foco de texto, no se disparan.
    fn handle_shortcuts(&mut self, ui: &mut egui::Ui) {
        if self.is_fullscreen
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.dispatch(Action::Fullscreen);
            return;
        }
        if self.shortcut_dialog.open || ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let binding = ui.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => crate::ui::shortcut_dialog::keybinding_from_egui(*key, *modifiers),
                _ => None,
            })
        });
        if let Some(action) = binding.and_then(|b| self.shortcuts.action_for(b)) {
            self.dispatch(action);
        }
    }

    /// Ejecuta una acción: único punto por el que toolbar, menú y atajos
    /// disparan efectos en la app.
    fn dispatch(&mut self, action: Action) {
        match action {
            Action::Open => self.open_dialog(),
            Action::Prev => self.navigate(-1),
            Action::Next => self.navigate(1),
            Action::RotateCw => self.rotate_image(true),
            Action::RotateCcw => self.rotate_image(false),
            Action::Fit => {
                if self.texture.is_some() {
                    self.transform.fit();
                    self.user_interacted = false;
                }
            }
            Action::Fullscreen => self.toggle_fullscreen(),
            Action::ToggleTheme => self.toggle_theme(),
            Action::ToggleSidebar => self.toggle_sidebar(),
            Action::ToggleInfo => self.info_panel.show = !self.info_panel.show,
            Action::ToggleSlideshow => {}
            Action::SlideshowFaster => {}
            Action::SlideshowSlower => {}
            Action::EditShortcuts => self.shortcut_dialog.open = true,
        }
    }

    /// Rota la imagen actual 90° (CW si `cw`, CCW si no) y re-aplica fit.
    fn rotate_image(&mut self, cw: bool) {
        if self.texture.is_none() {
            return;
        }
        if cw {
            self.transform.rotate_cw();
        } else {
            self.transform.rotate_ccw();
        }
        self.user_interacted = false;
    }

    /// Alterna el fullscreen nativo del viewport.
    fn toggle_fullscreen(&mut self) {
        self.is_fullscreen = !self.is_fullscreen;
        self.ctx
            .send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
        tracing::info!(fullscreen = self.is_fullscreen, "toggled fullscreen");
    }

    /// Alterna el tema y lo persiste en disco.
    fn toggle_theme(&mut self) {
        self.settings.theme = theme::toggle(&self.settings.theme).to_string();
        if let Ok(path) = settings_path() {
            if let Err(e) = self.settings.save(&path) {
                tracing::warn!(error = %e, "failed to persist theme");
            }
        }
        tracing::info!(theme = %self.settings.theme, "theme toggled");
    }

    /// Construye la info de la imagen actual para la status bar.
    fn current_status_info(&mut self) -> Option<StatusInfo> {
        let nav = self.navigation.as_ref()?;
        let path = nav.current_path()?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        let size_bytes = match &self.size_for {
            Some((p, n)) if p == path => Some(*n),
            _ => {
                let n = std::fs::metadata(path).ok().map(|m| m.len());
                self.size_for = Some((path.clone(), n.unwrap_or(0)));
                Some(n.unwrap_or(0))
            }
        };
        Some(StatusInfo {
            name,
            width: self.transform.image_size.x as u32,
            height: self.transform.image_size.y as u32,
            size_bytes,
            index: nav.current + 1,
            total: nav.images.len(),
        })
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
        self.poll_exif();
        self.tick_animation();

        // Toolbar superior (acciones de la app).
        let action = toolbar::show(
            ui,
            &self.shortcuts,
            &self.settings.theme,
            self.is_fullscreen,
            false,
        );
        if let Some(action) = action {
            self.dispatch(action);
        }

        // Status bar inferior (info de la imagen actual).
        if self.texture.is_some() {
            if let Some(info) = self.current_status_info() {
                statusbar::show(ui, &info);
            }
        }

        if self.sidebar.show {
            if let Some(nav) = &self.navigation {
                let selected = self.sidebar.show(ui, nav, &self.thumb_cache);
                if let Some(index) = selected {
                    self.navigate_to(index);
                }
            }
        }

        if self.texture.is_some() && self.info_panel.show {
            if let Some(nav) = &self.navigation {
                if let Some(path) = nav.current_path() {
                    info_panel::show(ui, &self.exif_cache, path);
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
                ui.menu_button("Ver", |ui| {
                    if ui.button(Action::ToggleSidebar.label()).clicked() {
                        ui.close();
                        self.dispatch(Action::ToggleSidebar);
                    }
                    if ui.button(Action::Fullscreen.label()).clicked() {
                        ui.close();
                        self.dispatch(Action::Fullscreen);
                    }
                });
                ui.menu_button("Ayuda", |ui| {
                    if ui.button(Action::EditShortcuts.label()).clicked() {
                        ui.close();
                        self.dispatch(Action::EditShortcuts);
                    }
                });
            });
            if want_open {
                self.open_dialog();
            }

            match &self.texture {
                Some(texture) => {
                    let resp = viewer::show(ui, texture, &mut self.transform);
                    if resp.zoomed {
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

        if self.shortcut_dialog.open {
            let changed = self.shortcut_dialog.show(ui, &mut self.shortcuts);
            if changed {
                if let Ok(path) = settings_path() {
                    if let Err(e) = self.settings.save(&path) {
                        tracing::warn!(error = %e, "failed to persist shortcuts");
                    }
                }
            }
        }

        self.handle_shortcuts(ui);
    }
}
