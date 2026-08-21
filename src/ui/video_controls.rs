//! Barra de controles de reproducción.
//!
//! Vive en su propio panel, encima de la barra de estado. Es deliberado: si los
//! botones estuvieran sobre el área del visor, `viewer::show` los contaría como
//! zoom o arrastre y `pause_slideshow` se dispararía en cada clic.
//!
//! Compila en las tres plataformas: sólo dibuja a partir de una
//! `PlayerSnapshot` y devuelve la intención del usuario, sin tocar el backend.

use eframe::egui;

use crate::core::lang::Language;
use crate::core::video::playback::PlaybackState;
use crate::core::video::{format, seek, PlayerSnapshot};

/// Alto del panel, en la línea de los 24-30 px del resto de barras.
const PANEL_HEIGHT: f32 = 32.0;
const BUTTON_SIZE: f32 = 22.0;

/// Lo que el usuario pidió en este frame.
#[derive(Debug, Default, PartialEq)]
pub struct VideoControlsResponse {
    pub toggle_play: bool,
    pub seek_forward: bool,
    pub seek_backward: bool,
    pub toggle_mute: bool,
    /// Posición absoluta pedida con la barra de progreso.
    pub seek_to: Option<std::time::Duration>,
    /// Volumen nuevo pedido con el deslizador.
    pub set_volume: Option<u8>,
}

impl VideoControlsResponse {
    /// `true` si hubo alguna interacción, para no persistir ajustes de más.
    pub fn any(&self) -> bool {
        *self != Self::default()
    }
}

/// Dibuja la barra y devuelve lo que pidió el usuario.
pub fn show(ui: &mut egui::Ui, snapshot: &PlayerSnapshot, lang: Language) -> VideoControlsResponse {
    let t = lang.translations();
    let mut out = VideoControlsResponse::default();

    ui.horizontal_centered(|ui| {
        ui.add_space(6.0);

        let playing = snapshot.state == PlaybackState::Playing;
        if icon_button(
            ui,
            if playing { Icon::Pause } else { Icon::Play },
            t.video_play_pause,
        ) {
            out.toggle_play = true;
        }

        // Sin índice de búsqueda no se puede saltar: mejor deshabilitar los
        // botones que fallar en silencio.
        ui.add_enabled_ui(snapshot.info.can_seek, |ui| {
            if icon_button(ui, Icon::Back5, t.video_seek_backward) {
                out.seek_backward = true;
            }
            if icon_button(ui, Icon::Forward5, t.video_seek_forward) {
                out.seek_forward = true;
            }
        });

        ui.add_space(8.0);
        ui.label(format::format_position(
            snapshot.position,
            snapshot.duration(),
        ));
        ui.add_space(8.0);

        // El deslizador de volumen se ancla a la derecha; la barra de progreso
        // se queda con el espacio restante.
        egui::Sides::new().show(
            ui,
            |ui| {
                ui.add_enabled_ui(snapshot.info.can_seek, |ui| {
                    let mut fraction =
                        seek::progress_fraction(snapshot.position, snapshot.duration());
                    let slider = egui::Slider::new(&mut fraction, 0.0..=1.0)
                        .show_value(false)
                        .handle_shape(egui::style::HandleShape::Circle);
                    let resp = ui.add_sized([ui.available_width() - 8.0, 18.0], slider);
                    if resp.changed() {
                        out.seek_to =
                            Some(seek::position_from_fraction(fraction, snapshot.duration()));
                    }
                });
            },
            |ui| {
                if !snapshot.info.has_audio {
                    ui.add_space(4.0);
                    ui.weak(t.video_no_audio);
                    return;
                }
                let mut vol = i32::from(snapshot.volume);
                let resp = ui.add_sized(
                    [90.0, 18.0],
                    egui::Slider::new(&mut vol, 0..=100).show_value(false),
                );
                if resp.changed() {
                    out.set_volume = Some(vol.clamp(0, 100) as u8);
                }
                resp.on_hover_text(format!("{} {}%", t.video_volume, snapshot.volume));
                if icon_button(
                    ui,
                    if snapshot.muted {
                        Icon::Muted
                    } else {
                        Icon::Sound
                    },
                    t.video_toggle_mute,
                ) {
                    out.toggle_mute = true;
                }
            },
        );
    });

    out
}

/// Alto que hay que reservar para el panel.
pub fn panel_height() -> f32 {
    PANEL_HEIGHT
}

/// Iconos dibujados a mano, como en `toolbar_icons`.
///
/// Sólo primitivas simples: el encabezado de `toolbar_icons.rs` advierte que
/// `convex_polygon` con geometría degenerada rompe el frame entero.
#[derive(Clone, Copy)]
enum Icon {
    Play,
    Pause,
    Forward5,
    Back5,
    Sound,
    Muted,
}

fn stroke_width(r: f32) -> f32 {
    (r * 0.13).clamp(1.1, 2.2)
}

