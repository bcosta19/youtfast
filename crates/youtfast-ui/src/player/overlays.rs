//! Player overlays: seek bubbles, OSD pills, SponsorBlock skip button, and autoplay countdown.

use std::time::Instant;

use egui::{
    Align, Color32, CornerRadius, CursorIcon, Layout, Rect, RichText, Sense, Stroke, Ui, UiBuilder,
    pos2, vec2,
};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

use super::remember_control_focus;
use super::surface::on_video_style;
use youtfast_app::{Action, State};
use youtfast_appearance::theme;
use youtfast_core::model::SponsorBehavior;
use youtfast_mpv::Status;

/// A dark rounded label, centred over `bottom`.
pub(super) fn bubble(ui: &Ui, bottom: egui::Pos2, text: &str) {
    let galley = ui
        .painter()
        .layout(text.to_owned(), theme::medium(13.0), Color32::WHITE, 320.0);
    let size = galley.size() + vec2(16.0, 8.0);
    let clip = ui.clip_rect();
    let left =
        (bottom.x - size.x / 2.0).clamp(clip.left(), (clip.right() - size.x).max(clip.left()));
    let rect = Rect::from_min_size(pos2(left, bottom.y - size.y), size);
    let painter = ui.painter();
    let radius = CornerRadius::same(theme::RADIUS);
    painter.rect_filled(rect, radius, Color32::from_black_alpha(220));
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, Color32::from_white_alpha(28)),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(8.0, 4.0), galley, Color32::WHITE);
}

/// The note over the video's top ("+10 s", "Captions on").
pub(super) fn osd(ui: &Ui, rect: Rect, state: &State) {
    let locale: Locale = state.settings.language;
    if let Some((text, _)) = &state.osd {
        let top = pos2(rect.center().x, rect.top() + 24.0);
        let galley = ui.painter().layout_no_wrap(
            youtfast_i18n::message(locale, text).into_owned(),
            theme::semibold(15.0),
            Color32::WHITE,
        );
        let size = galley.size() + vec2(24.0, 12.0);
        let pill = Rect::from_center_size(top + vec2(0.0, size.y / 2.0), size);
        ui.painter().rect_filled(
            pill,
            CornerRadius::same((size.y / 2.0) as u8),
            Color32::from_black_alpha(170),
        );
        ui.painter()
            .galley(pill.min + vec2(12.0, 6.0), galley, Color32::WHITE);
    }
}

/// "Skip intro" while a segment the user skips by hand plays; also shown
/// for one skipped at once that the user went back into.
pub(super) fn skip_button(
    ui: &mut Ui,
    rect: Rect,
    strip: Rect,
    state: &State,
    status: &Status,
    shown: bool,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let Some(segment) = state.segments.iter().find(|segment| {
        matches!(
            state
                .settings
                .sponsorblock_categories
                .behavior_for(&segment.category),
            SponsorBehavior::AutoSkip | SponsorBehavior::ManualSkip
        ) && status.position >= segment.start
            && status.position < segment.end - 0.3
    }) else {
        return;
    };
    let label = segment.label();
    let text = trf(
        locale,
        "Skip {segment}",
        &[("segment", tr(locale, label).into_owned())],
    );
    let galley = ui
        .painter()
        .layout_no_wrap(text, theme::semibold(14.0), Color32::WHITE);
    let size = galley.size() + vec2(28.0, 16.0);
    let bottom = if shown {
        strip.top() - 12.0
    } else {
        rect.bottom() - 16.0
    };
    let button = Rect::from_min_size(pos2(rect.right() - 16.0 - size.x, bottom - size.y), size);
    let response = ui.interact(button, ui.id().with("skip-segment"), Sense::click());
    remember_control_focus(&response);
    let fill = if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        Color32::from_black_alpha(220)
    } else {
        Color32::from_black_alpha(170)
    };
    ui.painter().rect(
        button,
        CornerRadius::same(4),
        fill,
        Stroke::new(1.0, Color32::from_white_alpha(90)),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(button.min + vec2(14.0, 8.0), galley, Color32::WHITE);
    if response.clicked() {
        actions.push(Action::SkipSegment);
    }
}

/// The end of the video: the next one, a countdown, Cancel and Play now.
pub(super) fn countdown(ui: &mut Ui, rect: Rect, state: &State, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let Some((video, at)) = &state.autoplay else {
        return;
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(200));
    let seconds = at.saturating_duration_since(Instant::now()).as_secs() + 1;
    let card = Rect::from_center_size(rect.center(), vec2(rect.width().min(420.0) - 24.0, 200.0));
    let mut ui = ui.new_child(
        UiBuilder::new()
            .max_rect(card)
            .layout(Layout::top_down(Align::Center)),
    );
    on_video_style(&mut ui);
    ui.label(
        RichText::new(trf(
            locale,
            "Up next in {seconds}",
            &[("seconds", seconds.to_string())],
        ))
        .font(theme::regular(14.0))
        .color(Color32::from_white_alpha(200)),
    );
    ui.add_space(6.0);
    let display_title = state.display_title(video);
    let title = if display_title.is_empty() {
        tr(locale, "Next video")
    } else {
        std::borrow::Cow::Borrowed(display_title)
    };
    ui.add(
        egui::Label::new(
            RichText::new(title)
                .font(theme::bold(18.0))
                .color(Color32::WHITE),
        )
        .truncate(),
    );
    if let Some(channel) = &video.channel {
        ui.label(
            RichText::new(channel)
                .font(theme::regular(13.0))
                .color(Color32::from_white_alpha(200)),
        );
    }
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        let width = 2.0 * 110.0 + 12.0;
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        let cancel =
            egui::Button::new(RichText::new(tr(locale, "Cancel")).font(theme::medium(14.0)))
                .fill(Color32::from_white_alpha(40))
                .min_size(vec2(110.0, 36.0))
                .corner_radius(CornerRadius::same(18));
        if ui.add(cancel).clicked() {
            actions.push(Action::CancelAutoplay);
        }
        ui.add_space(12.0);
        let play = egui::Button::new(
            RichText::new(tr(locale, "Play now"))
                .font(theme::medium(14.0))
                .color(Color32::BLACK),
        )
        .fill(Color32::WHITE)
        .min_size(vec2(110.0, 36.0))
        .corner_radius(CornerRadius::same(18));
        if ui.add(play).clicked() {
            actions.push(Action::PlayNext);
        }
    });
}
