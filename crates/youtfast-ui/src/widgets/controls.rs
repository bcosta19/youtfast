//! Interactive controls: buttons, chips, sidebar navigation items, and search fields.

use egui::text::LayoutJob;
use egui::{Color32, CornerRadius, CursorIcon, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use youtfast_i18n::{Locale, gettext as tr};

use super::{focus_outline, hairline_color, hover_fade, mix, press_fade, style_spec};
use youtfast_app::{Action, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, Palette, RADIUS};
use youtfast_core::model::Channel;

/// A frameless button that is just the icon, a round tint under it that fades
/// in on hover.
pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, hint: &str) -> Response {
    let edge = (size + 12.0).max(32.0);
    let (rect, response) = ui.allocate_exact_size(vec2(edge, edge), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), hint));
    if response.has_focus() {
        ui.painter().circle_stroke(
            rect.center(),
            edge / 2.0,
            Stroke::new(1.5, ui.visuals().selection.stroke.color),
        );
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hover = hover_fade(ui, &response);
    if hover > 0.0 {
        let widgets = &ui.visuals().widgets;
        let fill = mix(
            widgets.hovered.weak_bg_fill,
            widgets.active.weak_bg_fill,
            press_fade(ui, &response),
        );
        ui.painter()
            .circle_filled(rect.center(), edge / 2.0, fill.gamma_multiply(hover));
    }
    let tint = ui.visuals().text_color();
    icon.image(tint, size)
        .paint_at(ui, Rect::from_center_size(rect.center(), vec2(size, size)));
    response.on_hover_text(hint)
}

/// How a pill button reads.
#[derive(Clone, Copy, PartialEq)]
pub enum ButtonStyle {
    /// A soft pill: `surface`, brighter on hover.
    Normal,
    /// On (subscribed, saved): the soft pill with the label in bold.
    On,
    /// The page's main action, YouTube's way: `text` filled, `window`
    /// label (white on dark, black on light).
    Primary,
}

/// A pill-shaped button with a label.
pub fn button(ui: &mut Ui, palette: &Palette, label: &str, style: ButtonStyle) -> Response {
    icon_text_button(ui, palette, None, label, style)
}

