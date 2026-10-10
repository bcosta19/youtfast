//! Themes: the palettes the settings page offers, and the one drawn.

use super::*;

impl App {
    /// Picks a theme and draws it.
    pub(super) fn set_theme(&mut self, choice: ThemeChoice) {
        // The listed palette is drawn at once, not after the rescan.
        if let ThemeChoice::Custom(filename) = &choice
            && let Some(theme) = self.custom_themes.find(filename)
        {
            self.state.settings.custom_theme_cache = Some(theme.clone());
        }
        self.state.settings.theme = choice;
        self.save_settings();
        // A custom choice scans again so its cache fills at once.
        if matches!(self.state.settings.theme, ThemeChoice::Custom(_)) {
            self.load_themes();
        } else {
            self.refresh_theme_list();
        }
    }

    /// Lists the themes folder again, reading the selected palette first.
    pub(super) fn load_themes(&mut self) {
        self.custom_themes.start(
            self.dirs.config.join("themes"),
            selected_theme(&self.state.settings),
            &self.theme_waker,
        );
        self.refresh_theme_list();
    }

    /// Takes the selected and the desktop's palettes from a finished scan.
    pub(super) fn adopt_themes(&mut self) {
        let mut changed = false;
        if let ThemeChoice::Custom(filename) = &self.state.settings.theme
            && let Some(theme) = self.custom_themes.find(filename)
            && self.state.settings.custom_theme_cache.as_ref() != Some(theme)
        {
            self.state.settings.custom_theme_cache = Some(theme.clone());
            changed = true;
        }
        if self.custom_themes.follows_omarchy() {
            if let Some(theme) = self.custom_themes.system_theme()
                && self.state.settings.system_theme_cache.as_ref() != Some(theme)
            {
                self.state.settings.system_theme_cache = Some(theme.clone());
                changed = true;
            }
        } else if self.state.settings.system_theme_cache.take().is_some() {
            changed = true;
        }
        if changed {
            self.save_settings();
        }
        self.refresh_theme_list();
    }

    /// Applies the settings' palette when it differs from the one drawn,
    /// revealing the change from the middle of the window outwards.
    pub(super) fn update_theme(&mut self, ctx: &egui::Context) {
        let palette = self.state.settings.palette();
        if palette == self.state.palette {
            return;
        }
        self.theme_transition.begin(ctx);
        if self.theme_transition.holding(ctx) {
            return;
        }
        theme::apply(ctx, &palette);
        self.state.palette = palette;
    }

    /// What the settings page lists and says under the theme setting.
    pub(super) fn refresh_theme_list(&mut self) {
        let state = &mut self.state;
        state.themes = self
            .custom_themes
            .picker_themes()
            .map(|theme| {
                (
                    theme.filename.clone(),
                    fastframe_theme::display_name(&theme.filename).to_owned(),
                )
            })
            .collect();
        state.follows_omarchy = self.custom_themes.follows_omarchy();
        let selected = match &state.settings.theme {
            ThemeChoice::Custom(filename) => Some(filename.as_str()),
            _ => None,
        };
        state.theme_status = theme::catalog_status(&self.custom_themes, selected).into_owned();
    }
}

/// The palette filename a `Custom` theme choice selects, for a scan.
pub(super) fn selected_theme(settings: &Settings) -> Option<String> {
    match &settings.theme {
        ThemeChoice::Custom(filename) => Some(filename.clone()),
        _ => None,
    }
}
