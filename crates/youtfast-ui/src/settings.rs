//! The settings page, the siblings' way: the page's title, then each group
//! as a heading over a card of rows — the option's name and what it does
//! at the left, its control at the right. The column sits in the middle of
//! the page, with an index of the groups beside it when there is room.

mod about;
mod appearance;
mod preferences;
mod sidebar;

use egui::{Align, Color32, CornerRadius, Layout, Response, RichText, Sense, Ui, pos2, vec2};

use super::{results, widgets};
use youtfast_app::Action;
use youtfast_app::State;
use youtfast_appearance::theme::{self, Palette};
use youtfast_i18n::gettext as tr;

/// The widest the settings read well.
pub(crate) const WIDTH: f32 = 760.0;
/// The room a row keeps at its right for a usual control.
pub(crate) const CONTROL_WIDTH: f32 = 260.0;
/// The index of the groups a wide window shows beside the column, and the
/// space between the two.
const INDEX_WIDTH: f32 = 200.0;
const INDEX_GAP: f32 = 32.0;
/// The height of one entry of the index.
const INDEX_ROW: f32 = 34.0;
/// How far below the top of the page a group's heading already counts as
/// the current one.
const INDEX_REACH: f32 = 96.0;

/// The page's groups, in order: the name the index lists and the group.
const GROUPS: [(&str, Group); 6] = [
    ("Appearance", appearance::appearance),
    ("Playback", preferences::playback),
    ("Sidebar", sidebar::sidebar),
    ("History", preferences::history),
    ("Notifications", preferences::notifications),
    ("About", about::about),
];

type Group = fn(&mut Ui, &State, &mut Vec<Action>);

/// What the index remembers between frames.
#[derive(Clone, Copy, Default)]
struct IndexState {
    /// The group under the top of the page, as the last frame drew it.
    current: usize,
    /// The group to scroll to in this frame.
    jump: Option<usize>,
    /// The group last chosen in the index: it stays marked until the page
    /// is scrolled by hand, since the last groups never reach the top.
    chosen: Option<usize>,
}

/// How far from the left of a page `page_width` wide a block `block` wide
/// sits to be in the middle. Whole pixels: a half-pixel offset would blur
/// every glyph.
pub(crate) fn centered_left(page_width: f32, block: f32) -> f32 {
    ((page_width - block) / 2.0).max(0.0).floor()
}

/// What `add_contents` draws in a column `width` wide, `offset` from the
/// left of the room the ui has left.
pub(crate) fn column<R>(
    ui: &mut Ui,
    offset: f32,
    width: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let mut rect = ui.available_rect_before_wrap();
    rect.min.x += offset;
    rect.max.x = rect.min.x + width;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        // set_max_width also widens: a narrow window would cut
        // the text off at its edge instead of wrapping it.
        ui.set_max_width(width);
        add_contents(ui)
    })
    .inner
}

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    results::panel(ui, |ui| {
        let page = ui.available_rect_before_wrap();
        // A window with room for it lists the groups beside the column;
        // either way what is drawn sits in the middle of the page.
        let indexed = page.width() >= WIDTH + INDEX_WIDTH + INDEX_GAP;
        let width = WIDTH.min(page.width());
        let block = if indexed {
            width + INDEX_WIDTH + INDEX_GAP
        } else {
            width
        };
        let left = centered_left(page.width(), block);
        let offset = left + block - width;

        let id = egui::Id::new("settings-index");
        let mut index: IndexState = ui.ctx().data(|d| d.get_temp(id)).unwrap_or_default();
        if ui.input(|i| i.smooth_scroll_delta.y != 0.0 || i.pointer.is_decidedly_dragging()) {
            index.chosen = None;
        }
        let jump = index.jump.take();

        let mut current = 0;
        // The whole panel scrolls, so the wheel works beside the column
        // too and the bar stays at the window's edge.
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let top = ui.clip_rect().top();
                column(ui, offset, width, |ui| {
                    if !indexed {
                        results::title(ui, state, &tr(locale, "Settings"), None);
                    }
                    for (group, (_, draw)) in GROUPS.iter().enumerate() {
                        if indexed {
                            if jump == Some(group) {
                                ui.scroll_to_cursor(Some(Align::TOP));
                            }
                            if ui.cursor().top() <= top + INDEX_REACH {
                                current = group;
                            }
                        }
                        draw(ui, state, actions);
                        ui.add_space(24.0);
                    }
                });
            });

        if indexed {
            index.current = index.chosen.unwrap_or(current);
            let rect = egui::Rect::from_min_size(
                page.min + vec2(left, 0.0),
                vec2(INDEX_WIDTH, page.height()),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                results::title(ui, state, &tr(locale, "Settings"), None);
                if let Some(chosen) = group_index(ui, state, index.current) {
                    index.jump = Some(chosen);
                    index.chosen = Some(chosen);
                    index.current = chosen;
                }
            });
        }
        ui.ctx().data_mut(|d| d.insert_temp(id, index));
    });
}

