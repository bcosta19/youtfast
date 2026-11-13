//! New-upload alerts in the full page and the top-bar popup.

use egui::{CornerRadius, Rect, RichText, Sense, Stroke, Ui, vec2};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme;
use youtfast_core::model::View;
use youtfast_core::util::{self, AgeBucket};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

use super::{results, widgets};
use youtfast_app::Action;
use youtfast_app::State;

/// A compact view of the same visit, anchored to the top-bar bell.
pub(super) fn popup(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    let width = 420.0_f32.min((ui.ctx().content_rect().width() - 32.0).max(240.0));
    ui.set_width(width);

    // Header bar with icon, title, unread badge, and quick actions.
    ui.horizontal(|ui| {
        let (icon_rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
        Icon::Bell
            .image(palette.accent, 18.0)
            .paint_at(ui, icon_rect);
        ui.add_space(4.0);

        ui.label(
            RichText::new(tr(locale, "Notifications").as_ref())
                .font(theme::semibold(16.0))
                .color(palette.text),
        );

        let unread_count = state.alerts_visit.len();
        if unread_count > 0 {
            ui.add_space(2.0);
            let count_str = unread_count.to_string();
            let galley =
                ui.painter()
                    .layout_no_wrap(count_str, theme::semibold(11.0), palette.on_accent);
            let badge_width = (galley.size().x + 12.0).max(20.0);
            let (badge_rect, _) = ui.allocate_exact_size(vec2(badge_width, 18.0), Sense::hover());
            ui.painter()
                .rect_filled(badge_rect, CornerRadius::same(9), palette.accent);
            ui.painter().galley(
                badge_rect.center() - galley.size() / 2.0,
                galley,
                palette.on_accent,
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(
                ui,
                Icon::ExternalLink,
                16.0,
                tr(locale, "Open notifications").as_ref(),
            )
            .clicked()
            {
                actions.push(Action::Navigate(View::Notifications));
                ui.close();
            }

            if !state.alerts_visit.is_empty()
                && widgets::icon_button(
                    ui,
                    Icon::Check,
                    16.0,
                    tr(locale, "Mark all as seen").as_ref(),
                )
                .clicked()
            {
                actions.push(Action::MarkAlertsSeen);
            }

            if state.alerts_checking {
                widgets::spinner(ui, 16.0);
            } else if widgets::icon_button(
                ui,
                Icon::Refresh,
                16.0,
                tr(locale, "Check now").as_ref(),
            )
            .clicked()
            {
                actions.push(Action::CheckAlertsNow);
            }
        });
    });

    if state.settings.alerts {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let (clock_rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
            Icon::Clock
                .image(palette.dim, 12.0)
                .paint_at(ui, clock_rect);
            ui.add_space(2.0);
            ui.label(
                RichText::new(checked_line(state))
                    .font(theme::regular(12.0))
                    .color(palette.secondary),
            );
        });
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    if !state.settings.alerts {
        alerts_off_card(ui, state, actions);
    } else if state.alerts_visit.is_empty() {
        empty_state_card(ui, state, actions);
    } else {
        let height = (ui.ctx().content_rect().height() * 0.55).clamp(240.0, 480.0);
        egui::ScrollArea::vertical()
            .max_height(height)
            .auto_shrink([false, true])
            .show(ui, |ui| rows(ui, state, actions, false));

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let count = state.alerts_visit.len();
            ui.label(
                RichText::new(widgets::video_count_label(count as u64, locale))
                    .font(theme::regular(12.0))
                    .color(palette.secondary),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(
                    ui,
                    palette,
                    tr(locale, "Open notifications").as_ref(),
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
                {
                    actions.push(Action::Navigate(View::Notifications));
                    ui.close();
                }
            });
        });
    }
}

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;

    results::panel(ui, |ui| {
        results::title(
            ui,
            state,
            tr(locale, "Notifications").as_ref(),
            Some(state.alerts_visit.len()),
        );

        if !state.settings.alerts {
            alerts_off_card(ui, state, actions);
            return;
        }

        ui.horizontal(|ui| {
            if state.alerts_checking {
                widgets::loading(ui, palette, "new uploads", locale);
            } else if widgets::button(
                ui,
                palette,
                tr(locale, "Check now").as_ref(),
                widgets::ButtonStyle::Normal,
            )
            .clicked()
            {
                actions.push(Action::CheckAlertsNow);
            }
            if !state.alerts_visit.is_empty()
                && widgets::button(
                    ui,
                    palette,
                    tr(locale, "Mark all as seen").as_ref(),
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
            {
                actions.push(Action::MarkAlertsSeen);
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let (clock_rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
            Icon::Clock
                .image(palette.dim, 12.0)
                .paint_at(ui, clock_rect);
            ui.add_space(2.0);
            ui.label(
                RichText::new(checked_line(state))
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
            );
        });

        ui.add_space(16.0);
        if state.alerts_visit.is_empty() {
            empty_state_card(ui, state, actions);
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| rows(ui, state, actions, true));
    });
}

fn empty_state_card(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;

    ui.vertical_centered(|ui| {
        ui.add_space(24.0);

        let circle_size = 56.0;
        let (rect, _) = ui.allocate_exact_size(vec2(circle_size, circle_size), Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), circle_size / 2.0, palette.surface);
        ui.painter().circle_stroke(
            rect.center(),
            circle_size / 2.0,
            Stroke::new(1.0, palette.outline),
        );
        Icon::Bell
            .image(palette.dim, 26.0)
            .paint_at(ui, Rect::from_center_size(rect.center(), vec2(26.0, 26.0)));

        ui.add_space(14.0);
        ui.label(
            RichText::new(tr(locale, "Nothing new").as_ref())
                .font(theme::semibold(16.0))
                .color(palette.text),
        );

        ui.add_space(6.0);
        ui.label(
            RichText::new(tr(locale, "New videos from subscribed channels appear here").as_ref())
                .font(theme::regular(13.0))
                .color(palette.secondary),
        );

        ui.add_space(16.0);
        if state.alerts_checking {
            widgets::loading(ui, palette, "new uploads", locale);
        } else if widgets::button(
            ui,
            palette,
            tr(locale, "Check now").as_ref(),
            widgets::ButtonStyle::Normal,
        )
        .clicked()
        {
            actions.push(Action::CheckAlertsNow);
        }

        ui.add_space(24.0);
    });
}

fn alerts_off_card(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;

    ui.vertical_centered(|ui| {
        ui.add_space(24.0);

        let circle_size = 56.0;
        let (rect, _) = ui.allocate_exact_size(vec2(circle_size, circle_size), Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), circle_size / 2.0, palette.surface);
        ui.painter().circle_stroke(
            rect.center(),
            circle_size / 2.0,
            Stroke::new(1.0, palette.outline),
        );
        Icon::Alert
            .image(palette.warning, 26.0)
            .paint_at(ui, Rect::from_center_size(rect.center(), vec2(26.0, 26.0)));

        ui.add_space(14.0);
        ui.label(
            RichText::new(tr(locale, "Alerts are off").as_ref())
                .font(theme::semibold(16.0))
                .color(palette.text),
        );

        ui.add_space(6.0);
        ui.label(
            RichText::new(
                tr(
                    locale,
                    "Shows a desktop notification for each new video found.",
                )
                .as_ref(),
            )
            .font(theme::regular(13.0))
            .color(palette.secondary),
        );

        ui.add_space(16.0);
        if widgets::button(
            ui,
            palette,
            tr(locale, "Turn on").as_ref(),
            widgets::ButtonStyle::Primary,
        )
        .clicked()
        {
            actions.push(Action::SetAlerts(true));
        }

        ui.add_space(24.0);
    });
}

