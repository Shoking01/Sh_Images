use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Es,
    En,
}

impl Language {
    pub fn native_name(self) -> &'static str {
        match self {
            Language::Es => "Español",
            Language::En => "English",
        }
    }
    pub fn translations(self) -> &'static Translations {
        match self {
            Language::Es => &ES,
            Language::En => &EN,
        }
    }
}
pub struct Translations {
    pub open: &'static str,
    pub prev: &'static str,
    pub next: &'static str,
    pub rotate_cw: &'static str,
    pub rotate_ccw: &'static str,
    pub fit: &'static str,
    pub fullscreen: &'static str,
    pub toggle_theme: &'static str,
    pub toggle_sidebar: &'static str,
    pub toggle_info: &'static str,
    pub toggle_slideshow: &'static str,
    pub slideshow_faster: &'static str,
    pub slideshow_slower: &'static str,
    pub edit_shortcuts: &'static str,
    pub set_default_viewer: &'static str,
    pub edit: &'static str,
    pub save_copy: &'static str,
    pub save_as: &'static str,
    pub cancel_edit: &'static str,
    pub reset_edit: &'static str,
    pub apply_crop: &'static str,
    pub editor_title: &'static str,
    pub crop: &'static str,
    pub select_area: &'static str,
    pub apply: &'static str,
    pub cancel: &'static str,
    pub crop_active_hint: &'static str,
    pub crop_selection: &'static str,
    pub crop_dimensions: &'static str,
    pub filters: &'static str,
    pub grayscale: &'static str,
    pub sepia: &'static str,
    pub invert: &'static str,
    pub black_white: &'static str,
    pub adjustments: &'static str,
    pub brightness: &'static str,
    pub contrast: &'static str,
    pub saturation: &'static str,
    pub save_copy_btn: &'static str,
    pub save_as_btn: &'static str,
    pub reset_btn: &'static str,
    pub cancel_edit_btn: &'static str,
    pub filter_grayscale: &'static str,
    pub filter_sepia: &'static str,
    pub filter_invert: &'static str,
    pub filter_black_white: &'static str,
    pub default_viewer_dialog_title: &'static str,
    pub default_viewer_dialog_body: &'static str,
    pub default_viewer_dialog_formats: &'static str,
    pub default_viewer_dialog_continue: &'static str,
    pub continue_btn: &'static str,
    pub save_success: &'static str,
    pub save_error: &'static str,
    pub default_viewer_success: &'static str,
    pub default_viewer_error: &'static str,
    pub load_error: &'static str,
    pub cache_error: &'static str,
    pub no_size: &'static str,
    pub camera: &'static str,
    pub iso: &'static str,
    pub aperture: &'static str,
    pub focal: &'static str,
    pub date: &'static str,
    pub dimensions: &'static str,
    pub language: &'static str,
    pub menu_hint: &'static str,
    pub capture_prefix: &'static str,
    pub capture_suffix: &'static str,
    pub conflict_prefix: &'static str,
    pub conflict_suffix: &'static str,
    pub invalid_combo: &'static str,
    pub reset_all: &'static str,
}

const ES: Translations = Translations {
    open: "Abrir",
    prev: "Anterior",
    next: "Siguiente",
    rotate_cw: "Rotar 90° CW",
    rotate_ccw: "Rotar 90° CCW",
    fit: "Ajustar a la ventana",
    fullscreen: "Pantalla completa",
    toggle_theme: "Cambiar tema",
    toggle_sidebar: "Barra lateral",
    toggle_info: "Información",
    toggle_slideshow: "Slideshow",
    slideshow_faster: "Slideshow más rápido",
    slideshow_slower: "Slideshow más lento",
    edit_shortcuts: "Configurar atajos",
    set_default_viewer: "Establecer como predeterminado",
    edit: "Editar",
    save_copy: "Guardar copia",
    save_as: "Guardar como",
    cancel_edit: "Cancelar edición",
    reset_edit: "Restablecer",
    apply_crop: "Aplicar recorte",

    editor_title: "Editor",
    crop: "Recorte",
    select_area: "Seleccionar área",
    apply: "Aplicar",
    cancel: "Cancelar",
    crop_active_hint: "Arrastra sobre la imagen para seleccionar.",
    crop_selection: "Selección",
    crop_dimensions: "Recorte",
    filters: "Filtros",
    grayscale: "Grises",
    sepia: "Sepia",
    invert: "Invertir",
    black_white: "B/N",
    adjustments: "Ajustes",
    brightness: "Brillo",
    contrast: "Contraste",
    saturation: "Saturación",
    save_copy_btn: "Guardar copia",
    save_as_btn: "Guardar como…",
    reset_btn: "Restablecer",
    cancel_edit_btn: "Cancelar edición",

    filter_grayscale: "Grises",
    filter_sepia: "Sepia",
    filter_invert: "Invertir",
    filter_black_white: "B/N",

    default_viewer_dialog_title: "Establecer como visualizador predeterminado",
    default_viewer_dialog_body: "Sh_Images se asociará como programa predeterminado para abrir imágenes.",
    default_viewer_dialog_formats: "Formatos que se asociarán:",
    default_viewer_dialog_continue: "Al continuar se abrirá la Configuración de Windows.\nBusca \"Sh_Images\" en la lista y selecciónalo como predeterminado.",
    continue_btn: "Continuar",

    save_success: "Guardado:",
    save_error: "No se pudo guardar:",
    default_viewer_success: "Se abrió Configuración. Busca Sh_Images y elígelo como predeterminado.",
    default_viewer_error: "No se pudo abrir la configuración:",
    load_error: "No se pudo cargar la imagen para editar",
    cache_error: "La imagen excede el límite de memoria del cache",

    no_size: "—",

    camera: "Cámara",
    iso: "ISO",
    aperture: "Apertura",
    focal: "Focal",
    date: "Fecha",
    dimensions: "Dimensiones",

    language: "Idioma",
    menu_hint: "Archivo → Abrir… o Ctrl+O",

    capture_prefix: "Pulsa la nueva combinación para",
    capture_suffix: "(Esc cancela)",
    conflict_prefix: "«",
    conflict_suffix: "» ya usa esa combinación",
    invalid_combo: "Combinación no válida",
    reset_all: "Restablecer todos",
};