/// The index beside the column: one entry per group, the current one in
/// the primary colour beside an accent bar that slides to it. Returns the
/// entry clicked.
fn group_index(ui: &mut Ui, state: &State, current: usize) -> Option<usize> {
    let locale = state.settings.language;
    let palette = &state.palette;
    let mut chosen = None;
    let first = ui.cursor().top();
    ui.spacing_mut().item_spacing.y = 2.0;
    for (group, (name, _)) in GROUPS.iter().enumerate() {
        let name = tr(locale, name);
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), INDEX_ROW), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                ui.is_enabled(),
                group == current,
                name.as_ref(),
            )
        });
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let hover = widgets::hover_fade(ui, &response);
        let on = ui
            .ctx()
            .animate_bool(response.id.with("current"), group == current);
        if hover > 0.0 {
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(theme::RADIUS_SMALL),
                palette.surface.gamma_multiply(hover),
            );
        }
        widgets::focus_outline(ui, &response, CornerRadius::same(theme::RADIUS_SMALL));
        let font = if group == current {
            theme::semibold(14.0)
        } else {
            theme::medium(14.0)
        };
        let color = widgets::mix(palette.secondary, palette.text, hover.max(on));
        let galley = ui.painter().layout_no_wrap(name.into_owned(), font, color);
        ui.painter().galley(
            pos2(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
        if response.clicked() {
            chosen = Some(group);
        }
    }
    // One bar for the whole index, so it travels between entries instead
    // of going out on one and coming on at the other.
    let step = INDEX_ROW + ui.spacing().item_spacing.y;
    let at = ui.ctx().animate_value_with_time(
        ui.id().with("bar"),
        current as f32,
        ui.style().animation_time * 1.5,
    );
    let bar = egui::Rect::from_min_size(
        pos2(ui.max_rect().left(), first + at * step + 8.0),
        vec2(3.0, INDEX_ROW - 16.0),
    );
    ui.painter()
        .rect_filled(bar, CornerRadius::same(2), palette.accent);
    chosen
}

/// A row's name in medium and, under it, what it does in secondary. An
/// empty name leaves only the description.
pub(crate) fn row_text(ui: &mut Ui, palette: &Palette, label: &str, description: &str) {
    if !label.is_empty() {
        ui.label(
            RichText::new(label)
                .font(theme::medium(14.0))
                .color(palette.text),
        );
    }
    if !description.is_empty() {
        ui.add(
            egui::Label::new(
                RichText::new(description)
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
            )
            .wrap(),
        );
    }
}

/// One option: its name in medium and what it does in secondary at the
/// left, the control at the right in `control_width`. In a card too
/// narrow for both, the control goes under the text.
pub(crate) fn setting_row(
    ui: &mut Ui,
    palette: &Palette,
    label: &str,
    description: &str,
    control_width: f32,
    control: impl FnOnce(&mut Ui),
) {
    row(
        ui,
        control_width,
        |ui| row_text(ui, palette, label, description),
        control,
    );
}

/// A row of a card: `text` at the left, `control` at the right in
/// `control_width`, on the same line while the card leaves the text at
/// least 160 for itself and under the text otherwise.
pub(crate) fn row(
    ui: &mut Ui,
    control_width: f32,
    text: impl FnOnce(&mut Ui),
    control: impl FnOnce(&mut Ui),
) {
    let reserved = control_width + 16.0;
    if ui.available_width() - reserved < 160.0 {
        ui.vertical(|ui| {
            text(ui);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), control);
            });
        });
    } else {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - reserved).max(0.0));
                text(ui);
            });
            ui.with_layout(Layout::right_to_left(Align::Center), control);
        });
    }
}

