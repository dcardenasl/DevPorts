//! `ui/src/bindings.ts` is generated from the Rust types and never edited by hand
//! (ADR-025, point 9): this test writes what it should contain and fails when the
//! committed file differs, so a type cannot change on one side only.
//!
//! Regenerate with `UPDATE_BINDINGS=1 cargo test -p devports-core --features bindings --test bindings`.

#![allow(clippy::expect_used)]

use devports_core::runtime::Snapshot;
use devports_proto::views::{InstanceView, LogEntry};
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
        .register::<InstanceView>()
        .register::<LogEntry>();
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
