//! What the tray, its dialogs and the notifications say, in the language in force
//! (ADR-041).
//!
//! Every phrase is a [`Msg`], and each language answers it with an exhaustive `match`
//! (no wildcard arm), so a phrase without a translation does not compile. The panel has
//! its own catalogs (`ui/src/i18n`); the CLI, its `--json` and the socket stay in Spanish.

use devports_proto::LanguagePreference;
use tauri::{AppHandle, Manager};

use crate::preferences::PreferencesStore;

/// The languages DevPorts speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    /// Spanish, the source language.
    Es,
    /// English.
    En,
}

/// The system's preferred languages, most preferred first (`es-CL`, `en-US`, …).
pub fn system_languages() -> Vec<String> {
    sys_locale::get_locales().collect()
}

/// The language in force right now: the saved preference, and for `system` the languages the
/// machine prefers at this moment.
pub fn current(app: &AppHandle) -> Locale {
    resolve(
        app.state::<PreferencesStore>().get().language,
        &system_languages(),
    )
}

/// `system` is the first of the system's languages DevPorts has a catalog for (`es-CL` is
/// Spanish, `en-GB` is English); none of them means English. The panel applies the same
/// rule (`ui/src/i18n/locale.ts`) and both are tested with the same table.
pub fn resolve<S: AsRef<str>>(preference: LanguagePreference, system: &[S]) -> Locale {
    match preference {
        LanguagePreference::Es => Locale::Es,
        LanguagePreference::En => Locale::En,
        LanguagePreference::System => system
            .iter()
            .find_map(|tag| {
                let language = tag
                    .as_ref()
                    .split(['-', '_'])
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                match language.as_str() {
                    "es" => Some(Locale::Es),
                    "en" => Some(Locale::En),
                    _ => None,
                }
            })
            .unwrap_or(Locale::En),
    }
}