/// `Checked 14:32, next check in 9 min`.
fn checked_line(state: &State) -> String {
    let locale = state.settings.language;

    let mut line = match state.last_checked {
        Some(at) => {
            let clock = jiff::Timestamp::from_second(at)
                .map(|at| format!("{}", at.to_zoned(jiff::tz::TimeZone::system()).time()))
                .unwrap_or_else(|_| "?".to_owned());
            trf(
                locale,
                "Checked at {time}",
                &[("time", clock.get(..5).unwrap_or(&clock).to_owned())],
            )
        }
        None => tr(locale, "Not checked yet").into_owned(),
    };
    let next = u64::from(state.settings.alerts_every_minutes).saturating_sub(
        state
            .last_checked
            .map(|at| (util::now() - at) as u64 / 60)
            .unwrap_or(0),
    );
    line.push_str(&trf(
        locale,
        ", next check in {minutes} min",
        &[("minutes", next.max(1).to_string())],
    ));
    line
}

/// Rows grouped by day under section headings; the cursor opens them.
fn rows(ui: &mut Ui, state: &State, actions: &mut Vec<Action>, page: bool) {
    let locale = state.settings.language;

    let today = util::Today::at(util::now());
    let videos = &state.alerts_visit;
    let buckets: Vec<AgeBucket> = videos
        .iter()
        .map(|video| today.bucket(video.published_at))
        .collect();
    let mut start = 0;
    while start < videos.len() {
        let day = buckets[start];
        let end = buckets[start..]
            .iter()
            .position(|bucket| *bucket != day)
            .map_or(videos.len(), |offset| start + offset);
        let mut render_items = |ui: &mut Ui| {
            for (index, video) in videos.iter().enumerate().take(end).skip(start) {
                let selected = page && state.is_cursor(index);
                let watched = widgets::watched(&state.library, video);
                let response =
                    widgets::video_row(ui, &state.palette, video, None, watched, selected, locale);
                if selected && state.cursor_moved {
                    response.clone().scroll_to_me(None);
                }
                if response.clicked() {
                    actions.push(if ui.input(|i| i.modifiers.command) {
                        Action::OpenVideoTab(video.clone())
                    } else {
                        Action::Open(video.clone())
                    });
                    if !page {
                        ui.close();
                    }
                }
                results::video_menu(&response, state, video, actions);
            }
        };

        if page {
            widgets::section(
                ui,
                &state.palette,
                &capitalized(day, locale),
                None,
                render_items,
            );
        } else {
            let day_title = capitalized(day, locale);
            if !day_title.is_empty() {
                ui.label(
                    RichText::new(day_title)
                        .font(theme::semibold(13.0))
                        .color(state.palette.secondary),
                );
                ui.add_space(4.0);
            }
            render_items(ui);
        }
        ui.add_space(16.0);
        start = end;
    }
}

