use serde::{Deserialize, Serialize};

use crate::core::actions::Action;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyCode {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Space,
    KeyM,
    KeyF,
    KeyH,
    KeyI,
    KeyK,
    KeyO,
    KeyT,
    OpenBracket,
    CloseBracket,
    F11,
    KeyF5,
    Comma,
    Period,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifiers {
    None,
    Ctrl,
    CtrlShift,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: Modifiers,
}

impl KeyBinding {
    pub fn new(key: KeyCode, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ShortcutError {
    #[error("shortcut already assigned to {0:?}")]
    Conflict(Action),
    #[error("unknown key: {0}")]
    InvalidKey(String),
    #[error("empty shortcut")]
    Empty,
}
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
    pub fn defaults() -> Self {
        let bindings = Action::all()
            .into_iter()
            .filter_map(|a| a.default_shortcut().map(|b| (a, b)))
            .collect();
        Self { bindings }
    }
    pub fn get(&self, action: Action) -> Option<&KeyBinding> {
        self.bindings.get(&action)
    }
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
    pub fn action_for(&self, binding: KeyBinding) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(_, b)| **b == binding)
            .map(|(a, _)| *a)
    }
    pub fn reset(&mut self) {
        *self = Self::defaults();
    }
    /// Asigna su atajo por defecto a las acciones que no tengan ninguno.
    ///
    /// Necesario tras cargar un `settings.toml` de una versión anterior: serde
    /// deserializa el mapa guardado tal cual, así que las acciones añadidas
    /// después se quedarían sin atajo hasta que el usuario pulsara
    /// "restablecer todos". Respeta las personalizaciones existentes y nunca
    /// crea un conflicto: si el atajo por defecto ya está ocupado, se deja la
    /// acción sin asignar.
    pub fn fill_missing_defaults(&mut self) -> usize {
        let mut added = 0;
        for action in Action::all() {
            if self.bindings.contains_key(&action) {
                continue;
            }
            let Some(binding) = action.default_shortcut() else {
                continue;
            };
            if self.action_for(binding).is_some() {
                tracing::debug!(
                    ?action,
                    "el atajo por defecto ya está en uso; la acción queda sin asignar"
                );
                continue;
            }
            self.bindings.insert(action, binding);
            added += 1;
        }
        added
    }
    pub fn iter(&self) -> impl Iterator<Item = (Action, KeyBinding)> + '_ {
        self.bindings.iter().map(|(a, b)| (*a, *b))
    }
}

impl KeyCode {
    fn to_str(self) -> &'static str {
        match self {
            KeyCode::ArrowLeft => "←",
            KeyCode::ArrowRight => "→",
            KeyCode::ArrowUp => "↑",
            KeyCode::ArrowDown => "↓",
            KeyCode::Space => "Espacio",
            KeyCode::KeyM => "M",
            KeyCode::KeyF => "F",
            KeyCode::KeyH => "H",
            KeyCode::KeyI => "I",
            KeyCode::KeyK => "K",
            KeyCode::KeyO => "O",
            KeyCode::KeyT => "T",
            KeyCode::OpenBracket => "[",
            KeyCode::CloseBracket => "]",
            KeyCode::F11 => "F11",
            KeyCode::KeyF5 => "F5",
            KeyCode::Comma => ",",
            KeyCode::Period => ".",
        }
    }
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "←" => Some(KeyCode::ArrowLeft),
            "→" => Some(KeyCode::ArrowRight),
            "↑" => Some(KeyCode::ArrowUp),
            "↓" => Some(KeyCode::ArrowDown),
            "Espacio" => Some(KeyCode::Space),
            "M" => Some(KeyCode::KeyM),
            "F" => Some(KeyCode::KeyF),
            "H" => Some(KeyCode::KeyH),
            "I" => Some(KeyCode::KeyI),
            "K" => Some(KeyCode::KeyK),
            "O" => Some(KeyCode::KeyO),
            "T" => Some(KeyCode::KeyT),
            "[" => Some(KeyCode::OpenBracket),
            "]" => Some(KeyCode::CloseBracket),
            "F11" => Some(KeyCode::F11),
            "F5" => Some(KeyCode::KeyF5),
            "," => Some(KeyCode::Comma),
            "." => Some(KeyCode::Period),
            _ => None,
        }
    }
}

impl std::fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mods = match self.modifiers {
            Modifiers::None => "",
            Modifiers::Ctrl => "Ctrl+",
            Modifiers::CtrlShift => "Ctrl+Shift+",
        };
        write!(f, "{mods}{}", self.key.to_str())
    }
}

