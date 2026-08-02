//! Acciones de la aplicación, centralizadas.
//!
//! Toolbar, menú y atajos convergen en el mismo enum; `app.rs::dispatch` es el
//! único punto que las ejecuta. `core/` no depende de `egui`.

use serde::{Deserialize, Serialize};

use crate::core::shortcuts::{KeyBinding, KeyCode, Modifiers};

/// Todas las acciones invocables de la app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Abrir un archivo con el diálogo nativo.
    Open,
    /// Imagen anterior.
    Prev,
    /// Imagen siguiente.
    Next,
    /// Rotar 90° en sentido horario.
    RotateCw,
    /// Rotar 90° en sentido antihorario.
    RotateCcw,
    /// Ajustar la imagen a la ventana.
    Fit,
    /// Alternar pantalla completa.
    Fullscreen,
    /// Alternar tema oscuro/claro.
    ToggleTheme,
    /// Mostrar u ocultar el sidebar.
    ToggleSidebar,
    /// Abrir el editor de atajos de teclado.
    EditShortcuts,
}

impl Action {
    /// Texto humano de la acción (para botones y tooltips).
    pub fn label(self) -> &'static str {
        match self {
            Action::Open => "Abrir",
            Action::Prev => "Anterior",
            Action::Next => "Siguiente",
            Action::RotateCw => "Rotar 90° CW",
            Action::RotateCcw => "Rotar 90° CCW",
            Action::Fit => "Ajustar a la ventana",
            Action::Fullscreen => "Pantalla completa",
            Action::ToggleTheme => "Cambiar tema",
            Action::ToggleSidebar => "Mostrar/ocultar barra lateral",
            Action::EditShortcuts => "Configurar atajos",
        }
    }

    /// Atajo por defecto de la acción.
    pub fn default_shortcut(self) -> Option<KeyBinding> {
        Some(match self {
            Action::Open => KeyBinding::new(KeyCode::KeyO, Modifiers::Ctrl),
            Action::Prev => KeyBinding::new(KeyCode::ArrowLeft, Modifiers::None),
            Action::Next => KeyBinding::new(KeyCode::ArrowRight, Modifiers::None),
            Action::RotateCw => KeyBinding::new(KeyCode::CloseBracket, Modifiers::Ctrl),
            Action::RotateCcw => KeyBinding::new(KeyCode::OpenBracket, Modifiers::Ctrl),
            Action::Fit => KeyBinding::new(KeyCode::KeyF, Modifiers::None),
            Action::Fullscreen => KeyBinding::new(KeyCode::F11, Modifiers::None),
            Action::ToggleTheme => KeyBinding::new(KeyCode::KeyT, Modifiers::Ctrl),
            Action::ToggleSidebar => KeyBinding::new(KeyCode::KeyH, Modifiers::None),
            Action::EditShortcuts => KeyBinding::new(KeyCode::KeyK, Modifiers::Ctrl),
        })
    }

    /// Todas las variantes en orden estable (para el editor de atajos).
    pub fn all() -> [Action; 10] {
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
            Action::EditShortcuts,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_a_label() {
        for action in Action::all() {
            assert!(!action.label().is_empty(), "{action:?} sin label");
        }
    }

    #[test]
    fn every_action_has_a_default_shortcut() {
        for action in Action::all() {
            assert!(
                action.default_shortcut().is_some(),
                "{action:?} sin atajo por defecto"
            );
        }
    }

    #[test]
    fn label_is_stable_and_descriptive() {
        assert_eq!(Action::Open.label(), "Abrir");
        assert_eq!(Action::Next.label(), "Siguiente");
        assert_eq!(Action::RotateCw.label(), "Rotar 90° CW");
        assert_eq!(Action::EditShortcuts.label(), "Configurar atajos");
    }

    #[test]
    fn all_returns_exactly_ten_actions() {
        let all = Action::all();
        assert_eq!(all.len(), 10);
        let unique: std::collections::HashSet<_> = all.into_iter().collect();
        assert_eq!(unique.len(), 10, "sin variantes duplicadas");
    }

    #[test]
    fn serde_roundtrip_uses_snake_case_names() {
        // TOML no serializa una variante unit suelta; se envuelve en un struct
        // (igual que hará `ShortcutMap` como campo de `Settings`).
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