/// `Today` → "Today", translated.
pub(super) fn capitalized(bucket: AgeBucket, locale: Locale) -> String {
    let source = match bucket {
        AgeBucket::Today => "Today",
        AgeBucket::Yesterday => "Yesterday",
        AgeBucket::ThisWeek => "This week",
        AgeBucket::ThisMonth => "This month",
        AgeBucket::Undated => "",
        AgeBucket::Earlier => "Earlier",
    };
    tr(locale, source).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Rect, pos2, vec2};
    use youtfast_core::model::Video;

    #[test]
    fn popup_renders_empty_state_cleanly() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        youtfast_appearance::theme::install_fonts(&ctx);

        let state = State {
            notifications_popup_open: true,
            alerts_visit: Vec::new(),
            ..State::default()
        };
        let mut actions = Vec::new();

        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
                ..Default::default()
            },
            |ui| popup(ui, &state, &mut actions),
        );
        output.textures_delta.clear();

        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();

        assert!(texts.iter().any(|t| t == "Notifications"));
        assert!(texts.iter().any(|t| t == "Nothing new"));
        assert!(
            texts
                .iter()
                .any(|t| t == "New videos from subscribed channels appear here")
        );
        assert!(texts.iter().any(|t| t == "Check now"));
    }

    #[test]
    fn popup_renders_alerts_off_state() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        youtfast_appearance::theme::install_fonts(&ctx);

        let mut state = State::default();
        state.settings.alerts = false;
        let mut actions = Vec::new();

        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
                ..Default::default()
            },
            |ui| popup(ui, &state, &mut actions),
        );
        output.textures_delta.clear();

        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();

        assert!(texts.iter().any(|t| t == "Alerts are off"));
        assert!(texts.iter().any(|t| t == "Turn on"));
    }

    #[test]
    fn popup_renders_with_items_and_footer() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        youtfast_appearance::theme::install_fonts(&ctx);

        let mut video = Video::from_id("vid-123".into());
        video.title = "New Cool Video".into();
        let state = State {
            notifications_popup_open: true,
            alerts_visit: vec![video],
            ..State::default()
        };
        let mut actions = Vec::new();

        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
                ..Default::default()
            },
            |ui| popup(ui, &state, &mut actions),
        );
        output.textures_delta.clear();

        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();

        assert!(texts.iter().any(|t| t == "Notifications"));
        assert!(texts.iter().any(|t| t == "1"));
        assert!(texts.iter().any(|t| t == "New Cool Video"));
        assert!(texts.iter().any(|t| t == "1 video"));
    }
}