const EN: Translations = Translations {
    open: "Open",
    prev: "Previous",
    next: "Next",
    rotate_cw: "Rotate 90° CW",
    rotate_ccw: "Rotate 90° CCW",
    fit: "Fit to window",
    fullscreen: "Fullscreen",
    toggle_theme: "Toggle theme",
    toggle_sidebar: "Sidebar",
    toggle_info: "Info",
    toggle_slideshow: "Slideshow",
    slideshow_faster: "Slideshow faster",
    slideshow_slower: "Slideshow slower",
    edit_shortcuts: "Configure shortcuts",
    set_default_viewer: "Set as default viewer",
    edit: "Edit",
    save_copy: "Save copy",
    save_as: "Save as",
    cancel_edit: "Cancel edit",
    reset_edit: "Reset",
    apply_crop: "Apply crop",

    editor_title: "Editor",
    crop: "Crop",
    select_area: "Select area",
    apply: "Apply",
    cancel: "Cancel",
    crop_active_hint: "Drag on the image to select.",
    crop_selection: "Selection",
    crop_dimensions: "Crop",
    filters: "Filters",
    grayscale: "Grayscale",
    sepia: "Sepia",
    invert: "Invert",
    black_white: "B/W",
    adjustments: "Adjustments",
    brightness: "Brightness",
    contrast: "Contrast",
    saturation: "Saturation",
    save_copy_btn: "Save copy",
    save_as_btn: "Save as…",
    reset_btn: "Reset",
    cancel_edit_btn: "Cancel edit",

    filter_grayscale: "Grayscale",
    filter_sepia: "Sepia",
    filter_invert: "Invert",
    filter_black_white: "B/W",

    default_viewer_dialog_title: "Set as default image viewer",
    default_viewer_dialog_body: "Sh_Images will be associated as the default program to open images.",
    default_viewer_dialog_formats: "Formats to associate:",
    default_viewer_dialog_continue: "Continuing will open Windows Settings.\nFind \"Sh_Images\" in the list and select it as default.",
    continue_btn: "Continue",

    save_success: "Saved:",
    save_error: "Could not save:",
    default_viewer_success: "Settings opened. Find Sh_Images and select it as default.",
    default_viewer_error: "Could not open settings:",
    load_error: "Could not load image for editing",
    cache_error: "Image exceeds cache memory limit",

    no_size: "—",

    camera: "Camera",
    iso: "ISO",
    aperture: "Aperture",
    focal: "Focal",
    date: "Date",
    dimensions: "Dimensions",

    language: "Language",
    menu_hint: "File → Open… or Ctrl+O",

    capture_prefix: "Press new shortcut for",
    capture_suffix: "(Esc cancels)",
    conflict_prefix: "«",
    conflict_suffix: "» already uses that shortcut",
    invalid_combo: "Invalid shortcut",
    reset_all: "Reset all",
};
#[macro_export]
macro_rules! t {
    ($lang:expr, $key:ident) => {
        $lang.translations().$key
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_have_native_names() {
        assert_eq!(Language::Es.native_name(), "Español");
        assert_eq!(Language::En.native_name(), "English");
    }

    #[test]
    fn es_and_en_have_same_keys() {
        let es = Language::Es.translations();
        let en = Language::En.translations();
        assert_eq!(es.open, "Abrir");
        assert_eq!(en.open, "Open");
        assert_eq!(es.edit, "Editar");
        assert_eq!(en.edit, "Edit");
    }

    #[test]
    fn language_serializes_lowercase() {
        #[derive(Serialize)]
        struct Wrap {
            lang: Language,
        }
        let s = toml::to_string(&Wrap { lang: Language::En }).unwrap();
        assert!(s.contains("en"), "serialized: {s}");
        let s = toml::to_string(&Wrap { lang: Language::Es }).unwrap();
        assert!(s.contains("es"), "serialized: {s}");
    }

    #[test]
    fn language_default_is_es() {
        assert_eq!(Language::default(), Language::Es);
    }
}
