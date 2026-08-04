//! Tests de integración de los flujos críticos (AGENTS.md §8.1).
//!
//! Lógica pura sobre `core/` (sin egui): el paso de "diálogo nativo" de la
//! spec se abstrae porque `open_path` ya recibe un `PathBuf`.

mod common;

use std::ops::Add;

use sh_images::config::settings::Settings;
use sh_images::core::exif::read_exif;
use sh_images::core::image_cache::ImageCache;
use sh_images::core::image_loader::load_image;
use sh_images::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use sh_images::core::preload::{preload_targets, PRELOAD_DEPTH};
use sh_images::core::shortcuts::ShortcutMap;
use sh_images::core::view::{Vec2, ViewTransform};

use common::{
    copy_fixture, corrupt_png_path, empty_png_path, gif_path, make_folder_with_images,
    make_folder_with_rect_images,
};

/// Flujo 1 — Apertura: abrir → decodificar → cachear.
///
/// Equivale a "abrir app → seleccionar imagen → renderizar": se construye la
/// navegación de la carpeta, se decodifica la imagen actual y se cachea.
#[test]
fn flujo_apertura_completo() {
    let (_dir, paths) = make_folder_with_images(3);
    let target = &paths[1];

    let nav = Navigation::from_folder(target, SUPPORTED_EXTENSIONS).expect("carpeta válida");
    assert_eq!(nav.current_path().expect("imagen actual"), target);

    let image = load_image(nav.current_path().expect("imagen actual"))
        .expect("decodificar imagen sintética");
    let (w, h) = image.dimensions();
    assert_eq!((w, h), (64, 64), "la imagen sintética es 64x64");

    let cache = ImageCache::new(512);
    let result = cache.insert_loaded(target.clone(), image);
    assert!(result.cached, "la imagen 64x64 cabe en el LRU 512MiB");
    let entry = cache.get(target).expect("re-leer del cache");
    assert_eq!(
        entry.dimensions(),
        (64, 64),
        "la entrada cacheada conserva dimensiones"
    );
    // `CacheEntryRef` mantiene el lock del cache; soltarlo antes de consultar
    // `contains` (de lo contrario `preload_targets` se bloquea en el mutex).
    drop(entry);

    // Pre-carga N±1: desde la imagen 1 (0-indexed), los vecinos son 0 y 2.
    let targets = preload_targets(&nav, PRELOAD_DEPTH, |p| cache.contains(p), |_| false);
    assert_eq!(targets.len(), 2, "dos vecinos a precargar");
    assert!(
        targets.iter().all(|p| !cache.contains(p)),
        "vecinos aún no cacheados"
    );
}

/// Flujo 2 — Navegación: forward/backward circular + orden correcto.
#[test]
fn flujo_navegacion_circular() {
    let (_dir, paths) = make_folder_with_images(5);

    let mut nav = Navigation::from_folder(&paths[0], SUPPORTED_EXTENSIONS).expect("carpeta válida");
    assert_eq!(nav.images.len(), 5);

    // Los paths están ordenados alfabéticamente por nombre img_0000..img_0004.
    assert_eq!(nav.current_path().expect("actual"), &paths[0]);

    // Forward hasta el final: vuelve al inicio (circular).
    for _ in 0..5 {
        nav.next();
    }
    assert_eq!(
        nav.current_path().expect("actual"),
        &paths[0],
        "circular forward"
    );

    // Backward desde el inicio: va al final (circular).
    nav.prev();
    assert_eq!(
        nav.current_path().expect("actual"),
        &paths[4],
        "circular backward"
    );

    // Vecinos N±1 desde el índice 2.
    let mid = Navigation::from_folder(&paths[2], SUPPORTED_EXTENSIONS).expect("válida");
    let [prev, next] = mid.neighbor_paths();
    assert_eq!(prev.expect("anterior"), &paths[1]);
    assert_eq!(next.expect("siguiente"), &paths[3]);
}

/// Flujo 3 — Zoom/Fit: zoom in centrado → fit restaura.
#[test]
fn flujo_zoom_fit_centrado() {
    let mut t = ViewTransform::new(Vec2::new(2000.0, 1000.0), Vec2::new(500.0, 500.0));
    let fit = t.fit_zoom();
    let center = Vec2::new(250.0, 250.0);

    // El centro de la imagen está bajo el centro del canvas (ancla fija).
    let origin = t.image_origin_screen();
    let img_center = origin.add(Vec2::new(2000.0 * t.zoom * 0.5, 1000.0 * t.zoom * 0.5));
    assert!((img_center.x - center.x).abs() < 1e-3, "centrada en x");
    assert!((img_center.y - center.y).abs() < 1e-3, "centrada en y");

    // Zoom in con rueda: sigue centrada.
    t.apply_center(2.0);
    assert!(t.zoom > fit, "zoom in supera el fit");
    let origin2 = t.image_origin_screen();
    let img_center2 = origin2.add(Vec2::new(2000.0 * t.zoom * 0.5, 1000.0 * t.zoom * 0.5));
    assert!(
        (img_center2.x - center.x).abs() < 1e-3,
        "sigue centrada en x"
    );
    assert!(
        (img_center2.y - center.y).abs() < 1e-3,
        "sigue centrada en y"
    );

    // Fit restaura zoom y centra.
    t.fit();
    let origin_fit = t.image_origin_screen();
    let expected = Vec2::new((500.0 - 2000.0 * fit) / 2.0, (500.0 - 1000.0 * fit) / 2.0);
    assert!((t.zoom - fit).abs() < 1e-3, "fit restaura zoom");
    assert!((origin_fit.x - expected.x).abs() < 1e-3, "fit centra en x");
    assert!((origin_fit.y - expected.y).abs() < 1e-3, "fit centra en y");
}

