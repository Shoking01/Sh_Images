use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::core::exif::{read_exif, ExifRead};

use eframe::egui;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageFormat;

use crate::config::settings::Settings;
use crate::core::actions::Action;
use crate::core::edit_state::EditState;
use crate::core::editor::{apply_all, apply_crop};
use crate::core::image_cache::ImageCache;
use crate::core::image_loader::load_image;
use crate::core::lang::Language;
use crate::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use crate::core::preload::{preload_targets, PRELOAD_DEPTH};
use crate::core::shortcuts::ShortcutMap;
use crate::core::slideshow;
use crate::core::thumb_queue::ThumbQueue;
use crate::core::thumbnail_cache::ThumbnailCache;
use crate::core::thumbnail_gen::{generate_thumbnail_capped, THUMB_MAX};
use crate::core::view::{Vec2, ViewTransform};
use crate::ui::{
    editor::{self},
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
struct LoadEvent {
    path: PathBuf,
    result: Result<()>,
}

struct AnimState {
    started: Instant,
    current_frame: usize,
}

const THUMB_POOL_SIZE: usize = 3;
pub struct ShImagesApp {
    settings: Settings,
    ctx: egui::Context,
    navigation: Option<Navigation>,
    transform: ViewTransform,
    texture: Option<egui::TextureHandle>,
    cache: Arc<ImageCache>,
    in_flight: Arc<Mutex<HashSet<PathBuf>>>,
    tx: mpsc::Sender<LoadEvent>,
    rx: Option<mpsc::Receiver<LoadEvent>>,
    toasts: Toasts,
    user_interacted: bool,
    last_viewport: Option<Vec2>,
    last_applied: Option<PathBuf>,
    thumb_cache: Arc<ThumbnailCache>,
    thumb_queue: ThumbQueue,
    thumb_events_rx: Option<mpsc::Receiver<()>>,
    sidebar: SidebarState,
    thumb_epoch: Arc<AtomicU64>,
    shortcuts: ShortcutMap,
    is_fullscreen: bool,
    shortcut_dialog: ShortcutDialog,
    size_for: Option<(PathBuf, u64)>,
    exif_cache: Arc<Mutex<HashMap<PathBuf, ExifRead>>>,
    exif_tx: mpsc::Sender<PathBuf>,
    exif_rx: Option<mpsc::Receiver<()>>,
    info_panel: InfoPanelState,
    anim: Option<AnimState>,
    slideshow_active: bool,
    slideshow_interval: Duration,
    slideshow_last_advance: Instant,
    pending_initial: Option<PathBuf>,
    default_viewer_dialog: bool,
    edit_state: Option<EditState>,
    edit_source_path: Option<PathBuf>,
    edit_texture: Option<egui::TextureHandle>,
}

impl ShImagesApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
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
                            let thumb = generate_thumbnail_capped(image.first_frame(), THUMB_MAX);
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
        let slideshow_interval = Duration::from_secs(settings.slideshow_interval_secs.max(1));
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
            slideshow_active: false,
            slideshow_interval,
            slideshow_last_advance: Instant::now(),
            pending_initial: initial_path,
            default_viewer_dialog: false,
            edit_state: None,
            edit_source_path: None,
            edit_texture: None,
        }
    }

    pub fn load_settings() -> Result<Settings> {
        settings_path().and_then(|path| Settings::load(&path))
    }

    fn in_flight_guard(&self) -> MutexGuard<'_, HashSet<PathBuf>> {
        self.in_flight.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn open_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .add_filter("Imágenes", SUPPORTED_EXTENSIONS)
            .pick_file();
        if let Some(path) = picked {
            let t = self.ctx.input(|i| i.time);
            self.open_path(path, t);
        }
    }

    fn open_path(&mut self, path: PathBuf, t: f64) {
        match Navigation::from_folder(&path, SUPPORTED_EXTENSIONS) {
            Ok(nav) => {
                tracing::info!(path = %path.display(), "opening image");
                self.thumb_epoch.fetch_add(1, Ordering::Relaxed);
                self.thumb_cache.clear();
                self.sidebar.clear_textures();
                self.thumb_queue.drain();
                self.exif_cache
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clear();
                self.navigation = Some(nav);
                self.enqueue_thumbnails();
                self.start_load(path);
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "failed to scan folder");
                self.toasts
                    .push(format!("No se pudo leer la carpeta: {e}"), t);
            }
        }
    }

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

    fn texture_from_cache(&self, path: &std::path::Path) -> Option<(egui::TextureHandle, Vec2)> {
        let entry = self.cache.get(path)?;
        let texture = make_texture(&self.ctx, entry.first_frame());
        let size = entry.dimensions();
        Some((texture, Vec2::new(size.0 as f32, size.1 as f32)))
    }

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
    fn navigate_to(&mut self, index: usize) {
        self.pause_slideshow();
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
    fn toggle_sidebar(&mut self) {
        self.sidebar.show = !self.sidebar.show;
        if self.sidebar.show {
            self.enqueue_thumbnails();
        }
    }
    fn enqueue_thumbnails(&mut self) {
        let Some(nav) = &self.navigation else {
            return;
        };
        for image_path in &nav.images {
            self.thumb_queue.push(image_path.clone());
        }
    }
    fn toggle_slideshow(&mut self) {
        self.slideshow_active = !self.slideshow_active;
        self.slideshow_last_advance = Instant::now();
        self.ctx.request_repaint();
        tracing::info!(active = self.slideshow_active, "slideshow toggled");
    }
    fn change_slideshow_speed(&mut self, faster: bool) {
        self.slideshow_interval = if faster {
            slideshow::faster(self.slideshow_interval)
        } else {
            slideshow::slower(self.slideshow_interval)
        };
        self.settings.slideshow_interval_secs = self.slideshow_interval.as_secs().max(1);
        if let Ok(path) = settings_path() {
            if let Err(e) = self.settings.save(&path) {
                tracing::warn!(error = %e, "failed to persist slideshow interval");
            }
        }
        self.slideshow_last_advance = Instant::now();
        tracing::info!(
            interval_secs = self.settings.slideshow_interval_secs,
            "slideshow speed changed"
        );
    }
    fn advance_slideshow(&mut self) {
        self.slideshow_last_advance = Instant::now();
        if let Some(nav) = &mut self.navigation {
            nav.next();
            if let Some(path) = nav.current_path().cloned() {
                self.start_load(path);
            }
        }
    }
    fn pause_slideshow(&mut self) {
        if self.slideshow_active {
            self.slideshow_active = false;
            tracing::info!("slideshow paused by user interaction");
        }
    }
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
    fn dispatch(&mut self, action: Action) {
        match action {
            Action::Open => self.open_dialog(),
            Action::Prev => {
                if self.edit_state.is_none() {
                    self.pause_slideshow();
                    self.navigate(-1);
                }
            }
            Action::Next => {
                if self.edit_state.is_none() {
                    self.pause_slideshow();
                    self.navigate(1);
                }
            }
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
            Action::ToggleSlideshow => self.toggle_slideshow(),
            Action::SlideshowFaster => self.change_slideshow_speed(true),
            Action::SlideshowSlower => self.change_slideshow_speed(false),
            Action::EditShortcuts => self.shortcut_dialog.open = true,
            Action::SetDefaultViewer => self.default_viewer_dialog = true,
            Action::Edit => self.enter_edit_mode(),
            Action::SaveCopy => self.save_edit_copy(false),
            Action::SaveAs => self.save_edit_copy(true),
            Action::CancelEdit => self.exit_edit_mode(),
            Action::ResetEdit => self.reset_edit(),
            Action::ApplyCrop => self.apply_crop_edit(),
            Action::SetLangEs => self.set_language(Language::Es),
            Action::SetLangEn => self.set_language(Language::En),
        }
    }
    fn set_language(&mut self, lang: Language) {
        self.settings.language = lang;
        if let Ok(path) = settings_path() {
            if let Err(e) = self.settings.save(&path) {
                tracing::warn!(error = %e, "failed to persist language");
            }
        }
        tracing::info!(language = ?lang, "language changed");
    }
    fn set_default_viewer(&mut self) {
        let t = self.ctx.input(|i| i.time);
        let tr = self.settings.language.translations();
        match crate::utils::default_app::set_as_default_image_viewer() {
            Ok(()) => self.toasts.push(tr.default_viewer_success.to_string(), t),
            Err(e) => self
                .toasts
                .push(format!("{} {e}", tr.default_viewer_error), t),
        }
    }
    fn enter_edit_mode(&mut self) {
        let Some(path) = self
            .navigation
            .as_ref()
            .and_then(|n| n.current_path())
            .cloned()
        else {
            return;
        };
        let Some(entry) = self.cache.get(&path) else {
            let tr = self.settings.language.translations();
            self.toasts
                .push(tr.load_error.to_string(), self.ctx.input(|i| i.time));
            return;
        };
        let image = entry.first_frame().clone();
        self.edit_state = Some(EditState::new(image));
        self.edit_source_path = Some(path);
        self.edit_texture = None;
        if let Some(ref p) = self.edit_source_path {
            tracing::info!("entered edit mode for {}", p.display());
        }
    }
    fn exit_edit_mode(&mut self) {
        self.edit_state = None;
        self.edit_source_path = None;
        self.edit_texture = None;
        tracing::info!("exited edit mode");
    }
    fn save_edit_copy(&mut self, choose_path: bool) {
        let Some(state) = &self.edit_state else {
            return;
        };
        let Some(ref source_path) = self.edit_source_path else {
            return;
        };
        let final_image = apply_all(
            &state.original,
            state.crop_rect,
            state.filter,
            state.brightness,
            state.contrast,
            state.saturation,
        );

        let save_path = if choose_path {
            let suggested = source_path
                .file_stem()
                .map(|s| format!("{}_edit", s.to_string_lossy()))
                .unwrap_or_else(|| "imagen_editada".to_string());
            rfd::FileDialog::new()
                .set_file_name(&suggested)
                .add_filter("PNG", &["png"])
                .add_filter("JPEG", &["jpg"])
                .add_filter("BMP", &["bmp"])
                .add_filter("WebP", &["webp"])
                .save_file()
        } else {
            let stem = source_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "imagen".to_string());
            let ext = source_path
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_else(|| "png".to_string());
            let parent = source_path.parent().unwrap_or(std::path::Path::new(""));
            Some(parent.join(format!("{}_edit.{}", stem, ext)))
        };

        if let Some(path) = save_path {
            let format = guess_format(&path);
            let result = match format {
                ImageFormat::Jpeg => final_image
                    .to_rgb8()
                    .save_with_format(&path, ImageFormat::Jpeg),
                ImageFormat::Bmp => final_image
                    .to_rgb8()
                    .save_with_format(&path, ImageFormat::Bmp),
                _ => final_image.save(&path),
            };
            let t = self.ctx.input(|i| i.time);
            let tr = self.settings.language.translations();
            match result {
                Ok(()) => {
                    tracing::info!(path = %path.display(), "saved edited image");
                    self.toasts
                        .push(format!("{} {}", tr.save_success, path.display()), t);
                    self.exit_edit_mode();
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to save edited image");
                    self.toasts.push(format!("{} {e}", tr.save_error), t);
                }
            }
        }
    }
    fn reset_edit(&mut self) {
        let Some(state) = &mut self.edit_state else {
            return;
        };
        state.reset();
        self.edit_texture = None;
    }
    fn apply_crop_edit(&mut self) {
        let Some(state) = &mut self.edit_state else {
            return;
        };
        let Some(rect) = state.crop_rect else {
            return;
        };
        let cropped = apply_crop(&state.original, rect);
        state.original = cropped.clone();
        state.working = cropped;
        state.crop_rect = None;
        state.filter = None;
        state.brightness = 0;
        state.contrast = 0;
        state.saturation = 0;
        self.edit_texture = None;
    }
    fn update_edit_texture(&mut self) {
        let Some(state) = &self.edit_state else {
            return;
        };
        let preview = apply_all(
            &state.original,
            None,
            state.filter,
            state.brightness,
            state.contrast,
            state.saturation,
        );
        self.edit_texture = Some(make_texture(&self.ctx, &preview));
        let dims = preview.dimensions();
        self.transform = ViewTransform::new(
            Vec2::new(dims.0 as f32, dims.1 as f32),
            self.transform.viewport,
        );
    }
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
    fn toggle_fullscreen(&mut self) {
        self.is_fullscreen = !self.is_fullscreen;
        self.ctx
            .send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
        tracing::info!(fullscreen = self.is_fullscreen, "toggled fullscreen");
    }
    fn toggle_theme(&mut self) {
        self.settings.theme = theme::toggle(&self.settings.theme).to_string();
        if let Ok(path) = settings_path() {
            if let Err(e) = self.settings.save(&path) {
                tracing::warn!(error = %e, "failed to persist theme");
            }
        }
        tracing::info!(theme = %self.settings.theme, "theme toggled");
    }
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
    fn show_default_viewer_dialog(&mut self, ui: &mut egui::Ui) {
        let mut confirmed = false;
        let mut cancelled = false;
        let tr = self.settings.language.translations();

        egui::Window::new(tr.default_viewer_dialog_title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
                ui.label(tr.default_viewer_dialog_body);
                ui.add_space(8.0);
                ui.label(tr.default_viewer_dialog_formats);
                ui.indent("formatos", |ui| {
                    let exts = crate::utils::default_app::associated_extensions();
                    let per_row = 5;
                    let rows = exts.len().div_ceil(per_row);
                    for row in 0..rows {
                        ui.horizontal(|ui| {
                            for col in 0..per_row {
                                let idx = row * per_row + col;
                                if idx < exts.len() {
                                    ui.monospace(exts[idx]);
                                }
                            }
                        });
                    }
                });
                ui.add_space(12.0);
                ui.label(tr.default_viewer_dialog_continue);
                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    if ui.button(tr.continue_btn).clicked() {
                        confirmed = true;
                    }
                    if ui.button(tr.cancel).clicked() {
                        cancelled = true;
                    }
                });
            });

        if confirmed {
            self.default_viewer_dialog = false;
            self.set_default_viewer();
        }
        if cancelled {
            self.default_viewer_dialog = false;
        }
    }
}
fn make_texture(ctx: &egui::Context, image: &DynamicImage) -> egui::TextureHandle {
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.to_rgba8();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    ctx.load_texture("image", color_image, egui::TextureOptions::LINEAR)
}
fn guess_format(path: &Path) -> ImageFormat {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "bmp" => ImageFormat::Bmp,
        "gif" => ImageFormat::Gif,
        "tiff" | "tif" => ImageFormat::Tiff,
        "webp" => ImageFormat::WebP,
        "avif" => ImageFormat::Avif,
        _ => ImageFormat::Png,
    }
}

