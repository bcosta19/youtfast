//! Video surface painting, loading spinner, gradient shade, and context menu.

use egui::{Color32, CornerRadius, Mesh, Rect, Shape, Ui, pos2, vec2};
use youtfast_i18n::{Locale, gettext as tr};

use youtfast_app::{Action, State};
use youtfast_appearance::theme;
use youtfast_core::util;
use youtfast_mpv::{Status, VideoSurface};

/// The gradient behind them, taller than the strip.
const SHADE: f32 = 140.0;

pub(super) fn video_context_menu(
    response: &egui::Response,
    state: &State,
    status: &Status,
    actions: &mut Vec<Action>,
) {
    let locale: Locale = state.settings.language;
    let Some(video) = &state.watching else {
        return;
    };
    response.context_menu(|ui| {
        if ui.button(tr(locale, "Copy video URL")).clicked() {
            actions.push(Action::CopyLink(video.url()));
            ui.close();
        }
        if ui
            .button(tr(locale, "Copy video URL at current time"))
            .clicked()
        {
            actions.push(Action::CopyLink(util::video_link(
                &video.id,
                Some(status.position.max(0.0) as u64),
            )));
            ui.close();
        }
        let mut looping = state.playback_loop || video.short;
        if ui
            .add_enabled(
                !video.short,
                egui::Checkbox::new(&mut looping, tr(locale, "Loop video")),
            )
            .changed()
        {
            actions.push(Action::ToggleLoop);
        }
        if ui.button(tr(locale, "Open in browser")).clicked() {
            actions.push(Action::OpenUrl(video.url()));
            ui.close();
        }
    });
}

/// The picture, or the error over black, and the spinner while it opens or
/// waits for the network.
pub(super) fn picture(
    ui: &mut Ui,
    rect: Rect,
    state: &State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
) {
    picture_rounded(ui, rect, CornerRadius::ZERO, state, status, surface);
}

/// Whether the player is still opening the selected video.
fn opening(status: &Status, waiting_for_frame: bool, audio_only: bool) -> bool {
    if audio_only {
        return false;
    }
    // The spinner lasts until the video's first frame, not merely until its
    // audio starts. A file with no video track never gets a frame, and a
    // failed renderer never will, so neither keeps it spinning.
    let waiting = waiting_for_frame && !status.paused && (status.idle || status.has_video);
    waiting || (!status.idle && (status.buffering || (status.position <= 0.0 && !status.paused)))
}

/// Inputs needed to draw the picture surface (video, audio-only cover, error,
/// or opening spinner) from either [`State`] or a detached PiP snapshot.
pub(super) struct PictureContent<'a> {
    pub locale: Locale,
    pub audio_only: bool,
    pub thumbnail: Option<&'a str>,
    pub player_error: Option<&'a str>,
    pub demo_synthetic_images: bool,
}

pub(super) fn picture_rounded(
    ui: &mut Ui,
    rect: Rect,
    radius: CornerRadius,
    state: &State,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
) {
    let thumbnail = state.watching.as_ref().map(|video| {
        state
            .dearrow_thumbnail_url(video)
            .filter(|url| youtfast_app::images::is_loaded(url))
            .unwrap_or_else(|| video.thumbnail())
    });
    let demo_synthetic_images = cfg!(feature = "demo")
        && ui.ctx().data(|d| {
            d.get_temp::<bool>(egui::Id::new("synthetic-picture"))
                .unwrap_or(true)
        })
        && ui
            .ctx()
            .data(|d| d.get_temp::<bool>(egui::Id::new("synthetic-images")))
            .unwrap_or(false);
    picture_content(
        ui,
        rect,
        radius,
        PictureContent {
            locale: state.settings.language,
            audio_only: state.audio_only,
            thumbnail: thumbnail.as_deref(),
            player_error: state.player_error.as_deref(),
            demo_synthetic_images,
        },
        status,
        surface,
    );
}

pub(super) fn picture_content(
    ui: &mut Ui,
    rect: Rect,
    radius: CornerRadius,
    content: PictureContent<'_>,
    status: Option<&Status>,
    surface: Option<&VideoSurface>,
) {
    let locale = content.locale;
    ui.painter().rect_filled(rect, radius, Color32::BLACK);
    if content.audio_only {
        if let Some(thumbnail) = content.thumbnail {
            youtfast_app::images::remote(ui, thumbnail)
                .fit_to_exact_size(rect.size())
                .corner_radius(radius)
                .paint_at(ui, rect);
            ui.painter()
                .rect_filled(rect, radius, Color32::from_black_alpha(150));
            let badge_width = 130.0_f32.min(rect.width() - 16.0);
            if badge_width > 40.0 && rect.height() >= 40.0 {
                let badge_rect = Rect::from_center_size(rect.center(), vec2(badge_width, 32.0));
                ui.painter().rect_filled(
                    badge_rect,
                    CornerRadius::same(16),
                    Color32::from_black_alpha(180),
                );
                let text = tr(locale, "Audio only");
                let galley = ui.painter().layout_no_wrap(
                    text.into_owned(),
                    theme::medium(13.0),
                    Color32::WHITE,
                );
                ui.painter().galley(
                    badge_rect.center() - galley.size() / 2.0,
                    galley,
                    Color32::WHITE,
                );
            }
        }
    } else {
        if let Some(surface) = surface {
            surface.paint(ui, rect);
        }
        if cfg!(feature = "demo") && surface.is_none() && content.demo_synthetic_images {
            youtfast_app::images::remote(ui, "demo")
                .fit_to_exact_size(rect.size())
                .corner_radius(radius)
                .paint_at(ui, rect);
        }
    }
    if let Some(error) = content.player_error {
        let galley = ui.painter().layout(
            youtfast_i18n::message(locale, error).into_owned(),
            theme::regular(16.0),
            Color32::WHITE,
            (rect.width() - 32.0).max(1.0),
        );
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    } else if status.is_some_and(|status| {
        opening(
            status,
            surface.is_none_or(VideoSurface::waiting_for_frame),
            content.audio_only,
        )
    }) {
        let size = (rect.height() / 6.0).clamp(20.0, 40.0);
        egui::Spinner::new()
            .size(size)
            .color(Color32::WHITE)
            .paint_at(ui, Rect::from_center_size(rect.center(), vec2(size, size)));
        ui.ctx().request_repaint();
    }
}

