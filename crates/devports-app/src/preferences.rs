//! The shell's copy of the saved preferences.
//!
//! The core owns the stored value (SQLite). The shell keeps a copy so a panel can be
//! opened in the right theme and language without waiting for a round trip: the copy is
//! loaded when the core starts and replaced only after the core has saved a change.

use std::sync::{Mutex, PoisonError};

use devports_proto::Preferences;

/// Thread-safe owner of the copy.
#[derive(Debug, Default)]
pub struct PreferencesStore(Mutex<Preferences>);

impl PreferencesStore {
    /// The preferences in force.
    pub fn get(&self) -> Preferences {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Replaces the copy with what the core has stored.
    pub fn replace(&self, preferences: Preferences) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = preferences;
    }
}

#[cfg(test)]
mod tests {
    use devports_proto::{LanguagePreference, ThemePreference};

    use super::*;

    // covers: UXR-9.1
    #[test]
    fn the_copy_starts_as_system_and_follows_what_is_replaced() {
        let store = PreferencesStore::default();
        assert_eq!(store.get(), Preferences::default());
        let chosen = Preferences {
            theme: ThemePreference::Light,
            language: LanguagePreference::Es,
        };
        store.replace(chosen);
        assert_eq!(store.get(), chosen);
    }
}
