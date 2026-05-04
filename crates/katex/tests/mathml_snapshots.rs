//! Snapshot test driver. For every `tests/snapshots/inputs/<slug>.tex`
//! we render via the public Rust API, normalize, and assert byte
//! equality against the committed `tests/snapshots/expected/<slug>.mml`
//! that `cargo xtask snapshot bless` produced from upstream KaTeX.
//!
//! This test never shells out to Node — `cargo test` works without a
//! Node toolchain installed. Drift between the committed expected
//! files and current upstream output is caught by a separate CI job
//! that runs `cargo xtask snapshot verify`.
//!
//! Slugs listed in `tests/snapshots/known_mismatches.txt` are skipped
//! (parity bugs we haven't fixed yet). If a known-mismatch slug starts
//! matching, the test fails so the line gets pruned and the corpus
//! stays honest.

#[path = "../../../xtask/src/normalize.rs"]
mod normalize;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use katex::{Settings, render_to_mathml_string};
use normalize::normalize_mathml;

fn snapshots_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn collect_inputs(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let read_dir = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return out,
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("tex") {
            continue;
        }
        let slug = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let body =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        out.insert(slug, body.trim_end_matches('\n').to_string());
    }
    out
}

fn load_known_mismatches(path: &Path) -> BTreeSet<String> {
    let text = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return BTreeSet::new(),
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[test]
fn mathml_snapshots_match_upstream() {
    let root = snapshots_dir();
    let inputs = collect_inputs(&root.join("inputs"));
    assert!(
        !inputs.is_empty(),
        "no snapshot inputs found under {} — populate tests/snapshots/inputs/*.tex",
        root.join("inputs").display()
    );
    let known_mismatches = load_known_mismatches(&root.join("known_mismatches.txt"));
    let unknown_known: Vec<&String> = known_mismatches
        .iter()
        .filter(|s| !inputs.contains_key(s.as_str()))
        .collect();
    assert!(
        unknown_known.is_empty(),
        "known_mismatches.txt lists slug(s) with no matching input file: {unknown_known:?}"
    );

    let settings = Settings::default();
    let mut failures: Vec<String> = Vec::new();
    let mut now_passing: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for (slug, tex) in &inputs {
        let expected_path = root.join("expected").join(format!("{slug}.mml"));
        let expected = match fs::read_to_string(&expected_path) {
            Ok(s) => s,
            Err(_) => {
                missing.push(slug.clone());
                continue;
            }
        };
        let expected = expected.trim_end_matches('\n').to_string();

        let actual_raw = match render_to_mathml_string(tex, &settings) {
            Ok(s) => s,
            Err(err) => {
                if !known_mismatches.contains(slug) {
                    failures.push(format!("[{slug}] render error: {err}\n  input: {tex}"));
                }
                continue;
            }
        };
        let actual = normalize_mathml(&actual_raw);
        let matches = actual == expected;
        let known = known_mismatches.contains(slug);
        match (matches, known) {
            (true, false) | (false, true) => {}
            (false, false) => failures.push(format!(
                "[{slug}] mismatch\n  input:    {tex}\n  expected: {expected}\n  actual:   {actual}"
            )),
            (true, true) => now_passing.push(slug.clone()),
        }
    }

    if !missing.is_empty() {
        panic!(
            "missing committed expected files for: {}\nrun `cargo xtask snapshot bless`",
            missing.join(", ")
        );
    }
    if !now_passing.is_empty() {
        panic!(
            "{} snapshot(s) listed in known_mismatches.txt now match upstream — \
             remove them from the file:\n  {}",
            now_passing.len(),
            now_passing.join("\n  ")
        );
    }
    if !failures.is_empty() {
        panic!(
            "{} snapshot failure(s):\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}