/// A [`setting_row`] whose control is a [`switch`]: the whole row takes
/// the click and lights up under the pointer. Returns whether it was
/// clicked. `name` is the switch's accessible name.
pub(super) fn switch_row(
    ui: &mut Ui,
    palette: &Palette,
    label: &str,
    description: &str,
    on: bool,
    name: &str,
) -> bool {
    let id = ui.id().with(("switch-row", label));
    // The row's click is read where the last frame drew it, before the
    // switch is added: the switch then stays above it and keeps its own
    // clicks and the keyboard's focus.
    let row = ui
        .ctx()
        .data(|d| d.get_temp::<egui::Rect>(id))
        .filter(|_| ui.is_enabled())
        .map(|rect| ui.interact(rect, id, Sense::CLICK));
    let fill = ui.painter().add(egui::Shape::Noop);
    let mut clicked = row.as_ref().is_some_and(Response::clicked);
    let drawn = ui
        .scope(|ui| {
            // A selectable label would take the click meant for the row.
            ui.style_mut().interaction.selectable_labels = false;
            setting_row(ui, palette, label, description, CONTROL_WIDTH, |ui| {
                clicked |= switch(ui, palette, on, name).clicked();
            });
        })
        .response
        .rect
        .expand2(vec2(6.0, 5.0));
    ui.ctx().data_mut(|d| d.insert_temp(id, drawn));
    let under = row.as_ref().is_some_and(Response::contains_pointer);
    if under {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let hover = ui.ctx().animate_bool(id.with("hover"), under);
    if hover > 0.0 {
        ui.painter().set(
            fill,
            egui::Shape::rect_filled(
                drawn,
                CornerRadius::same(theme::RADIUS_SMALL),
                palette.surface_hover.gamma_multiply(0.5 * hover),
            ),
        );
    }
    clicked
}

/// How wide a row's combo box is: at least 240, and enough for its widest
/// choice, so the button never outgrows the room `setting_row` keeps for it
/// and runs over the description. Mirrors egui's combo button: the text,
/// then the arrow, inside the button padding.
pub(super) fn combo_width(ui: &Ui, labels: impl IntoIterator<Item = impl Into<String>>) -> f32 {
    let spacing = ui.spacing();
    let chrome = spacing.icon_spacing + spacing.icon_width + 2.0 * spacing.button_padding.x;
    labels
        .into_iter()
        .map(|label| {
            egui::WidgetText::from(label.into())
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    egui::TextStyle::Button,
                )
                .size()
                .x
                + chrome
        })
        .fold(240.0, f32::max)
        .ceil()
}

