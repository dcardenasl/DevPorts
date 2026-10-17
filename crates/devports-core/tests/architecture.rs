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