/// A pill-shaped button with an icon before its label (the label may be
/// empty: then the pill is round around the icon).
pub fn icon_text_button(
    ui: &mut Ui,
    palette: &Palette,
    icon: Option<Icon>,
    label: &str,
    style: ButtonStyle,
) -> Response {
    let font = match style {
        ButtonStyle::Normal => theme::medium(14.0),
        ButtonStyle::On | ButtonStyle::Primary => theme::semibold(14.0),
    };
    let color = match style {
        ButtonStyle::Primary => palette.window,
        _ => palette.text,
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let icon_size = 18.0;
    let gap = if icon.is_some() && !label.is_empty() {
        6.0
    } else {
        0.0
    };
    let icon_width = if icon.is_some() { icon_size } else { 0.0 };
    let height = 34.0;
    let padding = if label.is_empty() { 8.0 } else { 14.0 };
    let width = (icon_width + gap + galley.size().x + 2.0 * padding).max(height);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let enabled = ui.is_enabled();
    let hovered = response.hovered() && enabled;
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hover = hover_fade(ui, &response);
    let press = press_fade(ui, &response);
    let spec = style_spec(ui);
    let fill = match style {
        ButtonStyle::Primary => palette
            .text
            .gamma_multiply(1.0 - 0.15 * hover - 0.1 * press),
        _ => mix(
            mix(palette.surface, palette.surface_hover, hover),
            palette.surface_active,
            press,
        ),
    };
    let visual_rect = rect.shrink(spec.press_inset * press);
    let radius_u8 = if spec.sharp_borders {
        spec.ctrl_radius
    } else {
        (visual_rect.height() / 2.0) as u8
    };
    let radius = CornerRadius::same(radius_u8);
    let painter = ui.painter();
    painter.rect_filled(visual_rect, radius, fill);
    if style != ButtonStyle::Primary && (spec.sharp_borders || spec.hairline_dividers) {
        let stroke_color = if spec.sharp_borders {
            palette.outline
        } else {
            hairline_color(palette)
        };
        painter.rect_stroke(
            visual_rect,
            radius,
            Stroke::new(1.0, stroke_color),
            egui::StrokeKind::Inside,
        );
    }
    super::paint_tactile_bevel(ui, palette, visual_rect, radius_u8);
    focus_outline(ui, &response, CornerRadius::same(radius_u8));
    let content = icon_width + gap + galley.size().x;
    let mut x = rect.center().x - content / 2.0;
    let tint = if enabled { color } else { palette.dim };
    if let Some(icon) = icon {
        icon.image(tint, icon_size).paint_at(
            ui,
            Rect::from_min_size(
                pos2(x, rect.center().y - icon_size / 2.0),
                vec2(icon_size, icon_size),
            ),
        );
        x += icon_size + gap;
    }
    let size = galley.size();
    ui.painter().galley_with_override_text_color(
        pos2(x, rect.center().y - size.y / 2.0),
        galley,
        tint,
    );
    response
}

/// Subscribe on this device, or show that the account already does.
/// `unsubscribed` is the button's style before subscribing.
pub fn subscribe(
    ui: &mut Ui,
    state: &State,
    channel: &Channel,
    unsubscribed: ButtonStyle,
    actions: &mut Vec<Action>,
) {
    let locale = state.settings.language;

    if state.subscribed_in_account(&channel.id) {
        ui.add_enabled_ui(false, |ui| {
            button(
                ui,
                &state.palette,
                tr(locale, "Subscribed").as_ref(),
                ButtonStyle::On,
            )
        })
        .inner
        .on_disabled_hover_text(
            tr(
                locale,
                "Your account is subscribed. Manage it on youtube.com.",
            )
            .as_ref(),
        );
        return;
    }
    let subscribed = state.library.is_subscribed(&channel.id);
    let (label, style) = if subscribed {
        ("Subscribed", ButtonStyle::On)
    } else {
        ("Subscribe", unsubscribed)
    };
    let hint = if subscribed {
        "Unsubscribe on this device"
    } else {
        "Follow this channel on this device: its videos join your feed"
    };
    if button(ui, &state.palette, tr(locale, label).as_ref(), style)
        .on_hover_text(tr(locale, hint).as_ref())
        .clicked()
    {
        actions.push(Action::ToggleSubscription(channel.clone()));
    }
}

/// A sidebar item, the siblings' way: the icon and the label (bold when
/// current), a rounded fill when current or hovered, and a count at the
/// end. `collapsed` draws only the icon, centred, with the count as a dot.
pub fn nav_item(
    ui: &mut Ui,
    palette: &Palette,
    icon: Icon,
    label: &str,
    count: Option<usize>,
    selected: bool,
    collapsed: bool,
) -> Response {
    let height = 40.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, label)
    });
    let hovered = response.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    // The fill and the label fade with the pointer and with the selection,
    // so moving between pages reads as one item handing over to the next.
    let hover = hover_fade(ui, &response);
    let on = ui.ctx().animate_bool_with_easing(
        response.id.with("selected"),
        selected,
        egui::emath::easing::cubic_out,
    );
    let spec = style_spec(ui);
    let item_radius = CornerRadius::same(spec.ctrl_radius.max(RADIUS));
    let lit = hover.max(on);
    if lit > 0.0 {
        let fill = mix(palette.surface_hover, palette.surface, on);
        ui.painter()
            .rect_filled(rect, item_radius, fill.gamma_multiply(lit));
        if spec.nav_accent_pill && on > 0.01 {
            let pill = Rect::from_center_size(
                pos2(rect.left() + 4.0, rect.center().y),
                vec2(3.0, 18.0 * on),
            );
            ui.painter().rect_filled(
                pill,
                CornerRadius::same(2),
                palette.accent.gamma_multiply(on),
            );
        }
    }
    focus_outline(ui, &response, item_radius);
    let color = mix(palette.secondary, palette.text, lit);
    let icon_size = 20.0;
    let count = count.filter(|count| *count > 0);
    if collapsed {
        let icon_rect = Rect::from_center_size(rect.center(), vec2(icon_size, icon_size));
        icon.image(color, icon_size).paint_at(ui, icon_rect);
        if count.is_some() {
            ui.painter()
                .circle_filled(icon_rect.right_top() + vec2(1.0, 1.0), 4.0, palette.accent);
        }
        return response.on_hover_text(label);
    }
    let icon_rect = Rect::from_min_size(
        pos2(rect.left() + 12.0, rect.center().y - icon_size / 2.0),
        vec2(icon_size, icon_size),
    );
    icon.image(color, icon_size).paint_at(ui, icon_rect);
    let font = if selected {
        theme::semibold(14.0)
    } else {
        theme::medium(14.0)
    };
    let mut right = rect.right() - 12.0;
    if let Some(count) = count {
        let galley = ui.painter().layout_no_wrap(
            count.to_string(),
            theme::semibold(12.0),
            palette.on_accent,
        );
        let pill = Rect::from_center_size(
            pos2(right - (galley.size().x + 12.0) / 2.0, rect.center().y),
            vec2((galley.size().x + 12.0).max(20.0), 20.0),
        );
        ui.painter()
            .rect_filled(pill, CornerRadius::same(10), palette.accent);
        ui.painter().galley(
            pill.center() - galley.size() / 2.0,
            galley,
            palette.on_accent,
        );
        right = pill.left() - 6.0;
    }
    let mut job = LayoutJob::default();
    job.append(label, 0.0, egui::TextFormat::simple(font, color));
    job.wrap.max_width = (right - icon_rect.right() - 14.0).max(10.0);
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        pos2(
            icon_rect.right() + 14.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    response
}

