//! Overlay de notificaciones (toasts) dibujado con egui.

use eframe::egui;

/// Duración de un toast en segundos.
pub const TOAST_SECONDS: f64 = 3.0;

struct Toast {
    message: String,
    expires_at: f64,
}

/// Colección de toasts visibles.
#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    /// Crea una colección vacía.
    pub fn new() -> Self {
        Self::default()
    }

    /// Añade un toast que expira `TOAST_SECONDS` después de `now` (segundos de egui).
    pub fn push(&mut self, message: impl Into<String>, now: f64) {
        self.items.push(Toast {
            message: message.into(),
            expires_at: now + TOAST_SECONDS,
        });
    }

    /// Elimina los toasts expirados en `now`.
    pub fn update(&mut self, now: f64) {
        self.items.retain(|t| t.expires_at > now);
    }

    /// Devuelve `true` si no hay toasts visibles.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Dibuja los toasts en la esquina inferior derecha.
    pub fn show(&self, ui: &mut egui::Ui) {
        if self.items.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                for toast in &self.items {
                    ui.label(&toast.message);
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_adds_toast_with_expiry() {
        let mut toasts = Toasts::new();
        toasts.push("test", 10.0);
        assert_eq!(toasts.items.len(), 1);
        assert_eq!(toasts.items[0].message, "test");
        assert!((toasts.items[0].expires_at - 13.0).abs() < 1e-9);
    }

    #[test]
    fn update_removes_expired_toasts() {
        let mut toasts = Toasts::new();
        toasts.push("one", 10.0);
        toasts.push("two", 12.0);
        toasts.update(15.1);
        assert!(toasts.is_empty());
    }

    #[test]
    fn update_keeps_unexpired_toasts() {
        let mut toasts = Toasts::new();
        toasts.push("one", 10.0);
        toasts.push("two", 14.0);
        toasts.update(13.1);
        assert_eq!(toasts.items.len(), 1);
        assert_eq!(toasts.items[0].message, "two");
    }
}
