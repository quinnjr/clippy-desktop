//! The speech balloon: a pale Office-Assistant bubble above Clippy holding the
//! conversation and a prompt field. Pure egui; the caller owns the state and
//! decides what to do with the returned [`BalloonAction`].

use crate::chat::{ChatState, Status};

/// What the balloon wants the app to do after a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BalloonAction {
    None,
    /// The user submitted a prompt.
    Submit(String),
    /// The user closed the balloon.
    Close,
}

/// Where to place the balloon, in egui points.
pub struct BalloonLayout {
    /// Top-left of the balloon body.
    pub pos: egui::Pos2,
    /// Balloon body size.
    pub size: egui::Vec2,
    /// X the tail points down to (Clippy's centre).
    pub tail_x: f32,
    /// Y of the balloon's bottom edge (where the tail starts).
    pub tail_y: f32,
}

const PALE: egui::Color32 = egui::Color32::from_rgb(255, 252, 214);
const INK: egui::Color32 = egui::Color32::from_rgb(40, 38, 24);
const BORDER: egui::Color32 = egui::Color32::from_rgb(120, 116, 74);
const USER: egui::Color32 = egui::Color32::from_rgb(28, 66, 128);
const WARN: egui::Color32 = egui::Color32::from_rgb(150, 40, 40);
const SEND_WIDTH: f32 = 58.0;

/// Renders the balloon for one frame and reports what the user did.
pub fn show(
    ctx: &egui::Context,
    layout: &BalloonLayout,
    chat: &ChatState,
    input: &mut String,
    focus: &mut bool,
) -> BalloonAction {
    // Escape closes the balloon wherever the focus is.
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return BalloonAction::Close;
    }

    draw_tail(ctx, layout);

    let frame = egui::Frame::new()
        .fill(PALE)
        .stroke(egui::Stroke::new(1.5_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(10));

    let inner_width = layout.size.x - 20.0;

    let response = egui::Area::new(egui::Id::new("clippy_balloon"))
        .order(egui::Order::Foreground)
        .fixed_pos(layout.pos)
        .show(ctx, |ui| {
            let mut action = BalloonAction::None;
            ui.set_max_width(layout.size.x);
            frame.show(ui, |ui| {
                ui.set_width(inner_width);
                ui.visuals_mut().override_text_color = Some(INK);

                ui.horizontal(|ui| {
                    ui.strong("Clippy");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("\u{00d7}").on_hover_text("Close").clicked() {
                            action = BalloonAction::Close;
                        }
                    });
                });
                ui.separator();

                egui::ScrollArea::vertical()
                    .max_height((layout.size.y - 96.0).max(48.0))
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(inner_width);
                        transcript(ui, chat);
                    });

                ui.separator();

                if let Some(help) = chat.unconfigured_message() {
                    ui.label(egui::RichText::new(help).monospace().small());
                } else if let Some(submitted) = prompt_row(ui, inner_width, chat, input, focus) {
                    action = BalloonAction::Submit(submitted);
                }
            });
            action
        });

    response.inner
}

fn transcript(ui: &mut egui::Ui, chat: &ChatState) {
    let mut empty = true;
    for message in chat.turns() {
        empty = false;
        turn(ui, &message.role, &message.content);
    }
    if !chat.streaming().is_empty() {
        empty = false;
        turn(ui, "assistant", chat.streaming());
    } else if chat.is_busy() {
        ui.add_space(3.0);
        ui.label(egui::RichText::new("Clippy is thinking\u{2026}").italics());
    }
    if let Status::Error(message) = chat.status() {
        ui.add_space(3.0);
        ui.colored_label(WARN, format!("Error: {message}"));
    }
    if empty && !matches!(chat.status(), Status::Error(_)) && !chat.is_busy() {
        ui.weak("Ask me anything. I might even be useful.");
    }
}

fn turn(ui: &mut egui::Ui, role: &str, content: &str) {
    let (who, color) = if role == "user" {
        ("You", USER)
    } else {
        ("Clippy", INK)
    };
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(egui::RichText::new(format!("{who}:")).strong().color(color));
        ui.label(content);
    });
}

/// The input field and Send button. Returns the prompt when submitted.
fn prompt_row(
    ui: &mut egui::Ui,
    inner_width: f32,
    chat: &ChatState,
    input: &mut String,
    focus: &mut bool,
) -> Option<String> {
    let mut submitted = None;
    ui.horizontal(|ui| {
        let idle = !chat.is_busy();
        let hint = if idle {
            "Ask me something\u{2026}"
        } else {
            "\u{2026}"
        };
        let field = egui::TextEdit::singleline(input)
            .hint_text(hint)
            .desired_width(inner_width - SEND_WIDTH - 8.0);
        let response = ui.add_enabled(idle, field);
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

        let can_send = idle && !input.trim().is_empty();
        let clicked = ui
            .add_enabled(
                can_send,
                egui::Button::new("Send").min_size(egui::vec2(SEND_WIDTH, 0.0)),
            )
            .clicked();

        if (enter || clicked) && !input.trim().is_empty() {
            submitted = Some(input.trim().to_string());
            input.clear();
            response.request_focus();
        } else if *focus {
            response.request_focus();
        }
    });
    *focus = false;
    submitted
}

/// Draws the downward tail from the balloon to Clippy, under the balloon body.
fn draw_tail(ctx: &egui::Context, layout: &BalloonLayout) {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let tip = egui::pos2(layout.tail_x, layout.tail_y + 13.0);
    let left = egui::pos2(layout.tail_x - 11.0, layout.tail_y - 1.0);
    let right = egui::pos2(layout.tail_x + 11.0, layout.tail_y - 1.0);
    painter.add(egui::Shape::convex_polygon(
        vec![left, right, tip],
        PALE,
        egui::Stroke::NONE,
    ));
    painter.line_segment([left, tip], egui::Stroke::new(1.5_f32, BORDER));
    painter.line_segment([right, tip], egui::Stroke::new(1.5_f32, BORDER));
}