/// One of the chips over a list, YouTube's way: a pill on `surface`,
/// `text` filled with the `window` colour when on.
pub fn chip(ui: &mut Ui, palette: &Palette, label: &str, on: bool) -> Response {
    let text_color = if on { palette.window } else { palette.text };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::medium(14.0), text_color);
    let height = 32.0;
    let size = vec2(galley.size().x + 28.0, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, label)
    });
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hover = hover_fade(ui, &response);
    let press = press_fade(ui, &response);
    let fill = if on {
        palette
            .text
            .gamma_multiply(1.0 - 0.12 * hover - 0.1 * press)
    } else {
        mix(
            mix(palette.surface, palette.surface_hover, hover),
            palette.surface_active,
            press,
        )
    };
    let radius = CornerRadius::same((height / 2.0) as u8);
    let spec = style_spec(ui);
    let visual_rect = rect.shrink(spec.press_inset * press);
    ui.painter().rect_filled(visual_rect, radius, fill);
    if !on && (spec.sharp_borders || spec.hairline_dividers) {
        let stroke_color = if spec.sharp_borders {
            palette.outline
        } else {
            hairline_color(palette)
        };
        ui.painter().rect_stroke(
            visual_rect,
            radius,
            Stroke::new(1.0, stroke_color),
            egui::StrokeKind::Inside,
        );
    }
    super::paint_tactile_bevel(ui, palette, visual_rect, (height / 2.0) as u8);
    focus_outline(ui, &response, radius);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, text_color);
    response
}

