//! `ui/src/bindings.ts` is generated from the Rust types and never edited by hand
//! (ADR-025, point 9): this test writes what it should contain and fails when the
//! committed file differs, so a type cannot change on one side only.
//!
//! Regenerate with `UPDATE_BINDINGS=1 cargo test -p devports-core --features bindings --test bindings`.

#![allow(clippy::expect_used)]

use devports_core::runtime::Snapshot;
use devports_proto::views::{InstanceView, LogEntry, LogFacts};
use specta::Types;
use specta_typescript::Typescript;

const HEADER: &str = "// Generated from the Rust types by `crates/devports-core/tests/bindings.rs`.\n// Do not edit: run `UPDATE_BINDINGS=1 cargo test -p devports-core --features bindings --test bindings`.\n\n";

fn generated() -> String {
    let types = Types::default()
        .register::<Snapshot>()
        .register::<devports_core::adopt::review::AdoptionOptions>()
        .register::<devports_core::adopt::review::AdoptionPreview>()
        .register::<devports_core::adopt::review::AdoptionError>()
        .register::<devports_core::adopt::review::ServiceEdits>()
        .register::<devports_core::uses::ConsumerOptions>()
        .register::<devports_core::uses::UseRequest>()
        .register::<devports_core::uses::UsesError>()
        .register::<devports_core::ignore::IgnoreRule>()
        .register::<devports_core::engine::LogError>()
        .register::<devports_core::settings::ScanFolders>()
        .register::<devports_core::settings::ScanPreview>()
        .register::<devports_core::settings::SettingsError>()
        .register::<devports_core::classify::Classification>()
        .register::<devports_core::classify::ProcessClass>()
        .register::<devports_proto::Preferences>()
        .register::<devports_core::reasons::CommandError>()
        .register::<devports_core::engine::ActionError>()
        .register::<InstanceView>()
        .register::<LogEntry>()
        .register::<LogFacts>();
    let body = Typescript::default()
        .export(&types, specta_serde::Format)
        .expect("the types can be exported");
    // Specta emits trailing spaces around documented unions; normalize in the
    // generator so checked-in bindings stay deterministic without hand edits.
    let body = body
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    format!("{HEADER}{body}\n")
}

/// The fields of the panel's types that carry text written in Spanish by the core or by a
/// person, and why each is allowed (ADR-041, point 6). Anything else the core tells the panel
/// is a code and its parameters; a new free-text field fails the test below until it is either
/// made typed or added here with its reason.
const FREE_TEXT: [(&str, &str); 7] = [
    (
        "reason: string | null",
        "InstanceView: why the person started a Temporal, written by them",
    ),
    (
        "stale: string | null",
        "InstanceView: a Git-date hint, typed in UXR-9.8",
    ),
    (
        "summary: string",
        "LogEntry: the Spanish sentence of the CLI, a fallback for the panel",
    ),
    ("message: string }", "LogError: the store's own explanation"),
    (
        "reason: string | null",
        "LogFacts: the reason the person gave for a Temporal",
    ),
    (
        "docker_error: string | null",
        "Snapshot: Docker's own explanation, typed in UXR-9.8",
    ),
    (
        "problems: string[]",
        "Snapshot: diagnostics of manifests and configuration, typed in UXR-9.8",
    ),
];

// covers: UXR-9.7
#[test]
fn free_text_in_the_panel_types_is_a_short_known_list() {
    let text = generated();
    let found: Vec<&str> = text
        .lines()
        .map(str::trim)
        .map(|line| line.trim_end_matches(','))
        .filter(|line| {
            [
                "message:",
                "reason:",
                "summary:",
                "stale:",
                "problems:",
                "warnings:",
                "docker_error:",
                "why:",
                "detail:",
            ]
            .iter()
            .any(|name| line.starts_with(name))
                && line.contains("string")
        })
        .collect();
    // `detail` carries what the system or a parser said, as it said it: always allowed.
    let mut unexpected: Vec<&str> = found
        .iter()
        .copied()
        .filter(|line| !line.starts_with("detail:"))
        .collect();
    for (allowed, _why) in FREE_TEXT {
        if let Some(at) = unexpected
            .iter()
            .position(|line| *line == allowed || line.starts_with(allowed))
        {
            unexpected.remove(at);
        }
    }
    assert!(
        unexpected.is_empty(),
        "free text reaches the panel without being on the list: {unexpected:?}"
    );
}

#[test]
fn the_typescript_bindings_match_the_rust_types() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/bindings.ts");
    let text = generated();
    if std::env::var_os("UPDATE_BINDINGS").is_some() {
        std::fs::write(&path, &text).expect("write the bindings");
    }
    let committed = std::fs::read_to_string(&path).expect("the bindings exist");
    assert_eq!(committed, text, "regenerate with UPDATE_BINDINGS=1");
}