/// Flujo 4 — Error: imagen corrupta no crashea; carpeta inexistente da Err;
/// carpeta sin imágenes soportadas da Ok con lista vacía.
#[test]
fn flujo_imagen_corrupta_no_crash() {
    let dir = tempfile::tempdir().expect("tempdir");

    let corrupt = corrupt_png_path(dir.path());
    let err = load_image(&corrupt).expect_err("png corrupto devuelve error");
    assert!(
        matches!(err, sh_images::utils::errors::ShImagesError::Decode(_)),
        "error de decode, no panic"
    );

    let empty = empty_png_path(dir.path());
    assert!(load_image(&empty).is_err(), "png vacío también es error");

    let gif = gif_path(dir.path());
    let gif_img = load_image(&gif).expect("gif válido decodifica");
    assert_eq!(gif_img.dimensions(), (1, 1));

    // Carpeta inexistente → Err(Io).
    let missing = dir.path().join("no_such_dir").join("a.png");
    let err = Navigation::from_folder(&missing, SUPPORTED_EXTENSIONS)
        .expect_err("carpeta inexistente da error");
    assert!(matches!(
        err,
        sh_images::utils::errors::ShImagesError::Io(_)
    ));

    // Carpeta con solo .txt → Ok con imágenes vacías.
    let empty_folder = dir.path().join("no_images");
    std::fs::create_dir_all(&empty_folder).expect("crear carpeta");
    std::fs::write(empty_folder.join("notes.txt"), b"not an image").expect("escribir txt");
    let nav = Navigation::from_folder(&empty_folder.join("x.txt"), SUPPORTED_EXTENSIONS)
        .expect("carpeta leíble");
    assert!(nav.images.is_empty(), "ninguna imagen soportada");
    assert_eq!(nav.current_path(), None, "sin imagen actual");
}

/// Flujo 5 — Configuración: modificar → guardar → recargar (cerrar/reabrir).
#[test]
fn flujo_configuracion_persistencia() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("settings.toml");

    let first = Settings::default();
    assert_eq!(first.cache_memory_limit_mb, 512);
    first.save(&path).expect("guardar defaults");

    // "Cerrar la app": modificar el archivo como haría un segundo arranque.
    let modified = Settings {
        cache_memory_limit_mb: 256,
        theme: "light".to_string(),
        shortcuts: ShortcutMap::defaults(),
        slideshow_interval_secs: 5,
    };
    modified.save(&path).expect("guardar modificado");

    // "Reabrir la app": recargar desde disco.
    let loaded = Settings::load(&path).expect("cargar settings");
    assert_eq!(loaded, modified, "persiste el último valor guardado");
    assert_eq!(loaded.theme, "light");
}

/// Flujo 6 — Rotación: abrir → rotar → verificar transform (sin GUI).
///
/// Equivale a "abrir imagen → Ctrl+] (rotate CW) → ver que queda en fit con
/// dimensiones efectivas intercambiadas". Se testea la math pura de core.
#[test]
fn flujo_rotacion_visual() {
    // Imagen NO cuadrada (80x40) y viewport NO cuadrado (600x300): la rotación
    // debe intercambiar dims y cambiar el fit.
    let (_dir, paths) = make_folder_with_rect_images(1, 80, 40);
    let target = &paths[0];

    let image = load_image(target).expect("decodificar imagen sintética");
    let (w, h) = image.dimensions();
    assert_eq!((w, h), (80, 40), "imagen sintética no cuadrada");

    // "Abrir": el transform se crea en fit con las dimensiones reales.
    let mut t = ViewTransform::new(Vec2::new(w as f32, h as f32), Vec2::new(600.0, 300.0));
    assert_eq!(t.rotation, 0);
    let fit0 = t.fit_zoom(); // min(600/80, 300/40) = min(7.5, 7.5) = 7.5

    // "Rotar 90° CW": dimensiones efectivas intercambiadas y fit recomputado.
    t.rotate_cw();
    assert_eq!(t.rotation, 1);
    let eff = t.effective_size();
    assert_eq!((eff.x as u32, eff.y as u32), (h, w), "dims intercambiadas");
    // min(600/40, 300/80) = min(15.0, 3.75) = 3.75 ≠ 7.5
    assert!(
        (t.fit_zoom() - fit0).abs() > 1e-6,
        "fit cambia con rotación"
    );
    // "Rotar 90° CCW" restaura la orientación original.
    t.rotate_ccw();
    assert_eq!(t.rotation, 0);
    assert!((t.fit_zoom() - fit0).abs() < 1e-3, "vuelve al fit original");
}

/// Flujo 7 — EXIF: leer metadatos de un JPEG real; no-crash sin EXIF.
#[test]
fn flujo_exif() {
    let dir = tempfile::tempdir().expect("tempdir");

    let jpg = copy_fixture(dir.path(), "exif.jpg");
    let img = read_exif(&jpg).expect("leer ok").expect("jpg con exif");
    assert!(
        img.make.is_some() || img.model.is_some() || img.fecha.is_some(),
        "JPEG con EXIF expone cámara o fecha"
    );

    let png = copy_fixture(dir.path(), "sample.png");
    assert_eq!(read_exif(&png).expect("ok"), None, "PNG sin EXIF → None");
}
