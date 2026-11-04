//! The prefilled GitHub issue behind "Report this" in Settings → Appearance,
//! for a system whose interface font could not be loaded.
//!
//! Nothing is sent: the app only opens the browser at the issue's form, where
//! the user reads the text and decides whether to submit. The text holds the
//! version, the operating system, the desktop and the face the platform names
//! as its interface font, and nothing else about the person or the machine.

use std::process::Command;

const NEW_ISSUE: &str = "https://github.com/bcosta19/youtfast/issues/new";
const TITLE: &str = "System interface font could not be loaded";
/// No field is longer than this, whatever a tool printed.
const FIELD_LIMIT: usize = 200;

/// What the issue says about the machine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub version: String,
    /// The operating system and its version, as the platform words them.
    pub os: String,
    /// `XDG_CURRENT_DESKTOP` on Linux.
    pub desktop: Option<String>,
    /// What `fc-match system-ui` prints, where fontconfig is installed.
    pub system_ui: Option<String>,
    /// The interface font the desktop is configured with (GNOME's
    /// `font-name`, GTK's or KDE's setting), on Linux.
    pub desktop_font: Option<String>,
}

impl Report {
    /// Asks the machine. Runs `fc-match` on Linux, so call it off the
    /// interface thread.
    pub fn gather() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            os: operating_system(),
            desktop: desktop(),
            system_ui: fc_match(),
            desktop_font: super::desktop_font::configured_family(),
        }
    }

    fn body(&self) -> String {
        let mut lines = vec![
            "YouTFast could not load the system's interface font, so it draws with Inter."
                .to_owned(),
            String::new(),
            format!("- YouTFast: {}", field(&self.version)),
            format!("- OS: {}", field(&self.os)),
        ];
        if let Some(desktop) = &self.desktop {
            lines.push(format!("- Desktop session: {}", field(desktop)));
        }
        if let Some(configured) = &self.desktop_font {
            lines.push(format!(
                "- Desktop's configured interface font: {}",
                field(configured)
            ));
        }
        if let Some(system_ui) = &self.system_ui {
            lines.push(format!("- `fc-match system-ui`: {}", field(system_ui)));
        }
        lines.push(String::new());
        lines.push("Which font does your desktop use for its interface?".to_owned());
        lines.push(String::new());
        lines.join("\n")
    }

    /// The new-issue form with the title and body filled in.
    pub fn issue_url(&self) -> String {
        let mut url = url::Url::parse(NEW_ISSUE).expect("the issue address is valid");
        url.query_pairs_mut()
            .append_pair("title", TITLE)
            .append_pair("body", &self.body());
        url.into()
    }
}

/// One line, cut to the limit.
fn field(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(FIELD_LIMIT)
        .collect()
}

fn desktop() -> Option<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .filter(|desktop| !desktop.trim().is_empty())
}

fn fc_match() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let output = Command::new("fc-match").arg("system-ui").output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (output.status.success() && !text.is_empty()).then(|| text.to_owned())
}

#[cfg(target_os = "linux")]
fn operating_system() -> String {
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let name = os_release_value(&release, "PRETTY_NAME").unwrap_or_else(|| "unknown".to_owned());
    format!("Linux, {name}")
}

#[cfg(target_os = "macos")]
fn operating_system() -> String {
    let version = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default();
    format!("macOS {}", version.trim())
}

#[cfg(target_os = "windows")]
fn operating_system() -> String {
    use std::os::windows::process::CommandExt as _;
    // The release build is a GUI app on Windows; without this flag a
    // console window would flash.
    let version = Command::new("cmd")
        .args(["/C", "ver"])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default();
    format!("Windows {}", version.trim())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn operating_system() -> String {
    std::env::consts::OS.to_owned()
}

/// A value of `/etc/os-release`, without its quotes.
#[cfg(any(target_os = "linux", test))]
fn os_release_value(release: &str, key: &str) -> Option<String> {
    release.lines().find_map(|line| {
        let value = line.strip_prefix(key)?.strip_prefix('=')?;
        Some(value.trim().trim_matches('"').trim_matches('\'').to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Report {
        Report {
            version: "0.4.0".into(),
            os: "Linux, Arch Linux".into(),
            desktop: Some("Hyprland".into()),
            system_ui: Some("Liberation Sans: \"Regular\" & more".into()),
            desktop_font: Some("Adwaita Sans".into()),
        }
    }

    fn query(url: &str) -> std::collections::HashMap<String, String> {
        url::Url::parse(url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    }

    #[test]
    fn the_url_opens_the_new_issue_form_of_this_project() {
        let url = report().issue_url();
        assert!(url.starts_with("https://github.com/bcosta19/youtfast/issues/new?"));
        let pairs = query(&url);
        assert_eq!(pairs["title"], TITLE);
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn the_text_is_encoded_and_survives_the_round_trip() {
        let url = report().issue_url();
        // No raw space, newline, quote or ampersand from the values.
        let query_part = url.split_once('?').unwrap().1;
        assert!(!query_part.contains([' ', '\n', '"']));
        assert_eq!(query_part.matches('&').count(), 1);
        let body = &query(&url)["body"];
        assert!(body.contains("- YouTFast: 0.4.0\n"));
        assert!(body.contains("- OS: Linux, Arch Linux\n"));
        assert!(body.contains("- Desktop session: Hyprland\n"));
        assert!(body.contains("- Desktop's configured interface font: Adwaita Sans\n"));
        assert!(body.contains("- `fc-match system-ui`: Liberation Sans: \"Regular\" & more\n"));
        assert!(body.contains("Which font does your desktop use"));
    }

    #[test]
    fn missing_details_are_left_out_and_long_ones_cut() {
        let body = Report {
            desktop: None,
            system_ui: None,
            desktop_font: None,
            os: "x".repeat(5000),
            ..report()
        }
        .body();
        assert!(!body.contains("Desktop session"));
        assert!(!body.contains("fc-match"));
        assert!(!body.contains("configured"));
        assert!(body.len() < 600);
    }

    #[test]
    fn a_value_cannot_add_lines_to_the_text() {
        let body = Report {
            os: "Linux\n- Home: /home/someone".into(),
            ..report()
        }
        .body();
        assert!(body.contains("- OS: Linux - Home: /home/someone\n"));
    }

    #[test]
    fn the_text_names_no_one() {
        let body = report().body();
        for private in ["HOME", "USER", "@", "/home/", "hostname"] {
            assert!(!body.contains(private), "{private}");
        }
    }

    #[test]
    fn os_release_values_lose_their_quotes() {
        let release = "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\n";
        assert_eq!(
            os_release_value(release, "PRETTY_NAME").as_deref(),
            Some("Arch Linux")
        );
        assert_eq!(os_release_value(release, "VERSION"), None);
    }
}
