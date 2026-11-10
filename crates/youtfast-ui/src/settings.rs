//! The settings page, the siblings' way: the page's title, then each group
//! as a heading over a card of rows — the option's name and what it does
//! at the left, its control at the right. The column sits in the middle of
//! the page, with an index of the groups beside it when there is room.

use egui::{Align, Color32, CornerRadius, Layout, Response, RichText, Sense, Ui, pos2, vec2};

use super::{results, widgets};
use youtfast_app::Action;
use youtfast_app::State;
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme::{self, InterfaceFont, Palette};
use youtfast_i18n::{Locale, format as trf, gettext as tr};
use youtfast_storage::settings::{
    HistorySync, PlaybackBuffer, SidebarButton, ThemeChoice, default_sidebar,
};

/// The widest the settings read well.
const WIDTH: f32 = 760.0;
/// The room a row keeps at its right for a usual control.
const CONTROL_WIDTH: f32 = 260.0;
/// The index of the groups a wide window shows beside the column, and the
/// space between the two.
const INDEX_WIDTH: f32 = 200.0;
const INDEX_GAP: f32 = 32.0;
/// The height of one entry of the index.
const INDEX_ROW: f32 = 34.0;
/// How far below the top of the page a group's heading already counts as
/// the current one.
const INDEX_REACH: f32 = 96.0;
/// How often the checks may run, in minutes.
const INTERVALS: [u32; 4] = [5, 15, 30, 60];

/// The page's groups, in order: the name the index lists and the group.
const GROUPS: [(&str, Group); 6] = [
    ("Appearance", appearance),
    ("Playback", playback),
    ("Sidebar", sidebar),
    ("History", history),
    ("Notifications", notifications),
    ("About", about),
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
        // Whole pixels: a half-pixel offset would blur every glyph.
        let left = ((page.width() - block) / 2.0).max(0.0).floor();
        let column = left + block - width;

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
                let mut rect = ui.available_rect_before_wrap();
                rect.min.x += column;
                rect.max.x = rect.min.x + width;
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    // set_max_width also widens: a narrow window would cut
                    // the text off at its edge instead of wrapping it.
                    ui.set_max_width(width);
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

/// The Appearance group: the theme, its status, where palettes live.
fn appearance(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Appearance"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let description = if state.follows_omarchy {
                tr(locale, "Automatic follows your Omarchy theme.")
            } else {
                tr(locale, "Automatic uses YouTFast's dark theme.")
            };
            setting_row(
                ui,
                palette,
                &tr(locale, "Language"),
                &tr(locale, "Choose the language used by the interface."),
                CONTROL_WIDTH,
                |ui| {
                    language_picker(ui, state, actions);
                },
            );
            ui.add_space(12.0);
            setting_row(
                ui,
                palette,
                &tr(locale, "Interface font"),
                &tr(
                    locale,
                    "The font that draws the interface. System uses your desktop's own.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    interface_font_picker(ui, state, actions);
                },
            );
            if state.settings.interface_font == InterfaceFont::System && state.system_font_missing {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(tr(
                        locale,
                        "Your system's interface font couldn't be loaded, so YouTFast uses Inter.",
                    ))
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
                );
                ui.add_space(4.0);
                if widgets::button(
                    ui,
                    palette,
                    &tr(locale, "Report this"),
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
                {
                    actions.push(Action::ReportInterfaceFont);
                }
            }
            ui.add_space(12.0);
            setting_row(
                ui,
                palette,
                &tr(locale, "Theme"),
                &description,
                CONTROL_WIDTH,
                |ui| {
                    theme_picker(ui, state, actions);
                },
            );
            if !state.theme_status.is_empty() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(youtfast_i18n::message(locale, &state.theme_status))
                        .font(theme::regular(13.0))
                        .color(palette.secondary),
                );
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(trf(
                    locale,
                    "Palettes are JSON files in {path}",
                    &[("path", state.themes_dir.clone())],
                ))
                .font(theme::regular(13.0))
                .color(palette.secondary),
            );
        });
    });
}

fn language_picker(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    egui::ComboBox::from_id_salt("interface-language")
        .width(200.0)
        .selected_text(state.settings.language.native_name())
        .show_ui(ui, |ui| {
            for &locale in Locale::ALL {
                if ui
                    .selectable_label(state.settings.language == locale, locale.native_name())
                    .clicked()
                {
                    actions.push(Action::SetLanguage(locale));
                    ui.close();
                }
            }
        });
}