/// Everything the shell says to the person outside the panel.
#[derive(Debug, Clone, Copy)]
pub enum Msg<'a> {
    /// Tray menu: open the panel.
    OpenPanel,
    /// Tray menu: link the `devports` command.
    InstallCommand,
    /// Tray menu: start with the session.
    Autostart,
    /// Tray menu: quit.
    Quit,
    /// The start-with-session switch could not be changed.
    AutostartFailed { error: &'a str },
    /// The command was linked at `link`; `path_hint` is empty or a ready-made paragraph.
    InstallDone { link: &'a str, path_hint: &'a str },
    /// The folder of the link is not on the PATH.
    PathHint { folder: &'a str },
    /// The command could not be linked.
    InstallFailed { error: &'a str },
    /// Notification action: extend a lease by 30 minutes.
    ExtendAction,
    /// Title: a temporary instance expires in one minute.
    LeaseExpiring { name: &'a str },
    /// Name used when the instance has no service.
    UnnamedTest,
    /// Detail of the lease notification: reason and project, either may be missing.
    LeaseDetail {
        reason: Option<&'a str>,
        project: Option<&'a str>,
    },
    /// Title: a duplicate of `service` appeared.
    DuplicateTitle { service: &'a str },
    /// Body: another server appeared, on `port` if known, in `project`.
    DuplicateBody { port: Option<u16>, project: &'a str },
}

/// `msg`, said in `locale`.
pub fn text(locale: Locale, msg: Msg<'_>) -> String {
    match locale {
        Locale::Es => spanish(msg),
        Locale::En => english(msg),
    }
}

fn spanish(msg: Msg<'_>) -> String {
    match msg {
        Msg::OpenPanel => "Abrir panel".to_owned(),
        Msg::InstallCommand => "Instalar el comando devports…".to_owned(),
        Msg::Autostart => "Abrir al iniciar sesión".to_owned(),
        Msg::Quit => "Salir".to_owned(),
        Msg::AutostartFailed { error } => {
            format!("No se pudo cambiar el inicio automático: {error}")
        }
        Msg::InstallDone { link, path_hint } => {
            format!("Listo: {link}\n\nAhora puedes usar `devports ls`.{path_hint}")
        }
        Msg::PathHint { folder } => format!(
            "\n\nAgrega {folder} a tu PATH (por ejemplo en ~/.zshrc: export PATH=\"{folder}:$PATH\")."
        ),
        Msg::InstallFailed { error } => format!("No se pudo instalar el comando: {error}"),
        Msg::ExtendAction => "Extender 30 min".to_owned(),
        Msg::LeaseExpiring { name } => format!("{name} vence en 1 minuto"),
        Msg::UnnamedTest => "una prueba".to_owned(),
        Msg::LeaseDetail { reason, project } => match (reason, project) {
            (Some(reason), Some(project)) => format!("«{reason}» del proyecto {project}"),
            (None, Some(project)) => format!("Proyecto {project}"),
            (Some(reason), None) => format!("«{reason}»"),
            (None, None) => String::new(),
        },
        Msg::DuplicateTitle { service } => format!("Duplicado de {service}"),
        Msg::DuplicateBody { port, project } => format!(
            "Apareció otro servidor{} sin pasar por DevPorts (proyecto {project})",
            place(port)
        ),
    }
}

fn english(msg: Msg<'_>) -> String {
    match msg {
        Msg::OpenPanel => "Open panel".to_owned(),
        Msg::InstallCommand => "Install the devports command…".to_owned(),
        Msg::Autostart => "Open at login".to_owned(),
        Msg::Quit => "Quit".to_owned(),
        Msg::AutostartFailed { error } => format!("Could not change opening at login: {error}"),
        Msg::InstallDone { link, path_hint } => {
            format!("Done: {link}\n\nYou can now use `devports ls`.{path_hint}")
        }
        Msg::PathHint { folder } => format!(
            "\n\nAdd {folder} to your PATH (for example in ~/.zshrc: export PATH=\"{folder}:$PATH\")."
        ),
        Msg::InstallFailed { error } => format!("Could not install the command: {error}"),
        Msg::ExtendAction => "Extend 30 min".to_owned(),
        Msg::LeaseExpiring { name } => format!("{name} expires in 1 minute"),
        Msg::UnnamedTest => "a test".to_owned(),
        Msg::LeaseDetail { reason, project } => match (reason, project) {
            (Some(reason), Some(project)) => format!("“{reason}” in project {project}"),
            (None, Some(project)) => format!("Project {project}"),
            (Some(reason), None) => format!("“{reason}”"),
            (None, None) => String::new(),
        },
        Msg::DuplicateTitle { service } => format!("Duplicate of {service}"),
        Msg::DuplicateBody { port, project } => format!(
            "Another server{} appeared without going through DevPorts (project {project})",
            place(port)
        ),
    }
}

/// ` :8080`, or nothing when the port is unknown.
fn place(port: Option<u16>) -> String {
    port.map_or_else(String::new, |port| format!(" :{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // covers: UXR-9.5
    #[test]
    fn the_system_languages_pick_a_language_the_way_the_panel_does() {
        // The same table as ui/src/i18n/locale.test.ts.
        let table: [(&[&str], Locale); 8] = [
            (&["es-CL"], Locale::Es),
            (&["es_MX"], Locale::Es),
            (&["en-GB"], Locale::En),
            (&["EN-us"], Locale::En),
            (&["fr-FR", "es-ES"], Locale::Es),
            (&["fr-FR", "en-GB", "es"], Locale::En),
            (&["fr-FR", "de"], Locale::En),
            (&[], Locale::En),
        ];
        for (tags, expected) in table {
            assert_eq!(
                resolve(LanguagePreference::System, tags),
                expected,
                "{tags:?}"
            );
        }
    }

    // covers: UXR-9.5
    #[test]
    fn a_chosen_language_ignores_the_system() {
        assert_eq!(resolve(LanguagePreference::Es, &["en-US"]), Locale::Es);
        assert_eq!(resolve(LanguagePreference::En, &["es-CL"]), Locale::En);
    }

    // covers: UXR-9.5
    #[test]
    fn phrases_come_out_in_each_language_with_their_details() {
        let lease = Msg::LeaseDetail {
            reason: Some("QA login"),
            project: Some("shop"),
        };
        assert_eq!(text(Locale::Es, lease), "«QA login» del proyecto shop");
        assert_eq!(text(Locale::En, lease), "“QA login” in project shop");
        let none = Msg::LeaseDetail {
            reason: None,
            project: None,
        };
        assert_eq!(text(Locale::Es, none), "");
        assert_eq!(text(Locale::En, none), "");
        let duplicate = Msg::DuplicateBody {
            port: Some(8082),
            project: "shop",
        };
        assert_eq!(
            text(Locale::En, duplicate),
            "Another server :8082 appeared without going through DevPorts (project shop)"
        );
        assert_eq!(
            text(
                Locale::Es,
                Msg::DuplicateBody {
                    port: None,
                    project: "shop"
                }
            ),
            "Apareció otro servidor sin pasar por DevPorts (proyecto shop)"
        );
        assert_eq!(text(Locale::Es, Msg::ExtendAction), "Extender 30 min");
        assert_eq!(text(Locale::En, Msg::ExtendAction), "Extend 30 min");
    }
}
