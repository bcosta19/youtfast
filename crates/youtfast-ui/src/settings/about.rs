//! The About settings group: version status, updates, and changelog notes.

use egui::{CornerRadius, RichText, Ui};
use youtfast_i18n::{format as trf, gettext as tr};

use super::{CONTROL_WIDTH, setting_row, switch_row};
use crate::widgets;
use youtfast_app::{Action, State};
use youtfast_appearance::icons::Icon;
use youtfast_appearance::theme;
use youtfast_desktop::updater::{self, InstallMethod, UpdateState};

/// The About group: installed version, update notifications, automatic
/// minor/patch updates, manual major update button, and changelog notes.
pub(super) fn about(ui: &mut Ui, state: &State, actions: &mut Vec<Action>) {
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
                UpdateState::Available(release) if !release.can_self_update() => {
                    if release.method == InstallMethod::Flatpak {
                        trf(
                            locale,
                            "Version v{version} is available. Flatpak updates cannot run inside the sandbox; update with the command below or from GitHub Releases.",
                            &[("version", release.version.clone())],
                        )
                    } else {
                        trf(
                            locale,
                            "Version v{version} is available. In-app updates are not available for this installation method; update from GitHub Releases.",
                            &[("version", release.version.clone())],
                        )
                    }
                }
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
                    pending_apply_on_exit: true,
                    ..
                } => trf(
                    locale,
                    "Update to v{version} is ready. Restart YouTFast to finish installing.",
                    &[("version", version.clone())],
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
                        UpdateState::Available(release) if release.can_self_update() => {
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
                        UpdateState::Available(release) => {
                            if let Some(cmd) = release.method.manual_update_command() {
                                if widgets::button(
                                    ui,
                                    &raised,
                                    &tr(locale, "Copy update command"),
                                    widgets::ButtonStyle::Primary,
                                )
                                .clicked()
                                {
                                    actions.push(Action::CopyUpdateCommand(cmd.to_owned()));
                                }
                            } else if widgets::icon_text_button(
                                ui,
                                &raised,
                                Some(Icon::ExternalLink),
                                &tr(locale, "Open release page"),
                                widgets::ButtonStyle::Primary,
                            )
                            .clicked()
                            {
                                actions.push(Action::OpenUrl(release.html_url.clone()));
                            }
                        }
                        UpdateState::Installed { .. } => {
                            if widgets::button(
                                ui,
                                &raised,
                                &tr(locale, "Restart now"),
                                widgets::ButtonStyle::Primary,
                            )
                            .clicked()
                            {
                                actions.push(Action::RestartToUpdate);
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    #[test]
    fn about_section_shows_version_auto_update_changelog_and_major_manual_button() {
        use youtfast_desktop::updater::{
            CURRENT_VERSION, InstallMethod, ReleaseInfo, UpdateKind, UpdateState,
        };

        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut state = State {
            update: UpdateState::Installed {
                version: "0.3.0".into(),
                kind: UpdateKind::Minor,
                changelog: "Added\n• Automatic minor and patch updates".into(),
                auto: true,
                pending_apply_on_exit: false,
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
        assert!(labels.contains(&"Restart now".to_owned()));
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
            method: InstallMethod::LinuxNative,
            install_supported: true,
            changelog: "Added\n• Major release".into(),
            html_url: "https://github.com/bcosta19/youtfast/releases/tag/v1.0.0".into(),
            asset_name: Some("youtfast-v1.0.0-x86_64-unknown-linux-gnu.tar.gz".into()),
            asset_url: Some("https://github.com/bcosta19/youtfast/releases/download/v1.0.0/youtfast-v1.0.0-x86_64-unknown-linux-gnu.tar.gz".into()),
            checksums_url: Some("https://github.com/bcosta19/youtfast/releases/download/v1.0.0/checksums.txt".into()),
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

        // Flatpak keeps sandbox boundaries and exposes the copy-command action.
        state.update = UpdateState::Available(ReleaseInfo {
            version: "1.0.0".into(),
            kind: UpdateKind::Major,
            method: InstallMethod::Flatpak,
            install_supported: false,
            changelog: String::new(),
            html_url: "https://github.com/bcosta19/youtfast/releases/tag/v1.0.0".into(),
            asset_name: Some("youtfast-v1.0.0-x86_64.flatpak".into()),
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
        assert!(labels.contains(&"Copy update command".to_owned()));
        output.textures_delta.clear();
    }
}