/// The interface font's menu: the system's, or Inter.
fn interface_font_picker(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let name = |font: InterfaceFont| match font {
        InterfaceFont::System => tr(locale, "System (default)").into_owned(),
        InterfaceFont::Inter => "Inter".to_owned(),
    };
    egui::ComboBox::from_id_salt("interface-font")
        .width(200.0)
        .selected_text(
            RichText::new(name(state.settings.interface_font)).font(theme::regular(14.0)),
        )
        .show_ui(ui, |ui| {
            for font in InterfaceFont::ALL {
                if ui
                    .selectable_label(
                        state.settings.interface_font == font,
                        RichText::new(name(font)).font(theme::regular(14.0)),
                    )
                    .clicked()
                {
                    actions.push(Action::SetInterfaceFont(font));
                    ui.close();
                }
            }
        });
}

/// The theme's menu: Automatic, Dark, Light, then the palette files.
fn theme_picker(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    // The desktop's own palette: "Automatic" covers it.
    let mut choices = vec![
        (ThemeChoice::Auto, tr(locale, "Automatic").into_owned()),
        (ThemeChoice::Dark, tr(locale, "Dark").into_owned()),
        (ThemeChoice::Light, tr(locale, "Light").into_owned()),
    ];
    choices.extend(
        state
            .themes
            .iter()
            .filter(|(filename, _)| filename != fastframe_theme::omarchy::FILENAME)
            .map(|(filename, name)| (ThemeChoice::Custom(filename.clone()), name.clone())),
    );
    let current = choices
        .iter()
        .find(|(choice, _)| *choice == state.settings.theme)
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| match &state.settings.theme {
            ThemeChoice::Custom(filename) => filename.clone(),
            _ => String::new(),
        });
    egui::ComboBox::from_id_salt("theme")
        .width(200.0)
        .selected_text(RichText::new(current).font(theme::regular(14.0)))
        .show_ui(ui, |ui| {
            for (choice, name) in choices {
                let selected = state.settings.theme == choice;
                if ui
                    .selectable_label(selected, RichText::new(name).font(theme::regular(14.0)))
                    .clicked()
                {
                    actions.push(Action::SetTheme(choice));
                }
            }
        });
}

/// The Playback group: autoplay, scrolling mini player, video buffer, SponsorBlock.
fn playback(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Playback"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let autoplay = state.settings.autoplay;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Autoplay"),
                &tr(
                    locale,
                    "At the end of a video, the next one starts after a countdown: first the queue, then the source list, then related videos.",
                ),
                autoplay,
                &tr(locale, "Autoplay"),
            ) {
                actions.push(Action::SetAutoplay(!autoplay));
            }
            ui.add_space(12.0);
            let scroll_miniplayer = state.settings.scroll_miniplayer;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Miniplayer while scrolling"),
                &tr(
                    locale,
                    "Keep the video visible when scrolling down to the comments.",
                ),
                scroll_miniplayer,
                &tr(locale, "Miniplayer while scrolling"),
            ) {
                actions.push(Action::SetScrollMiniplayer(!scroll_miniplayer));
            }
            ui.add_space(12.0);
            let buffer_choices = [
                (
                    PlaybackBuffer::Standard,
                    "Large — 150 MiB / 50 MiB (mpv default)",
                ),
                (
                    PlaybackBuffer::Compact,
                    "Small — 50 MiB / 20 MiB (less RAM)",
                ),
            ];
            let current_buffer = state.settings.playback_buffer;
            let buffer_description = match current_buffer {
                PlaybackBuffer::Standard => {
                    "Large (150 MiB ahead, 50 MiB back) buffers several minutes ahead for unstable connections and instant seeking, using more RAM (~300–650 MB) and bandwidth if you skip videos early. Small (50 MiB / 20 MiB) uses less RAM (~200 MB) and data, but holds only ~15–90s ahead."
                }
                PlaybackBuffer::Compact => {
                    "Small (50 MiB ahead, 20 MiB back) keeps RAM usage low (~200 MB) and wastes less data when skipping videos, but holds only ~15–90s ahead and re-downloads when seeking far back. Large (150 MiB / 50 MiB) buffers several minutes ahead using more RAM (~300–650 MB)."
                }
            };
            let buffer_width = combo_width(
                ui,
                buffer_choices.iter().map(|(_, label)| tr(locale, label)),
            );
            setting_row(
                ui,
                palette,
                &tr(locale, "Video buffer"),
                &tr(locale, buffer_description),
                buffer_width.max(CONTROL_WIDTH),
                |ui| {
                    egui::ComboBox::from_id_salt("playback-buffer")
                        .width(buffer_width)
                        .selected_text(tr(
                            locale,
                            buffer_choices
                                .iter()
                                .find(|(choice, _)| *choice == current_buffer)
                                .unwrap()
                                .1,
                        ))
                        .show_ui(ui, |ui| {
                            for (choice, label) in buffer_choices {
                                if ui
                                    .selectable_label(current_buffer == choice, tr(locale, label))
                                    .clicked()
                                {
                                    actions.push(Action::SetPlaybackBuffer(choice));
                                    ui.close();
                                }
                            }
                        });
                },
            );
            ui.add_space(12.0);
            let sponsorblock = state.settings.sponsorblock;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Skip sponsors (SponsorBlock)"),
                &tr(
                    locale,
                    "Skips sponsors and offers to skip intros, credits and other segments marked by the SponsorBlock community. Requests to sponsor.ajay.app use a hash prefix matching thousands of videos.",
                ),
                sponsorblock,
                "SponsorBlock",
            ) {
                actions.push(Action::SetSponsorBlock(!sponsorblock));
            }
        });
    });
}

