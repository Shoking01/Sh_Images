//! Atajos de teclado configurables (implementación completa en Task 3).

use serde::{Deserialize, Serialize};

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
