//! Fails when a layer reaches past the hexagon's dependency direction.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// Source lines of a layer, without comments or its `#[cfg(test)]` module.
fn code_lines(layer: &str) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(layer),
        &mut files,
    );
    assert!(!files.is_empty(), "no source files for {layer}");
    files
        .into_iter()
        .flat_map(|path| {
            let text = fs::read_to_string(&path).unwrap();
            let production = text
                .split("#[cfg(test)]")
                .next()
                .unwrap_or_default()
                .to_owned();
            production
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .map(|line| (path.clone(), line.to_owned()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn assert_absent(layer: &str, forbidden: &[&str]) {
    let violations: Vec<String> = code_lines(layer)
        .iter()
        .filter(|(_, line)| forbidden.iter().any(|word| line.contains(word)))
        .map(|(path, line)| format!("{}: {}", path.display(), line.trim()))
        .collect();
    assert!(
        violations.is_empty(),
        "{layer} breaks the boundary:\n{}",
        violations.join("\n")
    );
}

#[test]
fn domain_depends_on_nothing_else_in_the_crate() {
    assert_absent(
        "domain",
        &["crate::core", "crate::ports", "crate::adapters"],
    );
}

#[test]
fn ports_depend_only_on_the_domain() {
    assert_absent("ports", &["crate::core", "crate::adapters"]);
}

#[test]
fn the_core_never_names_an_adapter() {
    assert_absent("core", &["crate::adapters::", "use crate::adapters"]);
}

#[test]
fn the_core_touches_the_outside_world_only_through_ports() {
    assert_absent(
        "core",
        &[
            "std::fs",
            "fs::",
            "std::process",
            "Command::new",
            "std::thread",
            "thread::sleep",
            "Instant",
            "SystemTime",
            "std::env",
        ],
    );
}

#[test]
fn the_library_never_uses_binary_only_dependencies() {
    for layer in ["domain", "ports", "core", "adapters"] {
        assert_absent(layer, &["clap::", "anyhow::", "rmcp::", "tokio::"]);
    }
}