/// The Sidebar group: each button with a switch to show it and arrows to
/// move it. Account and Settings stay shown. Number keys follow this order.
fn sidebar(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Sidebar"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let card = widgets::on_card(palette);
            ui.add(
                egui::Label::new(
                    RichText::new(tr(
                        locale,
                        "Choose which buttons the sidebar shows and in what order. The number keys 1 to 9 follow the buttons as shown. Account and Settings always stay.",
                    ))
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            let entries = &state.settings.sidebar;
            let last = entries.len() - 1;
            // A row is as tall as its arrows.
            let step = 32.0 + ui.spacing().item_spacing.y;
            for (index, entry) in entries.iter().enumerate() {
                let name = sidebar_button_name(locale, entry.button);
                // A global id keyed by button (`push_id`'s widget ids still
                // follow the row's position), so focus and the switch's
                // animation travel with the row when it moves.
                let row = egui::UiBuilder::new().id(egui::Id::new(("sidebar-row", entry.button)));
                // A moved row slides from the place it had: the picture
                // only, the row itself is already where it goes.
                let place = ui.ctx().animate_value_with_time(
                    egui::Id::new(("sidebar-place", entry.button)),
                    index as f32,
                    ui.style().animation_time * 1.5,
                );
                let slide = vec2(0.0, (place - index as f32) * step);
                ui.with_visual_transform(egui::emath::TSTransform::from_translation(slide), |ui| {
                    ui.scope_builder(row, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.add_enabled_ui(entry.button.can_hide(), |ui| {
                                if switch(ui, palette, entry.visible, &name).clicked() {
                                    let mut next = entries.clone();
                                    next[index].visible = !entry.visible;
                                    actions.push(Action::SetSidebar(next));
                                }
                            });
                            ui.label(
                                RichText::new(&name)
                                    .font(theme::medium(14.0))
                                    .color(palette.text),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                for (icon, up, key, enabled) in [
                                    (Icon::ChevronDown, false, "Move {name} down", index < last),
                                    (Icon::ChevronUp, true, "Move {name} up", index > 0),
                                ] {
                                    let hint = trf(locale, key, &[("name", name.clone())]);
                                    let moved = ui
                                        .add_enabled_ui(enabled, |ui| {
                                            widgets::icon_button(ui, icon, 18.0, &hint)
                                        })
                                        .inner;
                                    if moved.clicked() {
                                        let to = if up {
                                            index.checked_sub(1)
                                        } else {
                                            Some(index + 1).filter(|to| *to <= last)
                                        };
                                        if let Some(to) = to {
                                            let mut next = entries.clone();
                                            next.swap(index, to);
                                            actions.push(Action::SetSidebar(next));
                                        }
                                    }
                                }
                            });
                        })
                    })
                });
            }
            ui.add_space(8.0);
            ui.add_enabled_ui(*entries != default_sidebar(), |ui| {
                if widgets::button(
                    ui,
                    &card,
                    &tr(locale, "Restore default"),
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
                {
                    actions.push(Action::SetSidebar(default_sidebar()));
                }
            });
        });
    });
}

