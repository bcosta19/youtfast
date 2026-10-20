//! Explicit interface languages. English is always the initial preference,
//! independent of the desktop locale. YouTube content and captions keep their
//! own languages. Catalogs are bundled at build time, with English fallback.
use std::borrow::Cow;

use serde::{Deserialize, Serialize};

pub use fastframe_i18n::{gettext, ngettext};

include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[serde(rename = "pt-BR")]
    PortugueseBrazil,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "it")]
    Italian,
    #[default]
    #[serde(rename = "en", other)]
    English,
}

impl Locale {
    pub const ALL: &'static [Self] = &[
        Self::English,
        Self::PortugueseBrazil,
        Self::Spanish,
        Self::French,
        Self::German,
        Self::Italian,
    ];

    /// Native names remain recognizable even when another language is selected.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::PortugueseBrazil => "Português (Brasil)",
            Self::Spanish => "Español",
            Self::French => "Français",
            Self::German => "Deutsch",
            Self::Italian => "Italiano",
        }
    }
}

impl fastframe_i18n::Locale for Locale {
    fn catalog(self) -> Option<&'static dyn fastframe_i18n::Translator> {
        match self {
            Self::English => None,
            Self::PortugueseBrazil => Some(&pt_br::Translator),
            Self::Spanish => Some(&es::Translator),
            Self::French => Some(&fr::Translator),
            Self::German => Some(&de::Translator),
            Self::Italian => Some(&it::Translator),
        }
    }
}

/// Named placeholders are expanded once, so user text containing another
/// placeholder is never interpreted as part of the translation template.
pub fn format(locale: Locale, source: &'static str, args: &[(&str, String)]) -> String {
    interpolate(&gettext(locale, source), args)
}

pub fn interpolate(template: &str, args: &[(&str, String)]) -> String {
    let mut remaining = template;
    let mut result = String::with_capacity(template.len());
    while let Some(start) = remaining.find('{') {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start..];
        let Some(end) = remaining.find('}') else {
            break;
        };
        let key = &remaining[1..end];
        match args.iter().find(|(name, _)| *name == key) {
            Some((_, value)) => result.push_str(value),
            None => result.push_str(&remaining[..=end]),
        }
        remaining = &remaining[end + 1..];
    }
    result.push_str(remaining);
    result
}

/// Only for app-generated status/error text. Never run content from videos,
/// channels, comments, or user-entered names through a translation catalog.
/// These messages have already crossed their existing privacy boundaries.
pub fn message(locale: Locale, text: &str) -> Cow<'_, str> {
    use fastframe_i18n::Locale as _;
    let Some(catalog) = locale.catalog() else {
        return Cow::Borrowed(text);
    };
    if let Some(error) = text.strip_suffix(" Sign in again to see your account.") {
        return Cow::Owned(format(
            locale,
            "{error} Sign in again to see your account.",
            &[("error", catalog.translate(error, None).into_owned())],
        ));
    }
    // Prefixes belong to the application. The suffix is translated only by
    // exact catalog lookup, retaining controlled error details verbatim.
    for (prefix, template) in [
        (
            "Could not check for uploads: ",
            "Could not check for uploads: {error}",
        ),
        (
            "Could not rate the video: ",
            "Could not rate the video: {error}",
        ),
        (
            "Could not change the playlist: ",
            "Could not change the playlist: {error}",
        ),
        (
            "Could not save the library: ",
            "Could not save the library: {error}",
        ),
        (
            "Could not save the settings: ",
            "Could not save the settings: {error}",
        ),
        (
            "The account was removed, but its session could not be deleted from the keyring: ",
            "The account was removed, but its session could not be deleted from the keyring: {error}",
        ),
    ] {
        if let Some(error) = text.strip_prefix(prefix) {
            return Cow::Owned(format(
                locale,
                template,
                &[("error", catalog.translate(error, None).into_owned())],
            ));
        }
    }
    catalog.translate(text, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_language_defaults_and_unknown_preferences_fall_back_to_english() {
        assert_eq!(Locale::default(), Locale::English);
        assert_eq!(
            serde_json::from_str::<Locale>("\"future-language\"").unwrap(),
            Locale::English
        );
        assert_eq!(gettext(Locale::English, "Settings"), "Settings");
        assert_eq!(
            gettext(Locale::PortugueseBrazil, "Settings"),
            "Configurações"
        );
        assert_eq!(gettext(Locale::Spanish, "Settings"), "Configuración");
        assert_eq!(gettext(Locale::French, "Settings"), "Paramètres");
        assert_eq!(gettext(Locale::German, "Settings"), "Einstellungen");
        assert_eq!(gettext(Locale::Italian, "Settings"), "Impostazioni");
        assert_eq!(
            gettext(Locale::PortugueseBrazil, "A future message"),
            "A future message"
        );
    }

    #[test]
    fn formatting_never_interprets_interpolated_user_content() {
        assert_eq!(
            interpolate(
                "{name}: {count}",
                &[("name", "{count}".into()), ("count", "2".into())]
            ),
            "{count}: 2"
        );
        assert_eq!(
            interpolate("{unknown} {unfinished", &[]),
            "{unknown} {unfinished"
        );
    }

    #[test]
    fn controlled_errors_translate_including_the_expired_account_suffix() {
        let english =
            "YouTube needs a valid session. Sign in again. Sign in again to see your account.";
        assert_eq!(message(Locale::English, english), english);
        assert_eq!(
            message(Locale::PortugueseBrazil, english),
            "O YouTube precisa de uma sessão válida. Entre novamente. Entre novamente para ver sua conta."
        );
        assert_eq!(
            message(
                Locale::PortugueseBrazil,
                "Could not rate the video: The request timed out. Retry."
            ),
            "Não foi possível avaliar o vídeo: O tempo da solicitação esgotou. Tente novamente."
        );
        assert_eq!(
            message(Locale::English, "The received transcript is invalid."),
            "The received transcript is invalid."
        );
        assert_eq!(
            message(
                Locale::PortugueseBrazil,
                "The received transcript is invalid."
            ),
            "A transcrição recebida é inválida."
        );
    }

    #[test]
    fn catalog_entries_preserve_named_placeholders() {
        fn placeholders(text: &str) -> Vec<&str> {
            let mut keys: Vec<_> = text
                .split('{')
                .skip(1)
                .filter_map(|s| s.split_once('}').map(|(key, _)| key))
                .collect();
            keys.sort_unstable();
            keys
        }
        for catalog in [
            include_str!("../assets/i18n/pt-BR.po"),
            include_str!("../assets/i18n/es.po"),
            include_str!("../assets/i18n/fr.po"),
            include_str!("../assets/i18n/de.po"),
            include_str!("../assets/i18n/it.po"),
        ] {
            let mut source = String::new();
            for line in catalog.lines() {
                if let Some(value) = line
                    .strip_prefix("msgid ")
                    .or_else(|| line.strip_prefix("msgid_plural "))
                {
                    source = serde_json::from_str(value).unwrap();
                } else if line.starts_with("msgstr") && !source.is_empty() {
                    let (_, value) = line.split_once(' ').unwrap();
                    let translation: String = serde_json::from_str(value).unwrap();
                    assert!(!translation.is_empty(), "{source}");
                    assert_eq!(
                        placeholders(&source),
                        placeholders(&translation),
                        "{source}"
                    );
                }
            }
        }
    }
}
