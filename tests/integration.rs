//! Tests de integración de los flujos críticos (AGENTS.md §8.1).
//!
//! Lógica pura sobre `core/` (sin egui): el paso de "diálogo nativo" de la
//! spec se abstrae porque `open_path` ya recibe un `PathBuf`.

mod common;

use std::ops::{Add, Div, Mul, Sub};

use image::GenericImageView;
use sh_images::config::settings::Settings;
use sh_images::core::image_cache::ImageCache;
use sh_images::core::image_loader::load_image;
use sh_images::core::navigation::{Navigation, SUPPORTED_EXTENSIONS};
use sh_images::core::preload::{preload_targets, PRELOAD_DEPTH};
use sh_images::core::shortcuts::ShortcutMap;
use sh_images::core::view::{Vec2, ViewTransform};

use common::{corrupt_png_path, empty_png_path, gif_path, make_folder_with_images};

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
    let result = cache.insert(target.clone(), image);
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

/// Flujo 3 — Zoom/Pan: zoom in → pan → fit restaura.
#[test]
fn flujo_zoom_pan_fit() {
    let mut t = ViewTransform::new(Vec2::new(2000.0, 1000.0), Vec2::new(500.0, 500.0));
    let fit = t.fit_zoom();
    let anchor = Vec2::new(250.0, 250.0);

    // Capturar el punto de imagen bajo el ancla ANTES del zoom.
    let origin = t.image_origin_screen();
    let image_point = anchor.sub(origin).div(t.zoom);

    // Zoom in en un punto ancla.
    t.apply_zoom_at(anchor, 2.0);
    assert!(t.zoom > fit, "zoom in supera el fit");

    // El punto de imagen capturado antes debe mapear al mismo ancla de pantalla.
    let new_origin = t.image_origin_screen();
    let new_screen = new_origin.add(image_point.mul(t.zoom));
    assert!((new_screen.x - anchor.x).abs() < 1e-3, "ancla fija en x");
    assert!((new_screen.y - anchor.y).abs() < 1e-3, "ancla fija en y");

    // Pan desplaza.
    let before = t.image_origin_screen();
    t.pan_by(Vec2::new(30.0, -10.0));
    let after = t.image_origin_screen();
    assert!((after.x - before.x - 30.0).abs() < 1e-3, "pan mueve en x");
    assert!((after.y - before.y + 10.0).abs() < 1e-3, "pan mueve en y");

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
    };
    modified.save(&path).expect("guardar modificado");

    // "Reabrir la app": recargar desde disco.
    let loaded = Settings::load(&path).expect("cargar settings");
    assert_eq!(loaded, modified, "persiste el último valor guardado");
    assert_eq!(loaded.theme, "light");
}