/// A sidebar button's name. Liked and Playlists are listed even while
/// signed out: they only show when signed in.
fn sidebar_button_name(locale: Locale, button: SidebarButton) -> String {
    match button {
        SidebarButton::Home => tr(locale, "Home"),
        SidebarButton::Subscriptions => tr(locale, "Subscriptions"),
        SidebarButton::History => tr(locale, "History"),
        SidebarButton::WatchLater => tr(locale, "Watch later"),
        SidebarButton::Liked => tr(locale, "Liked videos"),
        SidebarButton::Playlists => tr(locale, "Playlists"),
        SidebarButton::Notifications => tr(locale, "Notifications"),
        SidebarButton::Account => tr(locale, "Account"),
        SidebarButton::Settings | SidebarButton::Unknown => tr(locale, "Settings"),
    }
    .into_owned()
}

/// History keeps the account reporting choice visible beside its control.
fn history(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "History"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let choices = [
                (HistorySync::Separate, "Separate"),
                (HistorySync::Merged, "Merged in YouTFast"),
                (HistorySync::SendToYoutube, "Merged and sent to YouTube"),
            ];
            let current = state.settings.history_sync;
            let description = match current {
                HistorySync::Separate => {
                    "Keep account and device history separate. Nothing is sent to YouTube."
                }
                HistorySync::Merged => {
                    "Combine account and device history in YouTFast. Nothing is sent to YouTube."
                }
                HistorySync::SendToYoutube => {
                    "Combine history and send playback reports to Google from your account. Google may treat this as automated access."
                }
            };
            let sync_width = combo_width(ui, choices.iter().map(|(_, label)| tr(locale, label)));
            setting_row(
                ui,
                palette,
                &tr(locale, "History sync"),
                &tr(locale, description),
                sync_width.max(CONTROL_WIDTH),
                |ui| {
                    egui::ComboBox::from_id_salt("history-sync")
                        .width(sync_width)
                        .selected_text(tr(
                            locale,
                            choices
                                .iter()
                                .find(|(choice, _)| *choice == current)
                                .unwrap()
                                .1,
                        ))
                        .show_ui(ui, |ui| {
                            for (choice, label) in choices {
                                if ui
                                    .selectable_label(current == choice, tr(locale, label))
                                    .clicked()
                                {
                                    actions.push(Action::SetHistorySync(choice));
                                    ui.close();
                                }
                            }
                        });
                },
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr(locale, "Applies to signed-in accounts only."))
                    .font(theme::regular(13.0))
                    .color(palette.secondary),
            );
        });
    });
}

/// The Notifications group: the check, desktop popups, the interval.
fn notifications(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "Notifications"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let on = state.settings.alerts;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Check for new uploads"),
                &tr(
                    locale,
                    "Checks the same sources as Home. YouTube's notifications have no API, so the app watches subscribed channels instead.",
                ),
                on,
                &tr(locale, "Check for new uploads"),
            ) {
                actions.push(Action::SetAlerts(!on));
            }
            ui.add_space(12.0);
            let desktop = state.settings.desktop_notifications;
            setting_row(
                ui,
                palette,
                &tr(locale, "Desktop notifications"),
                &tr(
                    locale,
                    "Shows a desktop notification for each new video found.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    // Greyed: it only matters while the checks run.
                    let response = ui
                        .add_enabled_ui(on, |ui| {
                            switch(ui, palette, desktop, &tr(locale, "Desktop notifications"))
                        })
                        .inner;
                    if on && response.clicked() {
                        actions.push(Action::SetDesktopNotifications(!desktop));
                    }
                },
            );
            ui.add_space(12.0);
            let chips = 4.0 * 72.0 + 3.0 * 8.0;
            setting_row(
                ui,
                palette,
                &tr(locale, "Check every"),
                &tr(locale, "Time between checks."),
                chips,
                |ui| {
                    let raised = widgets::on_card(palette);
                    // Right to left: the longest interval goes in first.
                    for minutes in INTERVALS.into_iter().rev() {
                        let picked = state.settings.alerts_every_minutes == minutes;
                        if widgets::chip(
                            ui,
                            &raised,
                            &trf(locale, "{minutes} min", &[("minutes", minutes.to_string())]),
                            picked,
                        )
                        .clicked()
                        {
                            actions.push(Action::SetAlertsEvery(minutes));
                        }
                    }
                },
            );
        });
    });
}

