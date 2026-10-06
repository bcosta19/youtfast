//! Pieces shared by the result grid and the watch page.

use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Image, Rect, Response, RichText, Sense, Ui, vec2,
};

use crate::app::State;
use crate::model::{Action, Channel, Video};
use crate::util;

/// A thumbnail at `size`, with the length (or LIVE) in its corner.
pub fn thumbnail(ui: &mut Ui, video: &Video, size: egui::Vec2) -> Response {
    let response = ui.add(
        Image::new(video.thumbnail())
            .fit_to_exact_size(size)
            .corner_radius(CornerRadius::same(10)),
    );
    badge(ui, response.rect, video);
    response
}

fn badge(ui: &Ui, thumbnail: Rect, video: &Video) {
    let (text, fill) = if video.live {
        ("LIVE".to_owned(), super::ACCENT)
    } else if let Some(seconds) = video.duration {
        (util::duration(seconds), Color32::from_black_alpha(200))
    } else {
        return;
    };
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(text, FontId::proportional(12.0), Color32::WHITE);
    let size = galley.size() + vec2(8.0, 4.0);
    let rect = Rect::from_min_size(thumbnail.right_bottom() - size - vec2(6.0, 6.0), size);
    painter.rect_filled(rect, CornerRadius::same(4), fill);
    painter.galley(rect.min + vec2(4.0, 2.0), galley, Color32::WHITE);
}

/// A title cut to two lines with an ellipsis.
pub fn title(ui: &mut Ui, text: &str, size: f32) {
    let mut job = LayoutJob::simple(
        text.to_owned(),
        FontId::proportional(size),
        ui.visuals().strong_text_color(),
        ui.available_width(),
    );
    job.wrap.max_rows = 2;
    ui.label(job);
}

/// "1.2M views · 2 days ago", or "Live now".
pub fn meta(video: &Video) -> String {
    if video.live {
        return "Live now".to_owned();
    }
    let parts: Vec<String> = [video.views.map(util::views), video.published.clone()]
        .into_iter()
        .flatten()
        .collect();
    parts.join(" · ")
}

/// A video as a row: thumbnail on the left, title and details beside it.
/// `note` is an extra line ("Watched 2 days ago").
pub fn compact_card(ui: &mut Ui, video: &Video, note: Option<&str>, actions: &mut Vec<Action>) {
    let row = ui.horizontal_top(|ui| {
        thumbnail(ui, video, vec2(168.0, 94.5));
        ui.vertical(|ui| {
            title(ui, &video.title, 14.0);
            if let Some(channel) = &video.channel {
                ui.label(RichText::new(channel).weak().small());
            }
            let meta = meta(video);
            if !meta.is_empty() {
                ui.label(RichText::new(meta).weak().small());
            }
            if let Some(note) = note {
                ui.label(RichText::new(note).weak().small());
            }
        });
    });
    open_on_click(ui, row.response.rect, video, actions);
}

/// A channel as a row: its avatar when known, and its name, which opens it.
/// `trailing` adds controls at the end of the row.
pub fn channel_row(
    ui: &mut Ui,
    channel: &Channel,
    actions: &mut Vec<Action>,
    trailing: impl FnOnce(&mut Ui, &mut Vec<Action>),
) {
    ui.horizontal(|ui| {
        let size = vec2(40.0, 40.0);
        match &channel.avatar {
            Some(avatar) => {
                ui.add(
                    Image::new(avatar.clone())
                        .fit_to_exact_size(size)
                        .corner_radius(CornerRadius::same(20)),
                );
            }
            None => {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                ui.painter()
                    .circle_filled(rect.center(), 20.0, ui.visuals().faint_bg_color);
                let initial: String = channel.name.chars().take(1).collect();
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    initial.to_uppercase(),
                    FontId::proportional(18.0),
                    ui.visuals().strong_text_color(),
                );
            }
        }
        if ui.link(RichText::new(&channel.name).size(15.0)).clicked() {
            actions.push(Action::OpenChannel(channel.clone()));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            trailing(ui, actions);
        });
    });
}

/// Makes the area a finished card covers open its video when clicked.
pub fn open_on_click(ui: &mut Ui, rect: Rect, video: &Video, actions: &mut Vec<Action>) {
    let response = ui.interact(rect, ui.id().with(("video", &video.id)), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked() {
        actions.push(Action::Open(video.clone()));
    }
}

/// Subscribe on this device, or show that the account already does
/// (subscribing the account itself needs youtube.com: rustypipe has no
/// action for it).
pub fn subscribe_button(ui: &mut Ui, state: &State, channel: &Channel, actions: &mut Vec<Action>) {
    if state.subscribed_in_account(&channel.id) {
        ui.add_enabled(false, egui::Button::new("Subscribed"))
            .on_disabled_hover_text("Your account is subscribed. Manage it on youtube.com.");
        return;
    }
    let subscribed = state.library.is_subscribed(&channel.id);
    let button = if subscribed {
        egui::Button::new("Subscribed on this device")
    } else {
        egui::Button::new(RichText::new("Subscribe").color(Color32::WHITE)).fill(super::ACCENT)
    };
    let hint = if subscribed {
        "Unsubscribe on this computer"
    } else {
        "Follow this channel on this computer: its videos join your feed"
    };
    if ui.add(button).on_hover_text(hint).clicked() {
        actions.push(Action::ToggleSubscription(channel.clone()));
    }
}