/// A persistent filter toggle chip (e.g. `Hide watched`) visually distinct
/// from mutually exclusive feed-mode chips in modern styles, while staying a
/// classic pill in [`theme::InterfaceStyle::Classic`].
pub fn toggle_chip(ui: &mut Ui, palette: &Palette, label: &str, on: bool) -> Response {
    let spec = style_spec(ui);
    if !spec.hairline_dividers {
        return chip(ui, palette, label, on);
    }
    let text_color = if on {
        palette.accent
    } else {
        palette.secondary
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::medium(13.5), text_color);
    let height = 32.0;
    let dot_space = 13.0;
    let size = vec2(galley.size().x + 26.0 + dot_space, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, label)
    });
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hover = hover_fade(ui, &response);
    let press = press_fade(ui, &response);
    let on_t = ui.ctx().animate_bool_with_easing(
        response.id.with("toggle-on"),
        on,
        egui::emath::easing::cubic_out,
    );
    let off_fill = mix(palette.surface, palette.surface_hover, hover);
    let on_fill = mix(
        palette.accent.gamma_multiply(0.16),
        palette.accent.gamma_multiply(0.24),
        hover,
    );
    let fill = mix(off_fill, on_fill, on_t);
    let border = mix(
        palette.outline.gamma_multiply(0.85),
        palette.accent.gamma_multiply(0.55),
        on_t,
    );
    let visual = rect.shrink(spec.press_inset * press);
    let radius = CornerRadius::same((height / 2.0) as u8);
    let painter = ui.painter();
    painter.rect_filled(visual, radius, fill);
    painter.rect_stroke(
        visual,
        radius,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    focus_outline(ui, &response, radius);
    let fg = mix(
        mix(palette.secondary, palette.text, hover),
        palette.accent,
        on_t,
    );
    let dot_center = pos2(visual.left() + 14.0, visual.center().y);
    painter.circle_filled(dot_center, 3.5, fg.gamma_multiply(0.55 + 0.45 * on_t));
    painter.galley_with_override_text_color(
        pos2(
            dot_center.x + 8.0,
            visual.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    response
}

/// An Apple-style segmented control with a smoothly sliding active thumb.
/// Returns the index clicked when the user picks a different segment.
pub fn segmented_control(
    ui: &mut Ui,
    palette: &Palette,
    id: egui::Id,
    labels: &[&str],
    selected: usize,
) -> Option<usize> {
    if labels.is_empty() {
        return None;
    }
    let spec = style_spec(ui);
    let pad = 3.0;
    let height = 32.0;
    let seg_height = height - 2.0 * pad;
    let selected = selected.min(labels.len() - 1);
    let mut content_width = 0.0;
    let segments: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let font = if index == selected {
                theme::semibold(13.0)
            } else {
                theme::medium(13.0)
            };
            let galley =
                ui.painter()
                    .layout_no_wrap((*label).to_owned(), font, Color32::PLACEHOLDER);
            let width = (galley.size().x + 20.0).max(52.0);
            let offset = content_width;
            content_width += width;
            (offset, width, galley)
        })
        .collect();
    let total_width = content_width + pad * 2.0;
    let (track_rect, _) = ui.allocate_exact_size(vec2(total_width, height), Sense::hover());
    let track_radius = CornerRadius::same(spec.ctrl_radius.saturating_add(2));
    let thumb_radius = CornerRadius::same(spec.ctrl_radius);
    let track_fill = if palette.dark {
        mix(palette.window, palette.surface, 0.55)
    } else {
        mix(palette.panel, palette.surface_hover, 0.6)
    };
    let border_color = if spec.sharp_borders {
        palette.outline
    } else {
        hairline_color(palette)
    };
    ui.painter()
        .rect_filled(track_rect, track_radius, track_fill);
    ui.painter().rect_stroke(
        track_rect,
        track_radius,
        Stroke::new(1.0, border_color),
        egui::StrokeKind::Inside,
    );

    let (target_x, target_w, _) = segments[selected];
    // Animate within the track so moving or resizing it cannot leave the thumb behind.
    let animate = |edge, fraction| {
        ui.ctx()
            .animate_value_with_time(id.with(edge), fraction, ui.style().animation_time)
    };
    let left = animate("thumb-left", target_x / content_width).clamp(0.0, 1.0);
    let right = animate("thumb-right", (target_x + target_w) / content_width).clamp(left, 1.0);
    let content_left = track_rect.left() + pad;
    let content_top = track_rect.top() + pad;
    let thumb_rect = Rect::from_min_max(
        pos2(content_left + left * content_width, content_top),
        pos2(
            content_left + right * content_width,
            content_top + seg_height,
        ),
    );
    let thumb_fill = if palette.dark {
        palette.surface_hover
    } else {
        palette.overlay
    };
    ui.painter()
        .rect_filled(thumb_rect, thumb_radius, thumb_fill);
    ui.painter().rect_stroke(
        thumb_rect,
        thumb_radius,
        Stroke::new(1.0, border_color),
        egui::StrokeKind::Inside,
    );
    super::paint_tactile_bevel(ui, palette, thumb_rect, spec.ctrl_radius);

    let mut chosen = None;
    for (index, (offset, width, galley)) in segments.into_iter().enumerate() {
        let seg_rect = Rect::from_min_size(
            pos2(content_left + offset, content_top),
            vec2(width, seg_height),
        );
        let response = ui.interact(seg_rect, id.with(index), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                ui.is_enabled(),
                index == selected,
                labels[index],
            )
        });
        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let hover = hover_fade(ui, &response);
        if index != selected && hover > 0.0 {
            ui.painter().rect_filled(
                seg_rect,
                thumb_radius,
                palette.surface_hover.gamma_multiply(hover * 0.45),
            );
        }
        focus_outline(ui, &response, thumb_radius);
        let text_color = if index == selected {
            palette.text
        } else {
            mix(palette.secondary, palette.text, hover)
        };
        ui.painter().galley_with_override_text_color(
            seg_rect.center() - galley.size() / 2.0,
            galley,
            text_color,
        );
        if response.clicked() && index != selected {
            chosen = Some(index);
        }
    }
    chosen
}