/// The About group: installed version, update notifications, automatic
/// minor/patch updates, manual major update button, and changelog notes.
fn about(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    use youtfast_appearance::icons::Icon;
    use youtfast_desktop::updater::{self, UpdateState};

    let locale = state.settings.language;
    let palette = &state.palette;
    widgets::section(ui, palette, &tr(locale, "About"), None, |ui| {
        widgets::card(ui, palette, |ui| {
            let version_title = trf(
                locale,
                "YouTFast {version}",
                &[("version", updater::CURRENT_VERSION.to_owned())],
            );
            let status_description = match &state.update {
                UpdateState::Idle => tr(
                    locale,
                    "Small native YouTube client built with Rust, egui, and libmpv (GPL-3.0-only).",
                )
                .into_owned(),
                UpdateState::Checking => tr(locale, "Checking for updates…").into_owned(),
                UpdateState::UpToDate => tr(locale, "YouTFast is up to date.").into_owned(),
                UpdateState::Available(release) if release.kind.is_major() => trf(
                    locale,
                    "Major version v{version} is available. Major updates require manual installation.",
                    &[("version", release.version.clone())],
                ),
                UpdateState::Available(release) => trf(
                    locale,
                    "Version v{version} is available.",
                    &[("version", release.version.clone())],
                ),
                UpdateState::Installing(release) => trf(
                    locale,
                    "Downloading and installing YouTFast v{version}…",
                    &[("version", release.version.clone())],
                ),
                UpdateState::Installed {
                    version,
                    auto: true,
                    ..
                } => trf(
                    locale,
                    "Automatically updated to v{version}. Restart YouTFast to use the new version.",
                    &[("version", version.clone())],
                ),
                UpdateState::Installed {
                    version,
                    auto: false,
                    ..
                } => trf(
                    locale,
                    "Updated to v{version}. Restart YouTFast to use the new version.",
                    &[("version", version.clone())],
                ),
                UpdateState::Failed(error) => youtfast_i18n::message(locale, error).into_owned(),
            };
            setting_row(
                ui,
                palette,
                &version_title,
                &status_description,
                CONTROL_WIDTH,
                |ui| {
                    let raised = widgets::on_card(palette);
                    match &state.update {
                        UpdateState::Available(release) => {
                            let label = trf(
                                locale,
                                "Update to v{version}",
                                &[("version", release.version.clone())],
                            );
                            if widgets::button(ui, &raised, &label, widgets::ButtonStyle::Primary)
                                .clicked()
                            {
                                actions.push(Action::InstallUpdate);
                            }
                        }
                        UpdateState::Checking | UpdateState::Installing(_) => {
                            widgets::spinner(ui, 16.0);
                        }
                        _ => {
                            if widgets::icon_text_button(
                                ui,
                                &raised,
                                Some(Icon::Refresh),
                                &tr(locale, "Check for updates"),
                                widgets::ButtonStyle::Normal,
                            )
                            .clicked()
                            {
                                actions.push(Action::CheckForUpdates);
                            }
                        }
                    }
                },
            );
            ui.add_space(12.0);
            let notify_on = state.settings.update_notifications;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Notify about new versions"),
                &tr(
                    locale,
                    "Show a system notification when a new version is available and ask if you want to update.",
                ),
                notify_on,
                &tr(locale, "Notify about new versions"),
            ) {
                actions.push(Action::SetUpdateNotifications(!notify_on));
            }
            ui.add_space(12.0);
            let auto_update = state.settings.auto_update;
            if switch_row(
                ui,
                palette,
                &tr(locale, "Automatic updates (minor and patch)"),
                &tr(
                    locale,
                    "Automatically download and install minor (0.x.0) and patch (0.0.x) updates and display the changelog of what changed. Major versions (x.0.0) always require clicking Update manually.",
                ),
                auto_update,
                &tr(locale, "Automatic updates (minor and patch)"),
            ) {
                actions.push(Action::SetAutoUpdate(!auto_update));
            }

            let active_changelog: Option<(String, String)> = match &state.update {
                UpdateState::Available(release) | UpdateState::Installing(release)
                    if !release.changelog.is_empty() =>
                {
                    Some((
                        trf(
                            locale,
                            "What's new in v{version}",
                            &[("version", release.version.clone())],
                        ),
                        release.changelog.clone(),
                    ))
                }
                UpdateState::Installed {
                    version, changelog, ..
                } if !changelog.is_empty() => Some((
                    trf(
                        locale,
                        "What changed in v{version} (Changelog)",
                        &[("version", version.clone())],
                    ),
                    changelog.clone(),
                )),
                _ if state.full_changelog_open => match (
                    &state.settings.last_updated_version,
                    &state.settings.last_updated_changelog,
                ) {
                    (Some(version), Some(changelog))
                        if !changelog.is_empty()
                            && updater::Version::parse(version)
                                >= updater::Version::parse(updater::CURRENT_VERSION) =>
                    {
                        Some((
                            trf(
                                locale,
                                "What changed in v{version} (Changelog)",
                                &[("version", version.clone())],
                            ),
                            changelog.clone(),
                        ))
                    }
                    _ => {
                        let notes = updater::bundled_changelog_for(updater::CURRENT_VERSION);
                        (!notes.is_empty()).then(|| {
                            (
                                trf(
                                    locale,
                                    "What changed in v{version} (Changelog)",
                                    &[("version", updater::CURRENT_VERSION.to_owned())],
                                ),
                                notes,
                            )
                        })
                    }
                },
                _ => None,
            };

            if let Some((heading, notes)) = active_changelog {
                ui.add_space(14.0);
                let inner = widgets::on_card(palette);
                egui::Frame::new()
                    .fill(inner.surface)
                    .corner_radius(CornerRadius::same(theme::RADIUS_SMALL))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(heading)
                                .font(theme::semibold(13.5))
                                .color(palette.text),
                        );
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(notes)
                                    .font(theme::regular(13.0))
                                    .color(palette.secondary),
                            )
                            .wrap(),
                        );
                    });
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let raised = widgets::on_card(palette);
                let toggle_label = if state.full_changelog_open {
                    tr(locale, "Hide changelog")
                } else {
                    tr(locale, "View changelog")
                };
                if widgets::button(ui, &raised, &toggle_label, widgets::ButtonStyle::Normal)
                    .clicked()
                {
                    actions.push(Action::ToggleFullChangelog);
                }
                if widgets::icon_text_button(
                    ui,
                    &raised,
                    Some(Icon::ExternalLink),
                    &tr(locale, "Releases on GitHub"),
                    widgets::ButtonStyle::Normal,
                )
                .clicked()
                {
                    actions.push(Action::OpenUrl(updater::RELEASES_URL.to_owned()));
                }
            });
        });
    });
}

