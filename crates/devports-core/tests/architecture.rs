//! Architecture guard: the dependency rules of `ARQUITECTURA.md` section 2.
//!
//! These rules are what keep the core testable without an interface and the CLI
//! fast to start. They are checked against the real resolved dependency graph,
//! transitive edges included, so a forbidden crate cannot sneak in through an
//! innocent-looking dependency.
//!
//! The test spawns `cargo metadata`, which the workspace lint forbids elsewhere
//! (`clippy.toml`); the lint is relaxed here, in a test, on purpose.

// Test helpers outside `#[test]` functions are not covered by the `allow-*-in-tests`
// switches of `clippy.toml`, so the panicking accessors are allowed here explicitly.
#![allow(
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde_json::Value;

/// Resolved dependency graph keyed by package name.
struct Graph {
    edges: BTreeMap<String, BTreeSet<String>>,
}

impl Graph {
    fn load() -> Self {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let output = Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--locked"])
            .output()
            .expect("cargo metadata should run");
        assert!(
            output.status.success(),
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata: Value =
            serde_json::from_slice(&output.stdout).expect("cargo metadata emits JSON");

        let names: BTreeMap<&str, &str> = metadata["packages"]
            .as_array()
            .expect("packages is an array")
            .iter()
            .map(|package| {
                (
                    package["id"].as_str().expect("package id"),
                    package["name"].as_str().expect("package name"),
                )
            })
            .collect();

        let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for node in metadata["resolve"]["nodes"]
            .as_array()
            .expect("resolve.nodes is an array")
        {
            let id = node["id"].as_str().expect("node id");
            let from = (*names.get(id).expect("node refers to a package")).to_owned();
            let targets = edges.entry(from).or_default();
            for dep in node["dependencies"].as_array().expect("dependencies") {
                let dep_id = dep.as_str().expect("dependency id");
                targets.insert((*names.get(dep_id).expect("known package")).to_owned());
            }
        }
        Self { edges }
    }

    /// Every package reachable from `root`, `root` excluded.
    fn closure(&self, root: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut pending = vec![root.to_owned()];
        while let Some(current) = pending.pop() {
            for next in self.edges.get(&current).into_iter().flatten() {
                if seen.insert(next.clone()) {
                    pending.push(next.clone());
                }
            }
        }
        seen
    }
}

#[test]
fn core_does_not_depend_on_tauri() {
    let closure = Graph::load().closure("devports-core");
    assert!(
        !closure.contains("tauri"),
        "devports-core must stay free of tauri, found it in its dependency closure"
    );
}

#[test]
fn proto_depends_on_no_other_workspace_crate() {
    let closure = Graph::load().closure("devports-proto");
    for forbidden in ["devports-core", "devports-cli", "devports-app", "tauri"] {
        assert!(
            !closure.contains(forbidden),
            "devports-proto must not depend on {forbidden}"
        );
    }
}

#[test]
fn cli_depends_on_proto_only() {
    let closure = Graph::load().closure("devports-cli");
    assert!(
        closure.contains("devports-proto"),
        "devports-cli talks to the core through devports-proto"
    );
    for forbidden in ["devports-core", "devports-app", "tauri"] {
        assert!(
            !closure.contains(forbidden),
            "devports-cli must not depend on {forbidden}"
        );
    }
}

/// Source files of every workspace crate (`crates/*/src/**/*.rs`).
fn workspace_sources() -> Vec<(std::path::PathBuf, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("read crates dir") {
        let src = entry.expect("crate dir").path().join("src");
        if src.is_dir() {
            walk(&src, &mut files);
        }
    }
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("read source");
            (path, text)
        })
        .collect()
}

/// `unsafe_code` is `deny` workspace-wide (not `forbid`, so the macOS adapter
/// can relax it). This test is what keeps the exception to that one module.
#[test]
fn unsafe_is_confined_to_the_allowed_files() {
    let markers = [
        "unsafe {",
        "unsafe fn",
        "unsafe impl",
        "unsafe trait",
        "allow(unsafe_code",
    ];
    // Each exception is one file that talks to a C or Objective-C API with no safe
    // binding for what it needs (ADR-025).
    let allowed = [
        std::path::Path::new("devports-core/src/platform/macos.rs"),
        std::path::Path::new("devports-app/src/window_style.rs"),
    ];
    for (path, text) in workspace_sources() {
        if allowed.iter().any(|file| path.ends_with(file)) {
            continue;
        }
        for line in text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
        {
            for marker in markers {
                assert!(
                    !line.contains(marker),
                    "`{marker}` found outside the files allowed to use unsafe, in {}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn the_workspace_denies_unsafe_code() {
    let manifest = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml"),
    )
    .expect("read workspace manifest");
    assert!(
        manifest
            .lines()
            .any(|line| line.trim() == "unsafe_code = \"deny\""),
        "the workspace must keep `unsafe_code = \"deny\"`"
    );
}