fn icon_button(ui: &mut egui::Ui, icon: Icon, tooltip: &str) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::Vec2::splat(BUTTON_SIZE), egui::Sense::click());
    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, visuals.corner_radius, visuals.bg_fill);
    }
    paint_icon(icon, painter, rect.shrink(4.0), visuals.text_color());
    response.clone().on_hover_text(tooltip);
    response.clicked()
}

fn paint_icon(icon: Icon, painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.5;
    let w = stroke_width(r);
    let stroke = egui::Stroke::new(w, color);
    match icon {
        Icon::Play => {
            // Triángulo por líneas, no por polígono relleno.
            let a = egui::pos2(c.x - r * 0.45, c.y - r * 0.7);
            let b = egui::pos2(c.x - r * 0.45, c.y + r * 0.7);
            let tip = egui::pos2(c.x + r * 0.7, c.y);
            painter.add(egui::Shape::line(vec![a, tip, b, a], stroke));
        }
        Icon::Pause => {
            let half = r * 0.65;
            let bar = r * 0.28;
            for dx in [-r * 0.38, r * 0.38] {
                painter.rect_filled(
                    egui::Rect::from_center_size(
                        egui::pos2(c.x + dx, c.y),
                        egui::vec2(bar, half * 2.0),
                    ),
                    1.0,
                    color,
                );
            }
        }
        Icon::Forward5 | Icon::Back5 => {
            let dir = if matches!(icon, Icon::Forward5) {
                1.0
            } else {
                -1.0
            };
            // Dos puntas de flecha: se lee como "saltar", no como "siguiente".
            for offset in [-r * 0.45, r * 0.15] {
                let x = c.x + offset * dir;
                painter.add(egui::Shape::line(
                    vec![
                        egui::pos2(x, c.y - r * 0.55),
                        egui::pos2(x + r * 0.45 * dir, c.y),
                        egui::pos2(x, c.y + r * 0.55),
                    ],
                    stroke,
                ));
            }
        }
        Icon::Sound | Icon::Muted => {
            // Cuerpo del altavoz.
            painter.rect_filled(
                egui::Rect::from_center_size(
                    egui::pos2(c.x - r * 0.45, c.y),
                    egui::vec2(r * 0.35, r * 0.6),
                ),
                0.0,
                color,
            );
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(c.x - r * 0.3, c.y - r * 0.3),
                    egui::pos2(c.x + r * 0.05, c.y - r * 0.75),
                    egui::pos2(c.x + r * 0.05, c.y + r * 0.75),
                    egui::pos2(c.x - r * 0.3, c.y + r * 0.3),
                ],
                stroke,
            ));
            if matches!(icon, Icon::Muted) {
                painter.line_segment(
                    [
                        egui::pos2(c.x + r * 0.3, c.y - r * 0.4),
                        egui::pos2(c.x + r * 0.8, c.y + r * 0.4),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x + r * 0.8, c.y - r * 0.4),
                        egui::pos2(c.x + r * 0.3, c.y + r * 0.4),
                    ],
                    stroke,
                );
            } else {
                painter.circle_stroke(egui::pos2(c.x + r * 0.25, c.y), r * 0.35, stroke);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::video::VideoInfo;
    use std::time::Duration;

    fn snapshot() -> PlayerSnapshot {
        PlayerSnapshot {
            state: PlaybackState::Playing,
            position: Duration::from_secs(30),
            info: VideoInfo {
                width: 1920,
                height: 1080,
                duration: Some(Duration::from_secs(120)),
                fps: Some(30.0),
                has_audio: true,
                container_rotation: 0,
                can_seek: true,
                hw_active: false,
            },
            volume: 80,
            muted: false,
            error: None,
        }
    }

    #[test]
    fn default_response_reports_no_interaction() {
        assert!(!VideoControlsResponse::default().any());
    }

    #[test]
    fn any_detects_each_kind_of_interaction() {
        assert!(VideoControlsResponse {
            toggle_play: true,
            ..Default::default()
        }
        .any());
        assert!(VideoControlsResponse {
            seek_to: Some(Duration::from_secs(5)),
            ..Default::default()
        }
        .any());
        assert!(VideoControlsResponse {
            set_volume: Some(50),
            ..Default::default()
        }
        .any());
    }

    #[test]
    fn panel_height_matches_the_other_bars() {
        // La barra de estado usa 24 y la de herramientas 30; ésta no debe
        // desentonar ni comerse el visor.
        assert!((24.0..=40.0).contains(&panel_height()));
    }

    #[test]
    fn snapshot_exposes_the_duration_for_the_progress_bar() {
        let s = snapshot();
        assert_eq!(s.duration(), Some(Duration::from_secs(120)));
        assert_eq!(seek::progress_fraction(s.position, s.duration()), 0.25);
    }

    #[test]
    fn stroke_width_stays_within_readable_bounds() {
        for r in [4.0f32, 9.0, 40.0] {
            let w = stroke_width(r);
            assert!((1.1..=2.2).contains(&w), "grosor {w} para r={r}");
        }
    }
}