/// An on/off switch: a pill, `accent` when on, its round knob sliding to
/// the side that is on and stretching while pressed. Dimmed while disabled.
/// The caller reads the click.
pub(super) fn switch(ui: &mut Ui, palette: &Palette, on: bool, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, on, label));
    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if ui.is_rect_visible(rect) {
        let t = ui
            .ctx()
            .animate_bool_with_easing(response.id, on, egui::emath::easing::cubic_out);
        let hover = widgets::hover_fade(ui, &response);
        let press = widgets::press_fade(ui, &response);
        let off = widgets::mix(palette.surface_active, palette.outline, hover);
        let fill = widgets::mix(off, palette.accent, t);
        let (fill, knob) = if enabled {
            (fill, Color32::WHITE)
        } else {
            (fill.gamma_multiply(0.5), Color32::WHITE.gamma_multiply(0.6))
        };
        ui.painter().rect_filled(rect, CornerRadius::same(11), fill);
        widgets::focus_outline(ui, &response, CornerRadius::same(11));
        // The knob stretches towards the other side while held, so the
        // press is felt before the switch moves.
        let stretch = 5.0 * press;
        let x = egui::lerp(
            rect.left() + 11.0 + stretch / 2.0..=rect.right() - 11.0 - stretch / 2.0,
            t,
        );
        let knob_rect =
            egui::Rect::from_center_size(pos2(x, rect.center().y), vec2(16.0 + stretch, 16.0));
        ui.painter()
            .rect_filled(knob_rect, CornerRadius::same(8), knob);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use youtfast_i18n::Locale;
    use youtfast_storage::settings::{HistorySync, PlaybackBuffer};

    /// Where each text of a frame was drawn.
    fn texts(output: &egui::FullOutput) -> Vec<(String, egui::Pos2)> {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((text.galley.text().to_owned(), text.pos)),
                _ => None,
            })
            .collect()
    }

    fn page(
        ctx: &egui::Context,
        state: &State,
        width: f32,
        events: Vec<egui::Event>,
        time: f64,
    ) -> Vec<(String, egui::Pos2)> {
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(width, 700.0),
                )),
                events,
                time: Some(time),
                ..Default::default()
            },
            |ui| show(ui, state, &mut actions),
        );
        let texts = texts(&output);
        output.textures_delta.clear();
        texts
    }

    fn left_of(texts: &[(String, egui::Pos2)], text: &str) -> Vec<f32> {
        texts
            .iter()
            .filter(|(drawn, _)| drawn == text)
            .map(|(_, pos)| pos.x)
            .collect()
    }

    #[test]
    fn the_column_fills_a_narrow_window_and_is_centred_in_a_wider_one() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let state = State::default();
        // Narrower than the column: it starts at the panel's margin.
        let narrow = page(&ctx, &state, 600.0, vec![], 0.0);
        assert_eq!(left_of(&narrow, "Settings"), [24.0]);
        assert_eq!(left_of(&narrow, "Playback"), [24.0]);
        // 1000 wide: 952 inside the margins, too little for the index, so
        // the 760 column starts at 24 + (952 - 760) / 2.
        let alone = page(&ctx, &state, 1000.0, vec![], 0.0);
        assert_eq!(left_of(&alone, "Settings"), [120.0]);
        assert_eq!(left_of(&alone, "Playback"), [120.0]);
        // 1600 wide: the index (200) and its gap (32) join the column, and
        // the three start at 24 + (1552 - 992) / 2. A group in view is
        // named over its card, then in the index.
        let indexed = page(&ctx, &state, 1600.0, vec![], 0.0);
        assert_eq!(left_of(&indexed, "Settings"), [304.0]);
        assert_eq!(left_of(&indexed, "Playback"), [536.0, 318.0]);
    }

    #[test]
    fn the_index_lists_the_groups_and_scrolls_to_the_one_clicked() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let state = State::default();
        let width = 1600.0;
        let first = page(&ctx, &state, width, vec![], 0.0);
        // Every group is named in the index, whether or not the page has
        // reached it; the ones in view are named again over their card.
        let index_left = left_of(&first, "Settings")[0] + 14.0;
        for (name, _) in GROUPS {
            assert!(left_of(&first, name).contains(&index_left), "{name}");
        }
        assert_eq!(left_of(&first, "Appearance").len(), 2);
        let heading = |texts: &[(String, egui::Pos2)]| {
            texts
                .iter()
                .filter(|(drawn, _)| drawn == "Notifications")
                .map(|(_, pos)| *pos)
                .max_by(|a, b| a.x.total_cmp(&b.x))
                .unwrap()
        };
        let entry = first
            .iter()
            .filter(|(drawn, _)| drawn == "Notifications")
            .map(|(_, pos)| *pos)
            .min_by(|a, b| a.x.total_cmp(&b.x))
            .unwrap();
        // Only the index names it: its card is below the window.
        assert_eq!(left_of(&first, "Notifications"), [index_left]);
        let at = entry + vec2(10.0, 8.0);
        page(
            &ctx,
            &state,
            width,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
            0.1,
        );
        page(
            &ctx,
            &state,
            width,
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            0.2,
        );
        let mut last = Vec::new();
        for frame in 0..60 {
            last = page(&ctx, &state, width, vec![], 0.3 + f64::from(frame) * 0.05);
        }
        let after = heading(&last);
        assert!(after.x > index_left && after.y < 700.0, "{after:?}");
        // The entry in the index stayed where it was.
        assert!(last.contains(&("Notifications".to_owned(), entry)));
    }

    #[test]
    fn a_switch_row_takes_the_click_anywhere_on_it() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let state = State::default();
        let draw = |events: Vec<egui::Event>, actions: &mut Vec<Action>| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| preferences::playback(ui, &state, actions),
            );
            let texts = texts(&output);
            output.textures_delta.clear();
            texts
        };
        let mut actions = Vec::new();
        let first = draw(vec![], &mut actions);
        let label = first
            .iter()
            .find(|(drawn, _)| drawn == "Autoplay")
            .map(|(_, pos)| *pos)
            .unwrap();
        // On the option's name, far from the switch at the right.
        let at = label + vec2(20.0, 6.0);
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        // The row takes its click where the frame before drew it.
        draw(vec![egui::Event::PointerMoved(at)], &mut actions);
        draw(vec![button(true)], &mut actions);
        draw(vec![button(false)], &mut actions);
        assert_eq!(actions, vec![Action::SetAutoplay(!state.settings.autoplay)]);
    }

    #[test]
    fn selecting_a_language_emits_an_action_and_redraws_the_same_settings_page() {
        fn render(
            ctx: &egui::Context,
            state: &State,
            events: Vec<egui::Event>,
            actions: &mut Vec<Action>,
        ) -> Vec<(String, egui::Pos2)> {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| show(ui, state, actions),
            );
            let labels = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((
                        text.galley.text().to_owned(),
                        text.pos + text.galley.size() / 2.0,
                    )),
                    _ => None,
                })
                .collect();
            output.textures_delta.clear();
            labels
        }
        fn click(ctx: &egui::Context, state: &State, pos: egui::Pos2, actions: &mut Vec<Action>) {
            for pressed in [true, false] {
                render(
                    ctx,
                    state,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    actions,
                );
            }
        }
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        for (locale, expected_title) in [
            (Locale::PortugueseBrazil, "Configurações"),
            (Locale::Spanish, "Configuración"),
            (Locale::French, "Paramètres"),
            (Locale::German, "Einstellungen"),
            (Locale::Italian, "Impostazioni"),
            (Locale::English, "Settings"),
        ] {
            let labels = render(&ctx, &state, Vec::new(), &mut actions);
            let picker = labels
                .iter()
                .find(|(label, _)| label == state.settings.language.native_name())
                .unwrap()
                .1;
            click(&ctx, &state, picker, &mut actions);
            let labels = render(&ctx, &state, Vec::new(), &mut actions);
            let option = labels
                .iter()
                .find(|(label, _)| label == locale.native_name())
                .unwrap()
                .1;
            click(&ctx, &state, option, &mut actions);
            assert_eq!(actions, [Action::SetLanguage(locale)]);
            actions.clear();
            state.settings.language = locale;
            let labels = render(&ctx, &state, Vec::new(), &mut actions);
            assert!(labels.iter().any(|(label, _)| label == expected_title));
        }
    }

    #[test]
    fn combo_rows_keep_their_description_clear_of_the_combo_in_every_language() {
        fn assert_clear(ctx: &egui::Context, selected: &str, view: impl FnMut(&mut Ui)) {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 800.0),
                    )),
                    ..Default::default()
                },
                view,
            );
            let texts: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((
                        text.galley.text().to_owned(),
                        text.galley.rect.translate(text.pos.to_vec2()),
                    )),
                    _ => None,
                })
                .collect();
            let combo = texts
                .iter()
                .find(|(text, _)| text == selected)
                .map(|(_, rect)| *rect)
                .unwrap_or_else(|| panic!("{selected:?} is drawn"));
            for (text, rect) in &texts {
                if text != selected && rect.y_range().intersects(combo.y_range()) {
                    assert!(
                        rect.right() <= combo.left(),
                        "{text:?} runs under the combo showing {selected:?}"
                    );
                }
            }
            output.textures_delta.clear();
        }

        for &locale in Locale::ALL {
            let ctx = egui::Context::default();
            theme::install_fonts(&ctx);
            let mut state = State::default();
            state.settings.language = locale;
            for (buffer, label) in [
                (
                    PlaybackBuffer::Standard,
                    "Large — 150 MiB / 50 MiB (mpv default)",
                ),
                (
                    PlaybackBuffer::Compact,
                    "Small — 50 MiB / 20 MiB (less RAM)",
                ),
            ] {
                state.settings.playback_buffer = buffer;
                assert_clear(&ctx, &tr(locale, label), |ui| {
                    preferences::playback(ui, &state, &mut Vec::new())
                });
            }
            for (sync, label) in [
                (HistorySync::Separate, "Separate"),
                (HistorySync::Merged, "Merged in YouTFast"),
                (HistorySync::SendToYoutube, "Merged and sent to YouTube"),
            ] {
                state.settings.history_sync = sync;
                assert_clear(&ctx, &tr(locale, label), |ui| {
                    preferences::history(ui, &state, &mut Vec::new())
                });
            }
        }
    }
}
