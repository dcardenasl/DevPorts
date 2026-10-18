//! The person's appearance and language preferences (ADR-040, ADR-041).
//!
//! They are edited from the panel only: the socket never carries them.

use serde::{Deserialize, Serialize};

/// Which theme the panel wears.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    /// Follow the system appearance.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

impl ThemePreference {
    /// The value stored and sent in the panel URL.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// The preference stored as `value`, or `None` if this build does not know it.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

/// Which language the panel, the tray and the notifications use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum LanguagePreference {
    /// Follow the system's preferred languages.
    #[default]
    System,
    /// Spanish.
    Es,
    /// English.
    En,
}

impl LanguagePreference {
    /// The value stored and sent in the panel URL.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Es => "es",
            Self::En => "en",
        }
    }

    /// The preference stored as `value`, or `None` if this build does not know it.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "es" => Some(Self::Es),
            "en" => Some(Self::En),
            _ => None,
        }
    }
}

/// Every preference, with `system` for anything never chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    /// The theme.
    pub theme: ThemePreference,
    /// The language.
    pub language: LanguagePreference,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_value_round_trips_through_its_stored_text() {
        for theme in [
            ThemePreference::System,
            ThemePreference::Light,
            ThemePreference::Dark,
        ] {
            assert_eq!(ThemePreference::parse(theme.as_str()), Some(theme));
        }
        for language in [
            LanguagePreference::System,
            LanguagePreference::Es,
            LanguagePreference::En,
        ] {
            assert_eq!(LanguagePreference::parse(language.as_str()), Some(language));
        }
    }

    #[test]
    fn stored_text_matches_the_wire_text() {
        let json = serde_json::to_string(&Preferences {
            theme: ThemePreference::Dark,
            language: LanguagePreference::Es,
        })
        .unwrap();
        assert_eq!(json, r#"{"theme":"dark","language":"es"}"#);
    }

    #[test]
    fn unknown_text_is_not_a_preference() {
        assert_eq!(ThemePreference::parse("sepia"), None);
        assert_eq!(LanguagePreference::parse("fr"), None);
        assert!(
            serde_json::from_str::<Preferences>(r#"{"theme":"sepia","language":"es"}"#).is_err()
        );
    }
}