/// Draws a thin playback progress bar along the bottom edge of `rect`.
pub(super) fn progress_strip(
    ui: &Ui,
    rect: Rect,
    bottom_radius: u8,
    status: &Status,
    watching_live: bool,
    accent: Color32,
) {
    if status.duration <= 0.0 || watching_live {
        return;
    }
    let fraction = (status.position / status.duration).clamp(0.0, 1.0) as f32;
    let track = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 3.0), rect.max);
    let track_radius = CornerRadius {
        nw: 0,
        ne: 0,
        sw: bottom_radius,
        se: bottom_radius,
    };
    ui.painter()
        .rect_filled(track, track_radius, Color32::from_white_alpha(45));
    let line = Rect::from_min_max(
        pos2(rect.left(), rect.bottom() - 3.0),
        pos2(rect.left() + rect.width() * fraction, rect.bottom()),
    );
    ui.painter().rect_filled(
        line,
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: bottom_radius,
            se: if fraction >= 0.99 { bottom_radius } else { 0 },
        },
        accent,
    );
}

/// White text and icons, and hovers that read over any picture, whatever
/// the theme.
pub(super) fn on_video_style(ui: &mut Ui) {
    let visuals = ui.visuals_mut();
    visuals.override_text_color = Some(Color32::WHITE);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(36);
    visuals.widgets.inactive.bg_fill = Color32::from_white_alpha(70);
    visuals.widgets.hovered.bg_fill = Color32::WHITE;
    visuals.widgets.active.bg_fill = Color32::WHITE;
    visuals.selection.bg_fill = Color32::WHITE;
    visuals.slider_trailing_fill = true;
}

/// The gradient that keeps the controls readable over a bright picture.
pub(super) fn shade(ui: &Ui, rect: Rect) {
    let top = (rect.bottom() - SHADE).max(rect.top());
    let shade = Rect::from_min_max(pos2(rect.left(), top), rect.max);
    let mut mesh = Mesh::default();
    let clear = Color32::from_black_alpha(0);
    let dark = Color32::from_black_alpha(210);
    mesh.colored_vertex(shade.left_top(), clear);
    mesh.colored_vertex(shade.right_top(), clear);
    mesh.colored_vertex(shade.right_bottom(), dark);
    mesh.colored_vertex(shade.left_bottom(), dark);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spinner_covers_the_gap_before_the_first_frame() {
        const NO_FRAME_YET: bool = true;
        const FRAME_SHOWN: bool = false;
        const VIDEO: bool = false;
        const AUDIO_ONLY: bool = true;

        let idle = Status {
            idle: true,
            ..Default::default()
        };
        assert!(
            opening(&idle, NO_FRAME_YET, VIDEO),
            "a selected video with no frame yet is still opening"
        );
        assert!(
            !opening(&idle, FRAME_SHOWN, VIDEO),
            "a frame belonging to this load needs no spinner"
        );
        assert!(
            !opening(&idle, NO_FRAME_YET, AUDIO_ONLY),
            "audio-only mode never shows the opening spinner"
        );

        let buffering = Status {
            buffering: true,
            ..Default::default()
        };
        assert!(opening(&buffering, FRAME_SHOWN, VIDEO));

        let paused_at_start = Status {
            position: 0.0,
            paused: true,
            ..Default::default()
        };
        assert!(!opening(&paused_at_start, FRAME_SHOWN, VIDEO));

        let playing = Status {
            position: 3.0,
            has_video: true,
            ..Default::default()
        };
        assert!(!opening(&playing, FRAME_SHOWN, VIDEO));
        assert!(
            opening(&playing, NO_FRAME_YET, VIDEO),
            "the spinner lasts until the first frame, not just until audio starts"
        );

        let without_picture = Status {
            position: 3.0,
            has_video: false,
            ..Default::default()
        };
        assert!(
            !opening(&without_picture, NO_FRAME_YET, VIDEO),
            "a file with no video track must not spin forever"
        );
    }
}