impl KeyBinding {
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
        let key = KeyCode::from_str(key_part)
            .ok_or_else(|| ShortcutError::InvalidKey(key_part.to_string()))?;
        Ok(KeyBinding::new(key, mods))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_no_duplicate_bindings() {
        // `ShortcutMap::defaults()` usa `.collect()` sin detectar conflictos:
        // dos acciones podrían compartir combinación y `action_for` devolvería
        // sólo la primera, en silencio. Este es el único test que lo caza.
        let map = ShortcutMap::defaults();
        let mut seen: std::collections::HashMap<KeyBinding, Action> =
            std::collections::HashMap::new();
        for (action, binding) in map.iter() {
            if let Some(other) = seen.insert(binding, action) {
                panic!("{action:?} y {other:?} comparten el atajo {binding}");
            }
        }
    }

    #[test]
    fn fill_missing_defaults_recovers_new_actions_in_an_old_config() {
        // Simula un settings.toml de una versión anterior: sin esto, las
        // acciones añadidas después se quedan sin atajo para siempre.
        let mut map = ShortcutMap::defaults();
        let removed = map.bindings.remove(&Action::VideoPlayPause);
        assert!(removed.is_some());
        assert!(map.get(Action::VideoPlayPause).is_none());

        let added = map.fill_missing_defaults();
        assert_eq!(added, 1);
        assert_eq!(
            map.get(Action::VideoPlayPause),
            Action::VideoPlayPause.default_shortcut().as_ref()
        );
    }

    #[test]
    fn fill_missing_defaults_preserves_user_customizations() {
        let mut map = ShortcutMap::defaults();
        let custom = KeyBinding::new(KeyCode::KeyM, Modifiers::CtrlShift);
        map.bindings.insert(Action::VideoPlayPause, custom);
        map.fill_missing_defaults();
        assert_eq!(
            map.get(Action::VideoPlayPause),
            Some(&custom),
            "no debe pisar lo que el usuario configuró"
        );
    }

    #[test]
    fn fill_missing_defaults_never_creates_a_conflict() {
        let mut map = ShortcutMap::defaults();
        map.bindings.remove(&Action::VideoPlayPause);
        // Otra acción ocupa ya el atajo por defecto de VideoPlayPause.
        let space = KeyBinding::new(KeyCode::Space, Modifiers::None);
        map.bindings.insert(Action::Fit, space);
        map.fill_missing_defaults();
        assert!(
            map.get(Action::VideoPlayPause).is_none(),
            "prefiere dejarla sin asignar antes que duplicar el atajo"
        );
    }

    #[test]
    fn defaults_has_one_entry_per_action_with_shortcut() {
        let map = ShortcutMap::defaults();
        assert_eq!(map.iter().count(), 20);
        for action in Action::all() {
            if crate::core::actions::NO_SHORTCUT.contains(&action) {
                assert!(
                    map.get(action).is_none(),
                    "{action:?} no debe tener binding default"
                );
            } else {
                assert!(map.get(action).is_some(), "{action:?} sin binding default");
            }
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
        assert_eq!(map.get(Action::ToggleInfo).unwrap().to_string(), "I");
        assert_eq!(map.get(Action::ToggleSlideshow).unwrap().to_string(), "F5");
        assert_eq!(map.get(Action::SlideshowFaster).unwrap().to_string(), ",");
        assert_eq!(map.get(Action::SlideshowSlower).unwrap().to_string(), ".");
        assert_eq!(
            map.get(Action::EditShortcuts).unwrap().to_string(),
            "Ctrl+K"
        );
    }

    #[test]
    fn assign_replaces_binding_without_conflict() {
        let mut map = ShortcutMap::defaults();
        map.assign(
            Action::Next,
            KeyBinding::new(KeyCode::KeyF, Modifiers::Ctrl),
        )
        .expect("sin conflicto");
        assert_eq!(map.get(Action::Next).unwrap().to_string(), "Ctrl+F");
    }

    #[test]
    fn assign_conflict_returns_error_and_keeps_original() {
        let mut map = ShortcutMap::defaults();
        let original = map.get(Action::Next).copied().expect("binding");
        let err = map
            .assign(
                Action::Next,
                KeyBinding::new(KeyCode::KeyO, Modifiers::Ctrl),
            )
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
        map.assign(
            Action::Next,
            KeyBinding::new(KeyCode::KeyF, Modifiers::Ctrl),
        )
        .expect("asignar");
        map.reset();
        assert_eq!(map, ShortcutMap::defaults());
    }

    #[test]
    fn to_string_and_parse_roundtrip() {
        for (_, binding) in ShortcutMap::defaults().iter() {
            let s = binding.to_string();
            assert_eq!(
                KeyBinding::parse(&s).expect("parse"),
                binding,
                "roundtrip de {s}"
            );
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
    #[test]
    fn snapshot_default_shortcuts_map() {
        let map = ShortcutMap::defaults();
        let s = toml::to_string(&map).expect("serializar");
        insta::assert_snapshot!(s);
    }
    #[test]
    fn snapshot_default_keybinding_strings() {
        let strings: Vec<String> = ShortcutMap::defaults()
            .iter()
            .map(|(a, b)| format!("{} -> {}", a.label(crate::core::lang::Language::Es), b))
            .collect();
        insta::assert_snapshot!(strings.join("\n"));
    }
}
