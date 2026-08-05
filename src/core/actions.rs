use serde::{Deserialize, Serialize};

use crate::core::lang::Language;
use crate::core::shortcuts::{KeyBinding, KeyCode, Modifiers};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Open,
    Prev,
    Next,
    RotateCw,
    RotateCcw,
    Fit,
    Fullscreen,
    ToggleTheme,
    ToggleSidebar,
    ToggleInfo,
    ToggleSlideshow,
    SlideshowFaster,
    SlideshowSlower,
    EditShortcuts,
    SetDefaultViewer,
    Edit,
    SaveCopy,
    SaveAs,
    CancelEdit,
    ResetEdit,
    ApplyCrop,
    SetLangEs,
    SetLangEn,
}

impl Action {
    pub fn label(self, lang: Language) -> &'static str {
        let t = lang.translations();
        match self {
            Action::Open => t.open,
            Action::Prev => t.prev,
            Action::Next => t.next,
            Action::RotateCw => t.rotate_cw,
            Action::RotateCcw => t.rotate_ccw,
            Action::Fit => t.fit,
            Action::Fullscreen => t.fullscreen,
            Action::ToggleTheme => t.toggle_theme,
            Action::ToggleSidebar => t.toggle_sidebar,
            Action::ToggleInfo => t.toggle_info,
            Action::ToggleSlideshow => t.toggle_slideshow,
            Action::SlideshowFaster => t.slideshow_faster,
            Action::SlideshowSlower => t.slideshow_slower,
            Action::EditShortcuts => t.edit_shortcuts,
            Action::SetDefaultViewer => t.set_default_viewer,
            Action::Edit => t.edit,
            Action::SaveCopy => t.save_copy,
            Action::SaveAs => t.save_as,
            Action::CancelEdit => t.cancel_edit,
            Action::ResetEdit => t.reset_edit,
            Action::ApplyCrop => t.apply_crop,
            Action::SetLangEs => "🌐 ES",
            Action::SetLangEn => "🌐 EN",
        }
    }
    pub fn default_shortcut(self) -> Option<KeyBinding> {
        match self {
            Action::Open => Some(KeyBinding::new(KeyCode::KeyO, Modifiers::Ctrl)),
            Action::Prev => Some(KeyBinding::new(KeyCode::ArrowLeft, Modifiers::None)),
            Action::Next => Some(KeyBinding::new(KeyCode::ArrowRight, Modifiers::None)),
            Action::RotateCw => Some(KeyBinding::new(KeyCode::CloseBracket, Modifiers::Ctrl)),
            Action::RotateCcw => Some(KeyBinding::new(KeyCode::OpenBracket, Modifiers::Ctrl)),
            Action::Fit => Some(KeyBinding::new(KeyCode::KeyF, Modifiers::None)),
            Action::Fullscreen => Some(KeyBinding::new(KeyCode::F11, Modifiers::None)),
            Action::ToggleTheme => Some(KeyBinding::new(KeyCode::KeyT, Modifiers::Ctrl)),
            Action::ToggleSidebar => Some(KeyBinding::new(KeyCode::KeyH, Modifiers::None)),
            Action::ToggleInfo => Some(KeyBinding::new(KeyCode::KeyI, Modifiers::None)),
            Action::ToggleSlideshow => Some(KeyBinding::new(KeyCode::KeyF5, Modifiers::None)),
            Action::SlideshowFaster => Some(KeyBinding::new(KeyCode::Comma, Modifiers::None)),
            Action::SlideshowSlower => Some(KeyBinding::new(KeyCode::Period, Modifiers::None)),
            Action::EditShortcuts => Some(KeyBinding::new(KeyCode::KeyK, Modifiers::Ctrl)),
            Action::SetDefaultViewer => None,
            Action::Edit => None,
            Action::SaveCopy => None,
            Action::SaveAs => None,
            Action::CancelEdit => None,
            Action::ResetEdit => None,
            Action::ApplyCrop => None,
            Action::SetLangEs => None,
            Action::SetLangEn => None,
        }
    }
    pub fn all() -> [Action; 23] {
        [
            Action::Open,
            Action::Prev,
            Action::Next,
            Action::RotateCw,
            Action::RotateCcw,
            Action::Fit,
            Action::Fullscreen,
            Action::ToggleTheme,
            Action::ToggleSidebar,
            Action::ToggleInfo,
            Action::ToggleSlideshow,
            Action::SlideshowFaster,
            Action::SlideshowSlower,
            Action::EditShortcuts,
            Action::SetDefaultViewer,
            Action::Edit,
            Action::SaveCopy,
            Action::SaveAs,
            Action::CancelEdit,
            Action::ResetEdit,
            Action::ApplyCrop,
            Action::SetLangEs,
            Action::SetLangEn,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lang::Language;

    #[test]
    fn every_action_has_a_label() {
        for action in Action::all() {
            assert!(
                !action.label(Language::Es).is_empty(),
                "{action:?} sin label"
            );
        }
    }

    #[test]
    fn every_action_has_a_default_shortcut_except_ui_only() {
        let no_shortcut = &[
            Action::SetDefaultViewer,
            Action::Edit,
            Action::SaveCopy,
            Action::SaveAs,
            Action::CancelEdit,
            Action::ResetEdit,
            Action::ApplyCrop,
            Action::SetLangEs,
            Action::SetLangEn,
        ];
        for action in Action::all() {
            if no_shortcut.contains(&action) {
                assert!(
                    action.default_shortcut().is_none(),
                    "{action:?} no debe tener atajo por defecto"
                );
            } else {
                assert!(
                    action.default_shortcut().is_some(),
                    "{action:?} sin atajo por defecto"
                );
            }
        }
    }

    #[test]
    fn label_is_stable_and_descriptive() {
        assert_eq!(Action::Open.label(Language::Es), "Abrir");
        assert_eq!(Action::Next.label(Language::Es), "Siguiente");
        assert_eq!(Action::RotateCw.label(Language::Es), "Rotar 90° CW");
        assert_eq!(
            Action::EditShortcuts.label(Language::Es),
            "Configurar atajos"
        );
        assert_eq!(Action::ToggleInfo.label(Language::Es), "Información");
        assert_eq!(Action::ToggleSlideshow.label(Language::Es), "Slideshow");
        assert_eq!(
            Action::SlideshowFaster.label(Language::Es),
            "Slideshow más rápido"
        );
        assert_eq!(
            Action::SlideshowSlower.label(Language::Es),
            "Slideshow más lento"
        );
    }

    #[test]
    fn all_returns_twenty_three_actions() {
        let all = Action::all();
        assert_eq!(all.len(), 23);
        let unique: std::collections::HashSet<_> = all.into_iter().collect();
        assert_eq!(unique.len(), 23, "sin variantes duplicadas");
    }

    #[test]
    fn serde_roundtrip_uses_snake_case_names() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Wrap {
            action: Action,
        }
        let w = Wrap {
            action: Action::RotateCw,
        };
        let s = toml::to_string(&w).expect("serializar");
        let back: Wrap = toml::from_str(&s).expect("deserializar");
        assert_eq!(back, w);
        assert!(s.contains("rotate_cw"), "serializado usa snake_case: {s}");
    }
}