/// One option: its name in medium and what it does in secondary at the
/// left, the control at the right in `control_width`. In a card too
/// narrow for both, the control goes under the text.
fn setting_row(
    ui: &mut Ui,
    palette: &Palette,
    label: &str,
    description: &str,
    control_width: f32,
    control: impl FnOnce(&mut Ui),
) {
    let text = |ui: &mut Ui| {
        ui.label(
            RichText::new(label)
                .font(theme::medium(14.0))
                .color(palette.text),
        );
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
    };
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
fn switch_row(
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
fn combo_width(ui: &Ui, labels: impl IntoIterator<Item = impl Into<String>>) -> f32 {
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
fn switch(ui: &mut Ui, palette: &Palette, on: bool, label: &str) -> Response {
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
                |ui| playback(ui, &state, actions),
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
    fn history_reporting_disclosure_is_visible_in_both_themes_and_widths() {
        for light in [false, true] {
            for width in [360.0, 900.0] {
                let ctx = egui::Context::default();
                theme::install_fonts(&ctx);
                let mut state = State::default();
                state.settings.history_sync = HistorySync::SendToYoutube;
                state.palette = if light {
                    Palette::light()
                } else {
                    Palette::dark()
                };
                let mut actions = Vec::new();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(width, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| history(ui, &state, &mut actions),
                );
                let labels: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some(text.galley.text()),
                        _ => None,
                    })
                    .collect();
                assert!(labels.contains(&"Merged and sent to YouTube"));
                assert!(labels.contains(&"Combine history and send playback reports to Google from your account. Google may treat this as automated access."));
                assert!(labels.contains(&"Applies to signed-in accounts only."));
                assert!(actions.is_empty());
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn the_sidebar_group_lists_every_button_and_restores_the_default() {
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
                        vec2(900.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| sidebar(ui, state, actions),
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
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        let labels = render(&ctx, &state, Vec::new(), &mut actions);
        for name in [
            "Home",
            "Subscriptions",
            "History",
            "Watch later",
            "Liked videos",
            "Playlists",
            "Notifications",
            "Account",
            "Settings",
            "Restore default",
        ] {
            assert!(labels.iter().any(|(text, _)| text == name), "{name}");
        }
        // Nothing to restore yet: the button is disabled and silent.
        let restore = labels
            .iter()
            .find(|(t, _)| t == "Restore default")
            .unwrap()
            .1;
        for pressed in [true, false] {
            render(
                &ctx,
                &state,
                vec![
                    egui::Event::PointerMoved(restore),
                    egui::Event::PointerButton {
                        pos: restore,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                &mut actions,
            );
        }
        assert!(actions.is_empty());
        // Once changed, it asks for the default list back.
        state.settings.sidebar.reverse();
        let labels = render(&ctx, &state, Vec::new(), &mut actions);
        let restore = labels
            .iter()
            .find(|(t, _)| t == "Restore default")
            .unwrap()
            .1;
        for pressed in [true, false] {
            render(
                &ctx,
                &state,
                vec![
                    egui::Event::PointerMoved(restore),
                    egui::Event::PointerButton {
                        pos: restore,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                &mut actions,
            );
        }
        assert_eq!(actions, vec![Action::SetSidebar(default_sidebar())]);
    }

    #[test]
    fn keyboard_focus_follows_a_moved_sidebar_row() {
        fn frame(
            ctx: &egui::Context,
            state: &State,
            events: Vec<egui::Event>,
            actions: &mut Vec<Action>,
        ) {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| sidebar(ui, state, actions),
            );
            output.textures_delta.clear();
        }
        fn key(key: egui::Key) -> Vec<egui::Event> {
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]
        }
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        frame(&ctx, &state, Vec::new(), &mut actions);
        // Home's switch, then Home's "move down".
        for _ in 0..2 {
            frame(&ctx, &state, key(egui::Key::Tab), &mut actions);
        }
        let focused = ctx.memory(|m| m.focused()).expect("a focused widget");
        frame(&ctx, &state, key(egui::Key::Enter), &mut actions);
        frame(&ctx, &state, Vec::new(), &mut actions);
        let Some(Action::SetSidebar(next)) = actions.pop() else {
            panic!("moving down emits the new list");
        };
        assert_eq!(next[0].button, SidebarButton::Subscriptions);
        assert_eq!(next[1].button, SidebarButton::Home);
        frame(&ctx, &state, Vec::new(), &mut actions);
        state.settings.sidebar = next;
        frame(&ctx, &state, Vec::new(), &mut actions);
        frame(&ctx, &state, Vec::new(), &mut actions);
        // The ids are keyed by button: focus stays on Home's arrow, which
        // is now in the second row, instead of the first row's.
        assert_eq!(ctx.memory(|m| m.focused()), Some(focused));
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

    /// Draws the Appearance group and returns its text, with the centre of
    /// each, after the pointer events.
    fn draw_appearance(
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
            |ui| appearance(ui, state, actions),
        );
        output.textures_delta.clear();
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((
                    text.galley.text().to_owned(),
                    text.pos + text.galley.size() / 2.0,
                )),
                _ => None,
            })
            .collect()
    }

    fn click_at(ctx: &egui::Context, state: &State, pos: egui::Pos2, actions: &mut Vec<Action>) {
        for pressed in [true, false] {
            draw_appearance(
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

    fn centre_of(labels: &[(String, egui::Pos2)], text: &str) -> egui::Pos2 {
        labels
            .iter()
            .find(|(label, _)| label == text)
            .unwrap_or_else(|| panic!("{text} is not drawn"))
            .1
    }

    const NOTICE: &str = "Your system's interface font couldn't be loaded, so YouTFast uses Inter.";

    #[test]
    fn the_font_notice_shows_only_when_the_systems_face_fell_back_to_inter() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        for (font, missing, shown) in [
            (InterfaceFont::System, false, false),
            (InterfaceFont::System, true, true),
            // Choosing Inter is not a failure.
            (InterfaceFont::Inter, true, false),
            (InterfaceFont::Inter, false, false),
        ] {
            let mut state = State::default();
            state.settings.interface_font = font;
            state.system_font_missing = missing;
            let labels = draw_appearance(&ctx, &state, Vec::new(), &mut Vec::new());
            let has = |text: &str| labels.iter().any(|(label, _)| label == text);
            assert_eq!(has(NOTICE), shown, "{font:?} {missing}");
            assert_eq!(has("Report this"), shown, "{font:?} {missing}");
            assert!(has("Interface font"));
        }
    }

    #[test]
    fn report_this_asks_to_open_the_issue_and_nothing_else() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let state = State {
            system_font_missing: true,
            ..State::default()
        };
        let mut actions = Vec::new();
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        assert!(actions.is_empty());
        click_at(
            &ctx,
            &state,
            centre_of(&labels, "Report this"),
            &mut actions,
        );
        assert_eq!(actions, [Action::ReportInterfaceFont]);
    }

    #[test]
    fn the_interface_font_menu_emits_the_chosen_face() {
        let ctx = egui::Context::default();
        youtfast_appearance::theme::install_fonts(&ctx);
        let mut state = State::default();
        let mut actions = Vec::new();
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(
            &ctx,
            &state,
            centre_of(&labels, "System (default)"),
            &mut actions,
        );
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(&ctx, &state, centre_of(&labels, "Inter"), &mut actions);
        assert_eq!(actions, [Action::SetInterfaceFont(InterfaceFont::Inter)]);
        actions.clear();
        // And back, from Inter.
        state.settings.interface_font = InterfaceFont::Inter;
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(&ctx, &state, centre_of(&labels, "Inter"), &mut actions);
        let labels = draw_appearance(&ctx, &state, Vec::new(), &mut actions);
        click_at(
            &ctx,
            &state,
            centre_of(&labels, "System (default)"),
            &mut actions,
        );
        assert_eq!(actions, [Action::SetInterfaceFont(InterfaceFont::System)]);
    }

    #[test]
    fn playback_buffer_shows_tradeoffs_and_emits_action_on_change() {
        for (buffer, expected_selected, expected_tradeoff) in [
            (
                PlaybackBuffer::Standard,
                "Large — 150 MiB / 50 MiB (mpv default)",
                "Large (150 MiB ahead, 50 MiB back) buffers several minutes ahead for unstable connections and instant seeking, using more RAM (~300–650 MB) and bandwidth if you skip videos early. Small (50 MiB / 20 MiB) uses less RAM (~200 MB) and data, but holds only ~15–90s ahead.",
            ),
            (
                PlaybackBuffer::Compact,
                "Small — 50 MiB / 20 MiB (less RAM)",
                "Small (50 MiB ahead, 20 MiB back) keeps RAM usage low (~200 MB) and wastes less data when skipping videos, but holds only ~15–90s ahead and re-downloads when seeking far back. Large (150 MiB / 50 MiB) buffers several minutes ahead using more RAM (~300–650 MB).",
            ),
        ] {
            let ctx = egui::Context::default();
            theme::install_fonts(&ctx);
            let mut state = State::default();
            state.settings.playback_buffer = buffer;
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(900.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| playback(ui, &state, &mut actions),
            );
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.text()),
                    _ => None,
                })
                .collect();
            assert!(labels.contains(&"Video buffer"));
            assert!(labels.contains(&expected_selected));
            assert!(labels.contains(&expected_tradeoff));
            assert!(actions.is_empty());
            output.textures_delta.clear();
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
                    playback(ui, &state, &mut Vec::new())
                });
            }
            for (sync, label) in [
                (HistorySync::Separate, "Separate"),
                (HistorySync::Merged, "Merged in YouTFast"),
                (HistorySync::SendToYoutube, "Merged and sent to YouTube"),
            ] {
                state.settings.history_sync = sync;
                assert_clear(&ctx, &tr(locale, label), |ui| {
                    history(ui, &state, &mut Vec::new())
                });
            }
        }
    }

    #[test]
    fn about_section_shows_version_auto_update_changelog_and_major_manual_button() {
        use youtfast_desktop::updater::{CURRENT_VERSION, ReleaseInfo, UpdateKind, UpdateState};

        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut state = State {
            update: UpdateState::Installed {
                version: "0.3.0".into(),
                kind: UpdateKind::Minor,
                changelog: "Added\n• Automatic minor and patch updates".into(),
                auto: true,
            },
            ..Default::default()
        };
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(900.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| about(ui, &state, &mut actions),
        );
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        let expected_version = format!("YouTFast {CURRENT_VERSION}");
        assert!(labels.contains(&"About".to_owned()));
        assert!(labels.contains(&expected_version));
        assert!(labels.contains(&"Notify about new versions".to_owned()));
        assert!(labels.contains(&"Automatic updates (minor and patch)".to_owned()));
        assert!(labels.contains(&"What changed in v0.3.0 (Changelog)".to_owned()));
        assert!(
            labels
                .iter()
                .any(|l| l.contains("Automatic minor and patch updates"))
        );
        output.textures_delta.clear();

        // Major update requires manual installation button.
        state.update = UpdateState::Available(ReleaseInfo {
            version: "1.0.0".into(),
            kind: UpdateKind::Major,
            changelog: "Added\n• Major release".into(),
            html_url: "https://github.com/bcosta19/youtfast/releases/tag/v1.0.0".into(),
            asset_name: None,
            asset_url: None,
            checksums_url: None,
        });
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    vec2(900.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| about(ui, &state, &mut actions),
        );
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        assert!(
            labels.contains(
                &"Major version v1.0.0 is available. Major updates require manual installation."
                    .to_owned()
            )
        );
        assert!(labels.contains(&"Update to v1.0.0".to_owned()));
        assert!(labels.contains(&"What's new in v1.0.0".to_owned()));
        output.textures_delta.clear();
    }
}
