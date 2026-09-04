//! The settings modal: a card, over a dimmed backdrop, for the chat server and
//! appearance. Pure egui; the app owns the [`SettingsDraft`] and decides what
//! to do with the returned [`SettingsAction`].

use crate::options::{MAX_SCALE, MIN_SCALE};

/// The editable copy of the settings while the modal is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsDraft {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub scale: u32,
    pub always_on_top: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsAction {
    None,
    Save,
    Cancel,
}

const CARD: egui::Color32 = egui::Color32::from_rgb(247, 246, 242);
const INK: egui::Color32 = egui::Color32::from_rgb(32, 30, 22);
const BORDER: egui::Color32 = egui::Color32::from_rgb(120, 116, 92);
const WARN: egui::Color32 = egui::Color32::from_rgb(150, 40, 40);

/// Renders the modal for one frame and reports what the user did. `error` is a
/// message from a failed save, if any.
pub fn show(
    ctx: &egui::Context,
    pos: egui::Pos2,
    size: egui::Vec2,
    draft: &mut SettingsDraft,
    config_path: &str,
    error: Option<&str>,
    focus_first: &mut bool,
) -> SettingsAction {
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return SettingsAction::Cancel;
    }

    // A dimmed backdrop over the whole (otherwise transparent) window.
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        ctx.content_rect(),
        0.0,
        egui::Color32::from_black_alpha(110),
    );

    let frame = egui::Frame::new()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.5_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(12));

    let response = egui::Area::new(egui::Id::new("clippy_settings"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let mut action = SettingsAction::None;
            ui.set_max_width(size.x);
            frame.show(ui, |ui| {
                ui.set_width(size.x - 24.0);
                ui.visuals_mut().override_text_color = Some(INK);

                ui.strong("Settings");
                ui.separator();

                ui.label(egui::RichText::new("Chat server").strong());
                ui.label(
                    egui::RichText::new("Point Clippy at any OpenAI-compatible server.").small(),
                );
                let base = grid_field(
                    ui,
                    "Base URL",
                    &mut draft.base_url,
                    "http://localhost:11434/v1",
                    false,
                );
                if *focus_first {
                    base.request_focus();
                    *focus_first = false;
                }
                grid_field(
                    ui,
                    "Model",
                    &mut draft.model,
                    "llama3.2 or gpt-4o-mini",
                    false,
                );
                grid_field(ui, "API key", &mut draft.api_key, "(optional)", true);

                ui.add_space(8.0);
                ui.label(egui::RichText::new("Appearance").strong());
                ui.add(egui::Slider::new(&mut draft.scale, MIN_SCALE..=MAX_SCALE).text("Size"));
                ui.checkbox(&mut draft.always_on_top, "Always on top");

                if let Some(error) = error {
                    ui.add_space(6.0);
                    ui.colored_label(WARN, format!("Could not save: {error}"));
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        action = SettingsAction::Save;
                    }
                    if ui.button("Cancel").clicked() {
                        action = SettingsAction::Cancel;
                    }
                });

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("Saved to {config_path}"))
                        .small()
                        .weak(),
                );
                ui.label(
                    egui::RichText::new("Command-line flags and OPENAI_* override the file.")
                        .small()
                        .weak(),
                );
            });
            action
        });

    response.inner
}

/// A labelled single-line field on its own row.
fn grid_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    password: bool,
) -> egui::Response {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(label).small());
    let field = egui::TextEdit::singleline(value)
        .hint_text(hint)
        .password(password)
        .desired_width(f32::INFINITY);
    ui.add(field)
}