/// Configuration for [`text_input`].
pub struct TextInputConfig<'a> {
    pub id: egui::Id,
    pub width: f32,
    pub hint: &'a str,
    pub password: bool,
    pub monospace: bool,
    pub char_limit: Option<usize>,
}

/// A polished single-line text input matching the app's surface elevation and
/// focus ring, replacing raw unstyled `egui::TextEdit` boxes.
pub fn text_input(
    ui: &mut Ui,
    palette: &Palette,
    text: &mut String,
    config: TextInputConfig<'_>,
) -> Response {
    let spec = style_spec(ui);
    let height = 32.0;
    let width = config.width.min(ui.available_width()).max(60.0);
    let (rect, hover_response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let focused = ui.memory(|m| m.has_focus(config.id));
    let focus_t = ui.ctx().animate_bool_with_easing(
        config.id.with("input-focus"),
        focused,
        egui::emath::easing::cubic_out,
    );
    let hover = hover_fade(ui, &hover_response);
    let radius = CornerRadius::same(spec.ctrl_radius);
    let base_fill = if palette.dark {
        mix(palette.window, palette.surface, 0.72)
    } else {
        mix(palette.panel, palette.overlay, 0.7)
    };
    let fill = mix(
        base_fill,
        palette.surface_hover,
        hover * 0.45 + focus_t * 0.35,
    );
    ui.painter().rect_filled(rect, radius, fill);
    let unfocused_border = if spec.sharp_borders {
        palette.outline
    } else {
        palette.outline.gamma_multiply(0.85)
    };
    let border_color = mix(
        unfocused_border,
        mix(palette.text.gamma_multiply(0.45), palette.accent, 0.55),
        focus_t,
    );
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0 + 0.5 * focus_t, border_color),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink2(vec2(10.0, 0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    let mut edit = egui::TextEdit::singleline(text)
        .id(config.id)
        .password(config.password)
        .frame(egui::Frame::NONE)
        .hint_text(egui::RichText::new(config.hint).color(palette.secondary))
        .vertical_align(egui::Align::Center)
        .desired_width(inner.width());
    if config.monospace {
        edit = edit.font(egui::TextStyle::Monospace);
    } else {
        edit = edit.font(theme::regular(13.5));
    }
    if let Some(limit) = config.char_limit {
        edit = edit.char_limit(limit);
    }
    child.add_sized(inner.size(), edit)
}

/// Compact pill badge with a leading status dot (Device sync, Cast status, etc.).
pub fn status_pill(ui: &mut Ui, label: &str, tint: Color32) {
    let horizontal_padding = 26.0;
    let galley = ui.painter().layout(
        label.to_owned(),
        theme::semibold(11.5),
        tint,
        (ui.available_width() - horizontal_padding).max(1.0),
    );
    let height = (galley.size().y + 6.0).max(22.0);
    let width = galley.size().x + horizontal_padding;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), label));
    let radius = CornerRadius::same(11);
    let painter = ui.painter();
    painter.rect_filled(rect, radius, tint.gamma_multiply(0.16));
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, tint.gamma_multiply(0.32)),
        egui::StrokeKind::Inside,
    );
    let dot = pos2(rect.left() + 10.0, rect.center().y);
    painter.circle_filled(dot, 3.0, tint);
    painter.galley(
        pos2(dot.x + 7.0, rect.center().y - galley.size().y / 2.0),
        galley,
        tint,
    );
}