impl eframe::App for ShImagesApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::apply(ui.ctx(), &self.settings.theme);
        let t = ui.input(|i| i.time);
        ui.ctx()
            .data_mut(|d| d.insert_temp("app_lang".into(), self.settings.language));
        if self.navigation.is_none() {
            if let Some(path) = self.pending_initial.take() {
                tracing::info!(path = %path.display(), "opening CLI path");
                self.open_path(path, t);
            }
        }

        self.poll_loader(t);
        self.poll_thumbnails();
        self.poll_exif();
        self.tick_animation();

        if self.slideshow_active && self.navigation.is_some() {
            if slideshow::elapsed_reached(
                self.slideshow_last_advance.elapsed(),
                self.slideshow_interval,
            ) {
                self.advance_slideshow();
            }
            self.ctx.request_repaint_after(self.slideshow_interval);
        }
        let action = toolbar::show(
            ui,
            &self.shortcuts,
            &self.settings.theme,
            self.is_fullscreen,
            self.slideshow_active,
            self.settings.language,
        );
        if let Some(action) = action {
            self.dispatch(action);
        }
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
                let lang = self.settings.language;
                let t = lang.translations();
                ui.menu_button(t.menu_file, |ui| {
                    if ui.button(t.open).clicked() {
                        ui.close();
                        want_open = true;
                    }
                });
                ui.menu_button(t.menu_view, |ui| {
                    if ui.button(Action::ToggleSidebar.label(lang)).clicked() {
                        ui.close();
                        self.dispatch(Action::ToggleSidebar);
                    }
                    if ui.button(Action::Fullscreen.label(lang)).clicked() {
                        ui.close();
                        self.dispatch(Action::Fullscreen);
                    }
                });
                ui.menu_button(t.menu_help, |ui| {
                    if ui.button(Action::EditShortcuts.label(lang)).clicked() {
                        ui.close();
                        self.dispatch(Action::EditShortcuts);
                    }
                });
            });
            if want_open {
                self.open_dialog();
            }
            if self.edit_state.is_some() {
                self.update_edit_texture();
                if let Some(edit_state) = &mut self.edit_state {
                    let editor_resp = editor::show(ui.ctx(), edit_state);
                    if editor_resp.start_crop {
                        if let Some(state) = &mut self.edit_state {
                            state.crop_mode = true;
                            state.crop_rect = None;
                        }
                    } else if editor_resp.apply_crop {
                        if let Some((x, y, w, h)) = viewer::get_crop_result(ui.ctx()) {
                            if let Some(state) = &mut self.edit_state {
                                state.crop_rect =
                                    Some(crate::core::edit_state::CropRect { x, y, w, h });
                                state.crop_mode = false;
                            }
                        } else {
                            self.apply_crop_edit();
                        }
                    } else if editor_resp.cancel_crop {
                        if let Some(state) = &mut self.edit_state {
                            state.crop_rect = None;
                            state.crop_mode = false;
                        }
                    } else if editor_resp.save {
                        self.save_edit_copy(false);
                    } else if editor_resp.save_as {
                        self.save_edit_copy(true);
                    } else if editor_resp.cancel {
                        self.exit_edit_mode();
                    } else if editor_resp.reset {
                        self.reset_edit();
                    }
                }
                if let Some(ref tex) = self.edit_texture {
                    let crop_mode = self.edit_state.as_ref().is_some_and(|s| s.crop_mode);
                    let crop_selection = if crop_mode {
                        viewer::get_crop_selection(ui.ctx())
                    } else {
                        None
                    };
                    let resp = viewer::show_editable(
                        ui,
                        tex,
                        &mut self.transform,
                        crop_mode,
                        crop_selection,
                    );
                    if resp.zoomed || resp.panned {
                        self.user_interacted = true;
                        self.pause_slideshow();
                    }
                }
            } else {
                match &self.texture {
                    Some(texture) => {
                        let resp = viewer::show(ui, texture, &mut self.transform);
                        if resp.zoomed || resp.panned {
                            self.user_interacted = true;
                            self.pause_slideshow();
                        }
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
            }
        });

        self.toasts.update(t);
        self.toasts.show(ui);

        if self.shortcut_dialog.open {
            let changed =
                self.shortcut_dialog
                    .show(ui, &mut self.shortcuts, self.settings.language);
            if changed {
                if let Ok(path) = settings_path() {
                    if let Err(e) = self.settings.save(&path) {
                        tracing::warn!(error = %e, "failed to persist shortcuts");
                    }
                }
            }
        }

        if self.default_viewer_dialog {
            self.show_default_viewer_dialog(ui);
        }

        self.handle_shortcuts(ui);
    }
}
