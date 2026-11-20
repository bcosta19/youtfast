//! Interactive controls: buttons, chips, sidebar navigation items, and search fields.

use egui::text::LayoutJob;
use egui::{Color32, CornerRadius, CursorIcon, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use youtfast_i18n::{Locale, gettext as tr};

use super::{focus_outline, hover_fade, mix, press_fade};
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
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same((height / 2.0) as u8), fill);
    focus_outline(ui, &response, CornerRadius::same((height / 2.0) as u8));
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
    let on = ui
        .ctx()
        .animate_bool(response.id.with("selected"), selected);
    let lit = hover.max(on);
    if lit > 0.0 {
        let fill = mix(palette.surface_hover, palette.surface, on);
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS), fill.gamma_multiply(lit));
    }
    focus_outline(ui, &response, CornerRadius::same(RADIUS));
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
    ui.painter().rect_filled(rect, radius, fill);
    focus_outline(ui, &response, radius);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, text_color);
    response
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
    id: egui::Id,
    width: f32,
) -> Response {
    let height = 36.0;
    let (rect, hover_response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let focused = ui.memory(|memory| memory.has_focus(id));
    let focus_t = ui.ctx().animate_bool(id.with("focused"), focused);
    let hover = hover_fade(ui, &hover_response);
    let fill = mix(palette.surface, palette.surface_hover, hover.max(focus_t));
    let radius = CornerRadius::same((height / 2.0) as u8);
    ui.painter().rect_filled(rect, radius, fill);
    let border_color = mix(
        palette.outline.gamma_multiply(0.75),
        mix(palette.text.gamma_multiply(0.5), palette.accent, 0.45),
        focus_t,
    );
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0 + 0.5 * focus_t, border_color),
        egui::StrokeKind::Inside,
    );
    let icon = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(16.0, 16.0));
    Icon::Search
        .image(mix(palette.secondary, palette.text, focus_t), 16.0)
        .paint_at(ui, icon);
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
            .frame(egui::Frame::NONE)
            .hint_text(egui::RichText::new(hint).color(palette.dim))
            .font(theme::regular(14.0))
            .vertical_align(egui::Align::Center)
            .desired_width(field_rect.width()),
    );
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
        let clear_hover = hover_fade(ui, &clear_response);
        if clear_response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if clear_hover > 0.0 {
            ui.painter().circle_filled(
                button.center(),
                12.0,
                palette.surface_active.gamma_multiply(clear_hover),
            );
        }
        focus_outline(ui, &clear_response, CornerRadius::same(12));
        Icon::Close
            .image(mix(palette.secondary, palette.text, clear_hover), 15.0)
            .paint_at(
                ui,
                Rect::from_center_size(button.center(), vec2(15.0, 15.0)),
            );
        if clear_response.clicked() {
            text.clear();
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    }
    response
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
                Id::new("search"),
                240.0,
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