/// Text edit identity, sizing, and optional embedded submit button.
#[derive(Debug)]
pub struct SearchField<'a> {
    id: egui::Id,
    width: f32,
    return_key: Option<egui::KeyboardShortcut>,
    submit_label: Option<&'a str>,
}

impl<'a> SearchField<'a> {
    /// Creates a search field with a decorative search icon.
    pub fn new(id: egui::Id, width: f32) -> Self {
        Self {
            id,
            width,
            return_key: Some(egui::KeyboardShortcut::new(
                egui::Modifiers::NONE,
                egui::Key::Enter,
            )),
            submit_label: None,
        }
    }

    /// Makes the leading icon a button with a localized accessible name.
    pub fn submit_icon(mut self, label: &'a str) -> Self {
        self.submit_label = Some(label);
        self
    }

    /// Prevents Enter from surrendering text edit focus.
    pub fn keep_focus_on_enter(mut self) -> Self {
        self.return_key = None;
        self
    }
}

/// The search field, the siblings' way: a rounded field with the search
/// icon at its left and a clear button at its right while it has text.
/// Returns the text edit's response (Enter is the caller's to read).
pub fn search_field(
    ui: &mut Ui,
    palette: &Palette,
    text: &mut String,
    hint: &str,
    locale: Locale,
    config: SearchField<'_>,
) -> Response {
    search_field_with_submit(ui, palette, text, hint, locale, config).0
}

