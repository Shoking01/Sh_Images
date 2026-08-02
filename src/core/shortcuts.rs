//! Atajos de teclado configurables (implementación completa en Task 3).

use serde::{Deserialize, Serialize};

use crate::core::actions::Action;

/// Tecla de un atajo (subconjunto serializable de `egui::Key`, sin egui en core).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyCode {
    ArrowLeft,
    ArrowRight,
    KeyF,
    KeyH,
    KeyK,
    KeyO,
    KeyT,
    OpenBracket,
    CloseBracket,
    F11,
}

/// Modificadores de un atajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifiers {
    /// Sin modificadores.
    None,
    /// Ctrl.
    Ctrl,
    /// Ctrl + Shift.
    CtrlShift,
}

/// Combinación de tecla + modificadores de un atajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: Modifiers,
}

impl KeyBinding {
    /// Crea un binding.
    pub fn new(key: KeyCode, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}

/// Error de un atajo inválido.
#[derive(Debug, thiserror::Error)]
pub enum ShortcutError {
    /// El binding ya está asignado a otra acción.
    #[error("shortcut already assigned to {0:?}")]
    Conflict(Action),
    /// Tecla no reconocida al parsear.
    #[error("unknown key: {0}")]
    InvalidKey(String),
    /// Input vacío al parsear.
    #[error("empty shortcut")]
    Empty,
}

/// Mapa acción → atajo, serializable en `settings.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutMap {
    bindings: std::collections::BTreeMap<Action, KeyBinding>,
}

impl Default for ShortcutMap {
    fn default() -> Self {
        Self::defaults()
    }
}

impl ShortcutMap {
    /// Mapa con los atajos por defecto (una entrada por acción).
    pub fn defaults() -> Self {
        let bindings = Action::all()
            .into_iter()
            .filter_map(|a| a.default_shortcut().map(|b| (a, b)))
            .collect();
        Self { bindings }
    }

    /// Devuelve el binding de `action`, o `None` si no está mapeada.
    pub fn get(&self, action: Action) -> Option<&KeyBinding> {
        self.bindings.get(&action)
    }

    /// Asigna `binding` a `action`.
    ///
    /// # Errors
    /// * `ShortcutError::Conflict(other)` si `binding` ya lo usa `other`.
    pub fn assign(&mut self, action: Action, binding: KeyBinding) -> Result<(), ShortcutError> {
        if let Some((other, _)) = self
            .bindings
            .iter()
            .find(|(a, b)| **b == binding && **a != action)
        {
            return Err(ShortcutError::Conflict(*other));
        }
        self.bindings.insert(action, binding);
        Ok(())
    }

    /// Devuelve la acción que usa `binding`, o `None`.
    pub fn action_for(&self, binding: KeyBinding) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(_, b)| **b == binding)
            .map(|(a, _)| *a)
    }

    /// Vuelve a los atajos por defecto.
    pub fn reset(&mut self) {
        *self = Self::defaults();
    }

    /// Iterador sobre las `(Action, KeyBinding)` del mapa (orden estable).
    pub fn iter(&self) -> impl Iterator<Item = (Action, KeyBinding)> + '_ {
        self.bindings.iter().map(|(a, b)| (*a, *b))
    }
}

impl KeyCode {
    /// Nombre corto para `KeyBinding::to_string` (`"→"`, `"F"`, `"F11"`).
    fn to_str(self) -> &'static str {
        match self {
            KeyCode::ArrowLeft => "←",
            KeyCode::ArrowRight => "→",
            KeyCode::KeyF => "F",
            KeyCode::KeyH => "H",
            KeyCode::KeyK => "K",
            KeyCode::KeyO => "O",
            KeyCode::KeyT => "T",
            KeyCode::OpenBracket => "[",
            KeyCode::CloseBracket => "]",
            KeyCode::F11 => "F11",
        }
    }

    /// Parsea el nombre corto devuelto por `to_str`.
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "←" => Some(KeyCode::ArrowLeft),
            "→" => Some(KeyCode::ArrowRight),
            "F" => Some(KeyCode::KeyF),
            "H" => Some(KeyCode::KeyH),
            "K" => Some(KeyCode::KeyK),
            "O" => Some(KeyCode::KeyO),
            "T" => Some(KeyCode::KeyT),
            "[" => Some(KeyCode::OpenBracket),
            "]" => Some(KeyCode::CloseBracket),
            "F11" => Some(KeyCode::F11),
            _ => None,
        }
    }
}

impl KeyBinding {
    /// Representación mostrada en la UI: `"Ctrl+O"`, `"→"`, `"F11"`.
    pub fn to_string(&self) -> String {
        let mods = match self.modifiers {
            Modifiers::None => "",
            Modifiers::Ctrl => "Ctrl+",
            Modifiers::CtrlShift => "Ctrl+Shift+",
        };
        format!("{mods}{}", self.key.to_str())
    }

