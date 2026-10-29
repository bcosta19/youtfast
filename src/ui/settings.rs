//! The settings page, the siblings' way: the page's title, then each group
//! as a heading over a card of rows — the option's name and what it does
//! at the left, its control at the right.

use egui::{Align, Color32, CornerRadius, Layout, Response, RichText, Sense, Ui, pos2, vec2};

use super::{results, widgets};
use crate::app::State;
use crate::appearance::i18n::{Locale, format as trf, gettext as tr};
use crate::appearance::icons::Icon;
use crate::appearance::theme::{self, Palette};
use crate::model::Action;
use crate::storage::settings::{
    HistorySync, PlaybackBuffer, SidebarButton, ThemeChoice, default_sidebar, sanitize_sidebar,
};

/// The widest the settings read well.
const WIDTH: f32 = 760.0;
/// The room a row keeps at its right for a usual control.
const CONTROL_WIDTH: f32 = 260.0;
/// How often the checks may run, in minutes.
const INTERVALS: [u32; 4] = [5, 15, 30, 60];

pub fn show(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
    let locale = state.settings.language;
    results::panel(ui, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                // set_max_width also widens: a narrow window would cut
                // the text off at its edge instead of wrapping it.
                ui.set_max_width(WIDTH.min(ui.available_width()));
                results::title(ui, state, &tr(locale, "Settings"), None);
                appearance(ui, state, actions);
                ui.add_space(24.0);
                playback(ui, state, actions);
                ui.add_space(24.0);
                sidebar(ui, state, actions);
                ui.add_space(24.0);
                history(ui, state, actions);
                ui.add_space(24.0);
                notifications(ui, state, actions);
                ui.add_space(24.0);
                about(ui, state, actions);
                ui.add_space(24.0);
            });
    });
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
                    RichText::new(crate::appearance::i18n::message(
                        locale,
                        &state.theme_status,
                    ))
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
            setting_row(
                ui,
                palette,
                &tr(locale, "Autoplay"),
                &tr(
                    locale,
                    "At the end of a video, the next one starts after a countdown: first the queue, then the source list, then related videos.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    if switch(ui, palette, autoplay, &tr(locale, "Autoplay")).clicked() {
                        actions.push(Action::SetAutoplay(!autoplay));
                    }
                },
            );
            ui.add_space(12.0);
            let scroll_miniplayer = state.settings.scroll_miniplayer;
            setting_row(
                ui,
                palette,
                &tr(locale, "Miniplayer while scrolling"),
                &tr(
                    locale,
                    "Keep the video visible when scrolling down to the comments.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    if switch(
                        ui,
                        palette,
                        scroll_miniplayer,
                        &tr(locale, "Miniplayer while scrolling"),
                    )
                    .clicked()
                    {
                        actions.push(Action::SetScrollMiniplayer(!scroll_miniplayer));
                    }
                },
            );
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
            setting_row(
                ui,
                palette,
                &tr(locale, "Skip sponsors (SponsorBlock)"),
                &tr(
                    locale,
                    "Skips sponsors and offers to skip intros, credits and other segments marked by the SponsorBlock community. Requests to sponsor.ajay.app use a hash prefix matching thousands of videos.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    if switch(ui, palette, sponsorblock, "SponsorBlock").clicked() {
                        actions.push(Action::SetSponsorBlock(!sponsorblock));
                    }
                },
            );
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
            let entries = sanitize_sidebar(state.settings.sidebar.clone());
            let last = entries.len() - 1;
            for (index, entry) in entries.iter().enumerate() {
                let name = sidebar_button_name(locale, entry.button);
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
                        for (icon, to, key, enabled) in [
                            (
                                Icon::ChevronDown,
                                index + 1,
                                "Move {name} down",
                                index < last,
                            ),
                            (
                                Icon::ChevronUp,
                                index.wrapping_sub(1),
                                "Move {name} up",
                                index > 0,
                            ),
                        ] {
                            let hint = trf(locale, key, &[("name", name.clone())]);
                            let moved = ui
                                .add_enabled_ui(enabled, |ui| {
                                    widgets::icon_button(ui, icon, 18.0, &hint)
                                })
                                .inner;
                            if moved.clicked() {
                                let mut next = entries.clone();
                                next.swap(index, to);
                                actions.push(Action::SetSidebar(next));
                            }
                        }
                    });
                });
            }
            ui.add_space(8.0);
            ui.add_enabled_ui(entries != default_sidebar(), |ui| {
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
            setting_row(
                ui,
                palette,
                &tr(locale, "Check for new uploads"),
                &tr(
                    locale,
                    "Checks the same sources as Home. YouTube's notifications have no API, so the app watches subscribed channels instead.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    if switch(ui, palette, on, &tr(locale, "Check for new uploads")).clicked() {
                        actions.push(Action::SetAlerts(!on));
                    }
                },
            );
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
    use crate::appearance::icons::Icon;
    use crate::desktop::updater::{self, UpdateState};

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
                UpdateState::Failed(error) => {
                    crate::appearance::i18n::message(locale, error).into_owned()
                }
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
            setting_row(
                ui,
                palette,
                &tr(locale, "Notify about new versions"),
                &tr(
                    locale,
                    "Show a system notification when a new version is available and ask if you want to update.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    if switch(
                        ui,
                        palette,
                        notify_on,
                        &tr(locale, "Notify about new versions"),
                    )
                    .clicked()
                    {
                        actions.push(Action::SetUpdateNotifications(!notify_on));
                    }
                },
            );
            ui.add_space(12.0);
            let auto_update = state.settings.auto_update;
            setting_row(
                ui,
                palette,
                &tr(locale, "Automatic updates (minor and patch)"),
                &tr(
                    locale,
                    "Automatically download and install minor (0.x.0) and patch (0.0.x) updates and display the changelog of what changed. Major versions (x.0.0) always require clicking Update manually.",
                ),
                CONTROL_WIDTH,
                |ui| {
                    if switch(
                        ui,
                        palette,
                        auto_update,
                        &tr(locale, "Automatic updates (minor and patch)"),
                    )
                    .clicked()
                    {
                        actions.push(Action::SetAutoUpdate(!auto_update));
                    }
                },
            );

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
/// the side that is on. Dimmed while disabled. The caller reads the click.
fn switch(ui: &mut Ui, palette: &Palette, on: bool, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, on, label));
    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(response.id, on);
        let off = if response.hovered() && enabled {
            palette.outline
        } else {
            palette.surface_active
        };
        let fill = Color32::from(egui::lerp(
            egui::Rgba::from(off)..=egui::Rgba::from(palette.accent),
            t,
        ));
        let (fill, knob) = if enabled {
            (fill, Color32::WHITE)
        } else {
            (fill.gamma_multiply(0.5), Color32::WHITE.gamma_multiply(0.6))
        };
        ui.painter().rect_filled(rect, CornerRadius::same(11), fill);
        widgets::focus_outline(ui, &response, CornerRadius::same(11));
        let x = egui::lerp(rect.left() + 11.0..=rect.right() - 11.0, t);
        ui.painter()
            .circle_filled(pos2(x, rect.center().y), 8.0, knob);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

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
        crate::appearance::theme::install_fonts(&ctx);
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
        use crate::desktop::updater::{CURRENT_VERSION, ReleaseInfo, UpdateKind, UpdateState};

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