/// Returns the text edit response and whether the embedded search button activated.
pub fn search_field_with_submit(
    ui: &mut Ui,
    palette: &Palette,
    text: &mut String,
    hint: &str,
    locale: Locale,
    SearchField {
        id,
        width,
        return_key,
        submit_label,
    }: SearchField<'_>,
) -> (Response, bool) {
    let spec = style_spec(ui);
    let height = 36.0;
    let (rect, hover_response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let focused = ui.memory(|memory| memory.has_focus(id));
    let focus_t = ui.ctx().animate_bool(id.with("focused"), focused);
    let hover = hover_fade(ui, &hover_response);
    let fill = mix(palette.surface, palette.surface_hover, hover.max(focus_t));
    let radius = CornerRadius::same((height / 2.0) as u8);
    ui.painter().rect_filled(rect, radius, fill);
    let unfocused_border = if spec.sharp_borders {
        palette.outline
    } else {
        palette.outline.gamma_multiply(0.75)
    };
    let border_color = mix(
        unfocused_border,
        mix(palette.text.gamma_multiply(0.5), palette.accent, 0.45),
        focus_t,
    );
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0 + 0.5 * focus_t, border_color),
        egui::StrokeKind::Inside,
    );
    let clear = !text.is_empty();
    let field_rect = Rect::from_min_max(
        pos2(rect.left() + 36.0, rect.top()),
        pos2(
            rect.right() - if clear { 34.0 } else { 14.0 },
            rect.bottom(),
        ),
    );
    let mut field_ui = ui.new_child(egui::UiBuilder::new().max_rect(field_rect));
    let response = field_ui.add_sized(
        field_rect.size(),
        egui::TextEdit::singleline(text)
            .id(id)
            .return_key(return_key)
            .frame(egui::Frame::NONE)
            .hint_text(egui::RichText::new(hint).color(palette.secondary))
            .font(theme::regular(14.0))
            .vertical_align(egui::Align::Center)
            .desired_width(field_rect.width()),
    );
    let search_center = pos2(rect.left() + 20.0, rect.center().y);
    let submitted = if let Some(label) = submit_label {
        let button = Rect::from_center_size(search_center, vec2(28.0, 28.0));
        let submit = ui.interact(button, id.with("submit"), Sense::click());
        submit.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
        });
        paint_search_button(ui, palette, &submit, Icon::Search, 16.0);
        submit.clicked()
    } else {
        Icon::Search
            .image(mix(palette.secondary, palette.text, focus_t), 16.0)
            .paint_at(ui, Rect::from_center_size(search_center, vec2(16.0, 16.0)));
        false
    };
    if clear {
        let button =
            Rect::from_center_size(pos2(rect.right() - 20.0, rect.center().y), vec2(24.0, 24.0));
        let clear_response = ui.interact(button, id.with("clear"), Sense::click());
        clear_response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                format!("{}: {hint}", tr(locale, "Clear")),
            )
        });
        paint_search_button(ui, palette, &clear_response, Icon::Close, 15.0);
        if clear_response.clicked() {
            text.clear();
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    }
    (response, submitted)
}

fn paint_search_button(ui: &Ui, palette: &Palette, response: &Response, icon: Icon, size: f32) {
    let hover = hover_fade(ui, response);
    let center = response.rect.center();
    let radius = response.rect.width() / 2.0;
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if hover > 0.0 {
        ui.painter()
            .circle_filled(center, radius, palette.surface_active.gamma_multiply(hover));
    }
    focus_outline(ui, response, CornerRadius::same(radius as u8));
    icon.image(mix(palette.secondary, palette.text, hover), size)
        .paint_at(ui, Rect::from_center_size(center, vec2(size, size)));
}

