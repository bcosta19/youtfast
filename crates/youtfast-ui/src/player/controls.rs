//! Transport buttons, seek bar, chapters, SponsorBlock segments, and live badge.

use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, Layout, Rect, RichText, Sense, Slider, Ui,
    UiBuilder, pos2, vec2,
};
use youtfast_i18n::{Locale, format as trf, gettext as tr};

use super::overlays::bubble;
use super::{Mode, control_button, remember_control_focus};
use youtfast_app::player::{LIVE_BEHIND, at_live_edge, speed_label};
use youtfast_app::{Action, PlayerMenu, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme;
use youtfast_core::model::{Fetch, Segment, SponsorBehavior, Video};
use youtfast_core::util;
use youtfast_mpv::Status;

const BUTTON_EDGE: f32 = 32.0; // `control_button` uses the widget's minimum target.
const BUTTON_SPACING: f32 = 4.0;
const GROUP_GAP: f32 = 8.0;

fn group_width(count: usize) -> f32 {
    count as f32 * BUTTON_EDGE + count.saturating_sub(1) as f32 * BUTTON_SPACING
}

#[derive(Clone, Copy, Default)]
pub(super) struct ButtonLayout {
    play: bool,
    next: bool,
    mute: bool,
    settings: bool,
    fullscreen: bool,
    page_controls: bool,
    pip: bool,
    captions: bool,
    audio_and_cast: bool,
}

impl ButtonLayout {
    pub(super) fn new(state: &State, status: &Status, mode: Mode, live: bool) -> Self {
        let page_controls = mode == Mode::Page && !state.watching_short();
        Self {
            play: true,
            next: !live
                && mode != Mode::Short
                && (state.playing_more.is_some() || state.next_video().is_some()),
            mute: true,
            settings: true,
            fullscreen: true,
            page_controls,
            pip: page_controls,
            captions: !status.subtitles.is_empty(),
            audio_and_cast: mode != Mode::Short,
        }
    }

    pub(super) fn preferred(mut self, width: f32, mode: Mode) -> Self {
        self.pip &= width >= 332.0;
        // Narrow bars keep audio-only on its shortcut and Cast in the top bar.
        self.audio_and_cast &= width >= 376.0;
        if mode == Mode::Short {
            self.mute &= width >= 112.0;
            self.fullscreen &= width >= 152.0;
            self.captions &= width >= 188.0;
        }
        self
    }

    fn right_width(self) -> f32 {
        group_width(
            usize::from(self.settings)
                + usize::from(self.fullscreen)
                + usize::from(self.page_controls) * 2
                + usize::from(self.pip)
                + usize::from(self.captions)
                + usize::from(self.audio_and_cast) * 2,
        )
    }

    fn gap(self) -> f32 {
        if self.play && self.settings {
            GROUP_GAP
        } else {
            0.0
        }
    }

    fn required_width(self) -> f32 {
        group_width(usize::from(self.play) + usize::from(self.next) + usize::from(self.mute))
            + self.gap()
            + self.right_width()
    }

    pub(super) fn fits(self, width: f32) -> bool {
        self.required_width() <= width
    }

    pub(super) fn compact(self, width: f32) -> Self {
        if self.fits(width) {
            return self;
        }
        if width < 2.0 * BUTTON_EDGE + GROUP_GAP {
            return Self {
                play: width >= BUTTON_EDGE,
                ..Self::default()
            };
        }
        // Keep full-size targets. Two groups each omit their trailing spacing.
        let mut slots =
            ((width - GROUP_GAP + 2.0 * BUTTON_SPACING) / (BUTTON_EDGE + BUTTON_SPACING)) as usize;
        let mut take = |enabled, count| {
            if enabled && slots >= count {
                slots -= count;
                true
            } else {
                false
            }
        };
        Self {
            play: take(self.play, 1),
            settings: take(self.settings, 1),
            fullscreen: take(self.fullscreen, 1),
            mute: take(self.mute, 1),
            next: take(self.next, 1),
            captions: take(self.captions, 1),
            page_controls: take(self.page_controls, 2),
            pip: take(self.pip, 1),
            audio_and_cast: take(self.audio_and_cast, 2),
        }
    }
}

/// The row under the seek bar.
pub(super) fn buttons(
    ui: &mut Ui,
    layout: ButtonLayout,
    state: &mut State,
    status: &Status,
    mode: Mode,
    live: bool,
    actions: &mut Vec<Action>,
) -> (Rect, Rect) {
    let locale: Locale = state.settings.language;
    // Reserve the entire right-hand group before laying out optional
    // volume/time/chapter content. A long title must never push a control
    // beyond the edge of the video.
    let row = ui.available_rect_before_wrap();
    let speed = (!at_live_edge(live, status) && status.speed != 1.0 && row.width() >= 900.0)
        .then(|| {
            let galley = ui.painter().layout_no_wrap(
                speed_label(status.speed),
                theme::medium(13.0),
                Color32::WHITE,
            );
            let width = galley.size().x + BUTTON_SPACING;
            (galley, width)
        })
        .filter(|(_, width)| layout.required_width() + width <= row.width());
    let right_width = layout.right_width() + speed.as_ref().map_or(0.0, |(_, width)| *width);
    let right_rect = Rect::from_min_max(
        pos2((row.right() - right_width).max(row.left()), row.top()),
        row.max,
    );
    let left_rect = Rect::from_min_max(
        row.min,
        pos2(
            (right_rect.left() - layout.gap()).max(row.left()),
            row.bottom(),
        ),
    );
    let mut right = ui.new_child(
        UiBuilder::new()
            .id_salt("right-controls")
            .max_rect(right_rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    right.spacing_mut().item_spacing.x = BUTTON_SPACING;
    let mut left = ui.new_child(
        UiBuilder::new()
            .id_salt("left-controls")
            .max_rect(left_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    left.spacing_mut().item_spacing.x = BUTTON_SPACING;
    let ui = &mut left;
    let (icon, hint) = if status.paused {
        (Icon::Play, tr(locale, "Play (k)"))
    } else {
        (Icon::Pause, tr(locale, "Pause (k)"))
    };
    if layout.play && control_button(ui, icon, 20.0, &hint).clicked() {
        actions.push(Action::TogglePause);
    }
    if layout.next {
        let hint = state
            .next_video()
            .map(|video| next_hint(state, video, locale))
            .unwrap_or_else(|| tr(locale, "Load next videos / retry (Shift+N)").into());
        if control_button(ui, Icon::SkipForward, 18.0, &hint).clicked() {
            actions.push(Action::PlayNext);
        }
    }
    let (icon, hint) = if status.muted || status.volume <= 0.0 {
        (Icon::VolumeMuted, tr(locale, "Unmute (m)"))
    } else {
        (Icon::Volume, tr(locale, "Mute (m)"))
    };
    if layout.mute && control_button(ui, icon, 18.0, &hint).clicked() {
        actions.push(Action::ToggleMute);
    }
    if mode != Mode::Short && left_rect.width() >= 300.0 {
        let mut volume = if status.muted { 0.0 } else { status.volume };
        let response = ui
            .scope(|ui| {
                ui.spacing_mut().slider_width = 80.0;
                ui.spacing_mut().interact_size.y = 14.0;
                ui.add(Slider::new(&mut volume, 0.0..=100.0).show_value(false))
            })
            .inner;
        remember_control_focus(&response);
        if response.changed() {
            actions.push(Action::SetVolume(volume));
        }
    }
    if layout.play && ui.available_width() >= 4.0 {
        ui.add_space(4.0);
        if live && ui.available_width() >= 24.0 {
            live_badge(ui, state, status, actions);
        } else {
            time_label(
                ui,
                state,
                status,
                mode != Mode::Short && row.width() >= 720.0,
                actions,
            );
        }
    }

    {
        let ui = &mut right;
        let (icon, hint) = if mode == Mode::Fullscreen {
            (Icon::ExitFullscreen, tr(locale, "Exit full screen (f)"))
        } else {
            (Icon::Fullscreen, tr(locale, "Full screen (f)"))
        };
        if layout.fullscreen && control_button(ui, icon, 18.0, &hint).clicked() {
            actions.push(Action::ToggleFullscreen);
        }
        // Full screen has no page layout to change.
        if layout.page_controls {
            let hint = if state.settings.theater {
                tr(locale, "Default view (t)")
            } else {
                tr(locale, "Theater mode (t)")
            };
            if control_button(ui, Icon::Theater, 18.0, &hint).clicked() {
                actions.push(Action::ToggleTheater);
            }
        }
        if layout.pip
            && control_button(
                ui,
                Icon::ExternalLink,
                18.0,
                &tr(locale, "Picture-in-picture (p)"),
            )
            .clicked()
        {
            actions.push(Action::TogglePip);
        }
        if layout.page_controls
            && control_button(ui, Icon::MiniPlayer, 18.0, &tr(locale, "Miniplayer (i)")).clicked()
        {
            actions.push(Action::ToggleMiniPlayer);
        }
        if layout.settings
            && control_button(ui, Icon::Settings, 18.0, &tr(locale, "Settings")).clicked()
        {
            state.player_menu = if state.player_menu == PlayerMenu::Closed {
                PlayerMenu::Main
            } else {
                PlayerMenu::Closed
            };
        }
        if layout.captions {
            let on = status.subtitles.iter().any(|subtitle| subtitle.selected);
            let response = control_button(ui, Icon::Captions, 18.0, &tr(locale, "Captions (c)"));
            if on {
                accent_line(ui, &response, state.palette.accent);
            }
            if response.clicked() {
                actions.push(Action::ToggleCaptions);
            }
        }
        if layout.audio_and_cast {
            let response = control_button(
                ui,
                Icon::AudioOnly,
                18.0,
                &tr(locale, "Audio only (Shift+A)"),
            );
            if state.audio_only {
                accent_line(ui, &response, state.palette.accent);
            }
            if response.clicked() {
                actions.push(Action::ToggleAudioOnly);
            }
            let cast = control_button(ui, Icon::Cast, 18.0, &tr(locale, "Cast to TV"));
            if state.cast.session.is_some() {
                accent_line(ui, &cast, state.palette.accent);
            }
            if cast.clicked() {
                actions.push(Action::Cast(youtfast_cast::Action::Open));
            }
        }
        if let Some((galley, _)) = speed {
            ui.label(galley);
        }
    }
    (left.min_rect(), right.min_rect())
}

/// YouTube's red line under a control while its feature is on.
fn accent_line(ui: &Ui, response: &egui::Response, accent: Color32) {
    let line = Rect::from_center_size(
        pos2(response.rect.center().x, response.rect.bottom() - 4.0),
        vec2(18.0, 2.5),
    );
    ui.painter()
        .rect_filled(line, CornerRadius::same(1), accent);
}

/// `Next: <title> (N)` for the next button's hover.
fn next_hint(state: &State, video: &Video, locale: Locale) -> String {
    let title = state.display_title(video);
    if title.is_empty() {
        tr(locale, "Next (Shift+N)").into_owned()
    } else {
        trf(
            locale,
            "Next: {title} (Shift+N)",
            &[("title", title.to_owned())],
        )
    }
}

/// `1:23 / 12:04 • Chapter`, the chapter when the video has them.
fn time_label(
    ui: &mut Ui,
    state: &State,
    status: &Status,
    show_chapter: bool,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let position = state.seek_preview.unwrap_or(status.position);
    let time = if status.duration > 0.0 {
        format!(
            "{} / {}",
            util::duration(position),
            util::duration(status.duration)
        )
    } else {
        util::duration(position)
    };
    let available = ui.available_width().max(0.0);
    if available < 28.0 {
        return;
    }
    // Digits that keep their width, so the label does not jitter as it counts.
    let font = theme::tabular(13.0);
    let full_width = ui
        .painter()
        .layout_no_wrap(time.clone(), font.clone(), Color32::WHITE)
        .size()
        .x
        + 4.0;
    let text = if full_width <= available {
        time.clone()
    } else {
        util::duration(position)
    };
    let industrial = theme::current_style(ui.ctx()) == theme::InterfaceStyle::Industrial;
    let time_resp = ui
        .add_sized(
            vec2(full_width.min(available), 32.0),
            egui::Label::new(RichText::new(text).font(font).color(Color32::WHITE)).truncate(),
        )
        .on_hover_text(time);
    if industrial {
        let readout = time_resp
            .rect
            .shrink2(vec2(0.0, 5.0))
            .expand2(vec2(3.0, 0.0));
        ui.painter().rect_stroke(
            readout,
            CornerRadius::same(3),
            egui::Stroke::new(1.0, Color32::from_white_alpha(64)),
            egui::StrokeKind::Inside,
        );
    }
    if show_chapter
        && ui.available_width() >= 80.0
        && let Some(chapter) = chapter_at(state, position)
    {
        let response = ui
            .add_sized(
                vec2(ui.available_width(), 32.0),
                egui::Label::new(
                    RichText::new(format!("• {chapter}"))
                        .font(theme::regular(13.0))
                        .color(Color32::from_white_alpha(200)),
                )
                .sense(Sense::click())
                .truncate(),
            )
            .on_hover_text(tr(locale, "Open chapters"));
        remember_control_focus(&response);
        egui::Popup::menu(&response).show(|ui| {
            if let Some(details) = state.details.ready() {
                for chapter in &details.chapters {
                    let label = format!(
                        "{}  {}",
                        util::duration(f64::from(chapter.seconds)),
                        chapter.name
                    );
                    if ui.button(label).clicked() {
                        actions.push(Action::SeekTo {
                            seconds: f64::from(chapter.seconds),
                            exact: true,
                        });
                        ui.close();
                    }
                }
            }
        });
    }
}

/// The chapter `seconds` falls in: the last one started by then.
fn chapter_at(state: &State, seconds: f64) -> Option<&str> {
    let details = state.details.ready()?;
    details
        .chapters
        .iter()
        .rev()
        .find(|chapter| f64::from(chapter.seconds) <= seconds)
        .map(|chapter| chapter.name.as_str())
}

/// `● LIVE`: red at the live edge; grey and a button back to it when
/// behind.
fn live_badge(ui: &mut Ui, state: &State, status: &Status, actions: &mut Vec<Action>) {
    let locale: Locale = state.settings.language;
    let edge = status.duration.max(status.cached_until);
    let behind = edge > 0.0 && edge - status.position > LIVE_BEHIND;
    let width = ui.available_width().clamp(24.0, 86.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 24.0), Sense::click());
    remember_control_focus(&response);
    let dot = if behind {
        Color32::from_gray(170)
    } else {
        state.palette.accent
    };
    if response.hovered() && behind {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        ui.painter()
            .rect_filled(rect, CornerRadius::same(12), Color32::from_white_alpha(36));
    }
    let painter = ui.painter();
    painter.circle_filled(pos2(rect.left() + 10.0, rect.center().y), 4.0, dot);
    if width >= 76.0 {
        painter.text(
            pos2(rect.left() + 20.0, rect.center().y),
            Align2::LEFT_CENTER,
            tr(locale, "LIVE"),
            theme::semibold(13.0),
            Color32::WHITE,
        );
    }
    let response = response.on_hover_text(if behind {
        tr(locale, "Back to live")
    } else {
        tr(locale, "Watching live")
    });
    if behind && response.clicked() {
        actions.push(Action::GoLive);
    }
}

/// The seek bar: a thin rounded track, played in `accent`, buffered in
/// white, cut at each chapter and marked with SponsorBlock's segments; it
/// thickens under the pointer, and a bubble over it names the time and the
/// chapter there.
pub(super) fn seek_bar(
    ui: &mut Ui,
    rect: Rect,
    state: &mut State,
    status: &Status,
    actions: &mut Vec<Action>,
) {
    let response = ui.interact(rect, ui.id().with("seek-bar"), Sense::click_and_drag());
    remember_control_focus(&response);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("timeline-keyboard-focus"), response.id));
    let duration = status.duration;
    let at_pointer = |pointer: egui::Pos2| {
        f64::from(((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0)) * duration
    };
    if duration > 0.0 {
        let pointer = response.interact_pointer_pos().map(at_pointer);
        if response.drag_stopped() || response.clicked() {
            if let Some(seconds) = pointer.or(state.seek_preview) {
                actions.push(Action::SeekTo {
                    seconds,
                    exact: true,
                });
            }
            state.seek_preview = None;
        } else if response.dragged()
            && let Some(seconds) = pointer
        {
            if state
                .seek_preview
                .is_none_or(|shown| (shown - seconds).abs() >= 0.5)
            {
                actions.push(Action::SeekTo {
                    seconds,
                    exact: false,
                });
            }
            state.seek_preview = Some(seconds);
        }
    }
    let active = (response.hovered() || response.dragged()) && duration > 0.0;
    if active {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let active_t = ui.ctx().animate_bool(response.id.with("active"), active);
    let thickness = 3.0 + 2.5 * active_t;
    let radius = CornerRadius::same((thickness / 2.0).ceil() as u8);
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), thickness));
    let x_at = |seconds: f64| {
        let fraction = if duration > 0.0 {
            (seconds / duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        track.left() + track.width() * fraction
    };
    let position = state.seek_preview.unwrap_or(status.position);
    let painter = ui.painter();
    let span = |from: f64, to: f64| {
        Rect::from_min_max(
            pos2(x_at(from), track.top()),
            pos2(x_at(to), track.bottom()),
        )
    };
    painter.rect_filled(track, radius, Color32::from_white_alpha(60));
    painter.rect_filled(
        span(0.0, status.cached_until),
        radius,
        Color32::from_white_alpha(110),
    );
    painter.rect_filled(span(0.0, position), radius, state.palette.accent);
    for segment in &state.segments {
        if state
            .settings
            .sponsorblock_categories
            .behavior_for(&segment.category)
            != SponsorBehavior::Ignore
        {
            painter.rect_filled(
                span(segment.start, segment.end),
                radius,
                segment_color(segment),
            );
        }
    }
    // The chapters: a gap in the track where each one starts.
    if let Fetch::Ready(details) = &state.details {
        for chapter in details
            .chapters
            .iter()
            .filter(|chapter| chapter.seconds > 0)
        {
            let x = x_at(f64::from(chapter.seconds));
            let gap = Rect::from_center_size(pos2(x, track.center().y), vec2(2.0, thickness + 1.0));
            painter.rect_filled(gap, CornerRadius::ZERO, Color32::from_black_alpha(200));
        }
    }
    if active_t > 0.01 {
        let knob = pos2(x_at(position), track.center().y);
        painter.circle_filled(
            knob,
            8.0 * active_t,
            Color32::from_black_alpha((110.0 * active_t) as u8),
        );
        painter.circle_filled(knob, 6.5 * active_t, state.palette.accent);
    }
    if active && let Some(pointer) = response.hover_pos().or(response.interact_pointer_pos()) {
        let seconds = at_pointer(pointer);
        let mut text = util::duration(seconds);
        if let Some(chapter) = chapter_at(state, seconds) {
            text = format!("{chapter}\n{text}");
        }
        crate::extras::seek_preview(ui, rect, state, seconds, actions);
        bubble(ui, pos2(pointer.x, rect.top() - 6.0), &text);
    }
}

/// SponsorBlock's colours, by category.
fn segment_color(segment: &Segment) -> Color32 {
    match segment.category.as_str() {
        "sponsor" => Color32::from_rgb(0x00, 0xd4, 0x00),
        "selfpromo" => Color32::from_rgb(0xff, 0xff, 0x00),
        "interaction" => Color32::from_rgb(0xcc, 0x00, 0xff),
        "intro" => Color32::from_rgb(0x00, 0xff, 0xff),
        "outro" => Color32::from_rgb(0x02, 0x02, 0xed),
        "preview" => Color32::from_rgb(0x00, 0x8f, 0xd6),
        "music_offtopic" => Color32::from_rgb(0xff, 0x99, 0x00),
        _ => Color32::from_rgb(0x73, 0x00, 0xff),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_core::model::{Chapter, Details};
    use youtfast_mpv::Subtitle;

    #[test]
    fn next_hint_translates_interface_text_and_preserves_the_video_title() {
        let state = State::default();
        let mut video = Video::from_id("aaaaaaaaaaa".into());
        video.title = "Qualidade {title}: um vídeo do canal".into();
        let english = next_hint(&state, &video, Locale::English);
        let portuguese = next_hint(&state, &video, Locale::PortugueseBrazil);
        assert_eq!(english, format!("Next: {} (Shift+N)", video.title));
        assert_eq!(portuguese, format!("Próximo: {} (Shift+N)", video.title));
        assert!(english.contains(&video.title));
        assert!(portuguese.contains(&video.title));
    }

    #[test]
    fn narrow_controls_stay_inside_the_video_and_keep_the_groups_apart() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State {
            watching: Some(Video::from_id("aaaaaaaaaaa".into())),
            queue: vec![Video::from_id("bbbbbbbbbbb".into())],
            details: Fetch::Ready(Details {
                chapters: vec![Chapter {
                    name: "Um capítulo com um nome muito longo ".repeat(20),
                    seconds: 0,
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let status = Status {
            duration: 100_000.0,
            position: 40_000.0,
            speed: 1.5,
            subtitles: vec![Subtitle {
                id: 1,
                lang: "pt".into(),
                title: "Português".into(),
                selected: true,
            }],
            ..Default::default()
        };
        for (mode, widths) in [
            (Mode::Page, &[296.0, 336.0, 376.0, 420.0, 720.0, 1280.0][..]),
            (
                Mode::Short,
                &[76.0, 100.0, 115.0, 140.0, 164.0, 200.0, 296.0][..],
            ),
        ] {
            for &width in widths {
                for captions in [false, true] {
                    let mut status = status.clone();
                    if !captions {
                        status.subtitles.clear();
                    }
                    for live in [false, true] {
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(Rect::from_min_size(
                                    pos2(0.0, 0.0),
                                    vec2(width, 48.0),
                                )),
                                ..Default::default()
                            },
                            |ui| {
                                let row = ui.max_rect();
                                let layout = ButtonLayout::new(&state, &status, mode, live)
                                    .preferred(row.width(), mode)
                                    .compact(row.width());
                                let (left, right) = buttons(
                                    ui,
                                    layout,
                                    &mut state,
                                    &status,
                                    mode,
                                    live,
                                    &mut Vec::new(),
                                );
                                assert!(
                                    left.right() <= right.left(),
                                    "control groups overlap at {width}: {left:?}, {right:?}"
                                );
                                assert!(
                                    right.right() <= row.right() + 0.1,
                                    "controls escape at {width}: {right:?} vs {row:?}"
                                );
                                assert!(left.left() >= row.left() - 0.1);
                            },
                        );
                        output.textures_delta.clear();
                    }
                }
            }
        }
    }
}