    /// Parsea la representación de `to_string` (`"Ctrl+O"`, `"→"`, `"F11"`).
    ///
    /// # Errors
    /// * `ShortcutError::Empty` si el input está vacío.
    /// * `ShortcutError::InvalidKey` si la tecla no se reconoce.
    pub fn parse(input: &str) -> Result<Self, ShortcutError> {
        if input.trim().is_empty() {
            return Err(ShortcutError::Empty);
        }
        let (mods, key_part) = if let Some(rest) = input.strip_prefix("Ctrl+Shift+") {
            (Modifiers::CtrlShift, rest)
        } else if let Some(rest) = input.strip_prefix("Ctrl+") {
            (Modifiers::Ctrl, rest)
        } else {
            (Modifiers::None, input)
        };
        let key = KeyCode::from_str(key_part).ok_or_else(|| {
            ShortcutError::InvalidKey(key_part.to_string())
        })?;
        Ok(KeyBinding::new(key, mods))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_has_one_entry_per_action() {
        let map = ShortcutMap::defaults();
        assert_eq!(map.iter().count(), 10);
        for action in Action::all() {
            assert!(map.get(action).is_some(), "{action:?} sin binding default");
        }
    }

    #[test]
    fn default_shortcuts_match_spec() {
        let map = ShortcutMap::defaults();
        assert_eq!(map.get(Action::Open).unwrap().to_string(), "Ctrl+O");
        assert_eq!(map.get(Action::Prev).unwrap().to_string(), "←");
        assert_eq!(map.get(Action::Next).unwrap().to_string(), "→");
        assert_eq!(map.get(Action::RotateCw).unwrap().to_string(), "Ctrl+]");
        assert_eq!(map.get(Action::RotateCcw).unwrap().to_string(), "Ctrl+[");
        assert_eq!(map.get(Action::Fit).unwrap().to_string(), "F");
        assert_eq!(map.get(Action::Fullscreen).unwrap().to_string(), "F11");
        assert_eq!(map.get(Action::ToggleTheme).unwrap().to_string(), "Ctrl+T");
        assert_eq!(map.get(Action::ToggleSidebar).unwrap().to_string(), "H");
        assert_eq!(map.get(Action::EditShortcuts).unwrap().to_string(), "Ctrl+K");
    }

    #[test]
    fn assign_replaces_binding_without_conflict() {
        let mut map = ShortcutMap::defaults();
        map.assign(Action::Next, KeyBinding::new(KeyCode::KeyF, Modifiers::Ctrl))
            .expect("sin conflicto");
        assert_eq!(map.get(Action::Next).unwrap().to_string(), "Ctrl+F");
    }

    #[test]
    fn assign_conflict_returns_error_and_keeps_original() {
        let mut map = ShortcutMap::defaults();
        let original = map.get(Action::Next).copied().expect("binding");
        let err = map
            .assign(Action::Next, KeyBinding::new(KeyCode::KeyO, Modifiers::Ctrl))
            .expect_err("Ctrl+O ya lo usa Open");
        assert!(matches!(err, ShortcutError::Conflict(Action::Open)));
        assert_eq!(map.get(Action::Next).copied(), Some(original), "no se mutó");
    }

    #[test]
    fn assign_to_same_action_same_binding_is_ok() {
        let mut map = ShortcutMap::defaults();
        let binding = map.get(Action::Fit).copied().expect("binding");
        map.assign(Action::Fit, binding).expect("re-asignar mismo");
    }

    #[test]
    fn action_for_reverse_lookup() {
        let map = ShortcutMap::defaults();
        assert_eq!(
            map.action_for(KeyBinding::new(KeyCode::KeyT, Modifiers::Ctrl)),
            Some(Action::ToggleTheme)
        );
        assert_eq!(
            map.action_for(KeyBinding::new(KeyCode::F11, Modifiers::None)),
            Some(Action::Fullscreen)
        );
        assert_eq!(
            map.action_for(KeyBinding::new(KeyCode::KeyF, Modifiers::Ctrl)),
            None
        );
    }

    #[test]
    fn reset_restores_defaults() {
        let mut map = ShortcutMap::defaults();
        map.assign(Action::Next, KeyBinding::new(KeyCode::KeyF, Modifiers::Ctrl))
            .expect("asignar");
        map.reset();
        assert_eq!(map, ShortcutMap::defaults());
    }

    #[test]
    fn to_string_and_parse_roundtrip() {
        for (_, binding) in ShortcutMap::defaults().iter() {
            let s = binding.to_string();
            assert_eq!(KeyBinding::parse(&s).expect("parse"), binding, "roundtrip de {s}");
        }
    }

    #[test]
    fn parse_empty_returns_empty_error() {
        assert!(matches!(
            KeyBinding::parse("   "),
            Err(ShortcutError::Empty)
        ));
    }

    #[test]
    fn parse_unknown_key_returns_invalid_key() {
        assert!(matches!(
            KeyBinding::parse("Ctrl+Z"),
            Err(ShortcutError::InvalidKey(_))
        ));
    }

    #[test]
    fn serde_roundtrip_preserves_bindings() {
        let map = ShortcutMap::defaults();
        let s = toml::to_string(&map).expect("serializar");
        let back: ShortcutMap = toml::from_str(&s).expect("deserializar");
        assert_eq!(back, map);
    }

    /// Congela el mapa default completo para detectar cambios no intencionados.
    #[test]
    fn snapshot_default_shortcuts_map() {
        let map = ShortcutMap::defaults();
        let s = toml::to_string(&map).expect("serializar");
        insta::assert_snapshot!(s);
    }

    /// Congela la representación mostrada en la UI de todos los defaults.
    #[test]
    fn snapshot_default_keybinding_strings() {
        let strings: Vec<String> = ShortcutMap::defaults()
            .iter()
            .map(|(a, b)| format!("{} -> {}", a.label(), b.to_string()))
            .collect();
        insta::assert_snapshot!(strings.join("\n"));
    }
}