/// Paints an animated pill switch inside `rect`, blending between `off` and
/// `accent` and stretching the knob while pressed.
pub fn paint_switch(
    ui: &Ui,
    rect: Rect,
    response: &Response,
    on: bool,
    off: Color32,
    accent: Color32,
) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let enabled = ui.is_enabled();
    let t = ui
        .ctx()
        .animate_bool_with_easing(response.id, on, egui::emath::easing::cubic_out);
    let press = press_fade(ui, response);
    let fill = mix(off, accent, t);
    let (fill, knob) = if enabled {
        (fill, Color32::WHITE)
    } else {
        (fill.gamma_multiply(0.5), Color32::WHITE.gamma_multiply(0.6))
    };
    let radius = CornerRadius::same((rect.height() / 2.0).round() as u8);
    ui.painter().rect_filled(rect, radius, fill);
    focus_outline(ui, response, radius);
    // The knob stretches towards the other side while held, so the press is
    // felt before the switch moves.
    let knob_d = (rect.height() - 6.0).max(8.0);
    let inset = rect.height() / 2.0;
    let stretch = rect.height() * 0.22 * press;
    let x = egui::lerp(
        rect.left() + inset + stretch / 2.0..=rect.right() - inset - stretch / 2.0,
        t,
    );
    let knob_rect =
        Rect::from_center_size(pos2(x, rect.center().y), vec2(knob_d + stretch, knob_d));
    ui.painter().rect_filled(
        knob_rect,
        CornerRadius::same((knob_d / 2.0).round() as u8),
        knob,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Id;

    #[test]
    fn search_clear_button_has_a_localized_accessible_name() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut query = "texto".to_owned();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            search_field(
                ui,
                &Palette::dark(),
                &mut query,
                "Pesquisar",
                Locale::PortugueseBrazil,
                SearchField::new(Id::new("search"), 240.0),
            );
        });
        output.textures_delta.clear();
        let tree = output
            .platform_output
            .accesskit_update
            .expect("accessibility tree");
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.label() == Some("Limpar: Pesquisar"))
        );
        assert!(
            !tree
                .nodes
                .iter()
                .any(|(_, node)| node.role() == egui::accesskit::Role::Button
                    && node.label().is_some_and(|label| label.contains("Enter")))
        );
    }

    #[test]
    fn embedded_search_button_has_a_name_and_submits_on_click_or_enter() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        theme::install_fonts(&ctx);
        let locale = Locale::PortugueseBrazil;
        let label = tr(locale, "Search (Enter)");
        let id = Id::new("embedded search");
        let mut query = "texto".to_owned();
        let mut draw = |events| {
            let mut result = (pos2(0.0, 0.0), false);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    let origin = ui.next_widget_position();
                    let (_, submitted) = search_field_with_submit(
                        ui,
                        &Palette::dark(),
                        &mut query,
                        "Pesquisar",
                        locale,
                        SearchField::new(id, 240.0).submit_icon(label.as_ref()),
                    );
                    result = (origin + vec2(20.0, 18.0), submitted);
                },
            );
            output.textures_delta.clear();
            (result, output.platform_output.accesskit_update)
        };

        let ((pos, submitted), tree) = draw(vec![]);
        assert!(!submitted);
        let tree = tree.expect("accessibility tree");
        assert!(tree.nodes.iter().any(|(_, node)| {
            node.role() == egui::accesskit::Role::Button
                && node.label() == Some("Pesquisar (Enter)")
        }));
        draw(vec![egui::Event::PointerMoved(pos)]);
        let pointer = |pressed| {
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]
        };
        let ((_, pressed), _) = draw(pointer(true));
        let ((_, clicked), _) = draw(pointer(false));
        assert!(!pressed);
        assert!(clicked);

        ctx.memory_mut(|memory| memory.request_focus(id.with("submit")));
        let ((_, submitted), _) = draw(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(submitted);
        assert_eq!(query, "texto");
    }

    #[test]
    fn status_pill_wraps_but_exposes_the_full_status_as_one_label() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        theme::install_fonts(&ctx);
        let status = "Could not synchronize histories because the other device is not reachable.";
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(150.0);
            status_pill(ui, status, Color32::WHITE);
        });
        output.textures_delta.clear();
        assert!(output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text)
                if text.galley.text() == status && text.galley.rows.len() > 1)
        }));
        let tree = output
            .platform_output
            .accesskit_update
            .expect("accessibility tree");
        assert!(tree.nodes.iter().any(|(_, node)| {
            node.role() == egui::accesskit::Role::Label && node.value() == Some(status)
        }));
    }

    #[test]
    fn segmented_thumb_moves_with_track_without_waiting_for_animation() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let palette = Palette::dark();
        let thumb = |space| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(space);
                    segmented_control(ui, &palette, Id::new("segments"), &["5 min", "15 min"], 1);
                });
            });
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.fill == palette.surface_hover => {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .expect("selected segment thumb")
        };
        let first = thumb(10.0);
        let moved = thumb(110.0);
        assert!((moved.left() - first.left() - 100.0).abs() < 0.1);
        assert!((moved.width() - first.width()).abs() < 0.1);
    }

    #[test]
    fn chip_uses_pill_corners() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            chip(ui, &Palette::dark(), "All", false);
        });
        output.textures_delta.clear();
        assert!(output.shapes.iter().any(|shape| {
            matches!(
                &shape.shape,
                egui::Shape::Rect(rect) if rect.corner_radius == CornerRadius::same(16)
            )
        }));
    }
}
