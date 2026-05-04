//! `cargo xtask snapshot {bless,verify}`.
//!
//! Both commands walk every `crates/katex/tests/snapshots/inputs/*.tex`,
//! shell out to `node tools/render-mathml.mjs` to render with upstream
//! KaTeX, normalize, and either write to `expected/` (bless) or diff
//! against committed expected files (verify).

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};

use crate::normalize::normalize_mathml;

pub fn run(args: &[String]) -> Result<()> {
    let mode = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| anyhow!("snapshot: expected `bless` or `verify`"))?;
    match mode {
        "bless" => bless(),
        "verify" => verify(),
        other => bail!("snapshot: unknown mode {other:?} (expected `bless` or `verify`)"),
    }
}

fn workspace_root() -> Result<PathBuf> {
    // `cargo xtask` is invoked through the workspace alias, so CARGO_MANIFEST_DIR
    // points at xtask/. Walk one level up.
    let manifest = env_var("CARGO_MANIFEST_DIR")?;
    Ok(PathBuf::from(manifest)
        .parent()
        .ok_or_else(|| anyhow!("CARGO_MANIFEST_DIR has no parent"))?
        .to_path_buf())
}

fn env_var(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("environment variable {name} not set"))
}

struct Paths {
    inputs_dir: PathBuf,
    expected_dir: PathBuf,
    render_script: PathBuf,
    root: PathBuf,
}

fn paths() -> Result<Paths> {
    let root = workspace_root()?;
    Ok(Paths {
        inputs_dir: root.join("crates/katex/tests/snapshots/inputs"),
        expected_dir: root.join("crates/katex/tests/snapshots/expected"),
        render_script: root.join("tools/render-mathml.mjs"),
        root,
    })
}

fn collect_inputs(inputs_dir: &Path) -> Result<BTreeMap<String, String>> {
    if !inputs_dir.exists() {
        bail!(
            "inputs directory {} does not exist; create it and add .tex files",
            inputs_dir.display()
        );
    }
    let mut out = BTreeMap::new();
    for entry in
        fs::read_dir(inputs_dir).with_context(|| format!("read_dir {}", inputs_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("tex") {
            continue;
        }
        let slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("non-utf8 file name {}", path.display()))?
            .to_string();
        let body = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        out.insert(slug, body.trim_end_matches('\n').to_string());
    }
    Ok(out)
}

fn render_one(script: &Path, root: &Path, tex: &str) -> Result<String> {
    let mut child = Command::new("node")
        .arg(script)
        .arg("-")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn node {}", script.display()))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow!("failed to open stdin for node"))?
        .write_all(tex.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!(
            "node {} failed for input {tex:?}\n--- stderr ---\n{}",
            script.display(),
            String::from_utf8_lossy(&out.stderr),
        );
    }
    let raw = String::from_utf8(out.stdout).with_context(|| "node output was not utf8")?;
    Ok(normalize_mathml(&raw))
}

fn bless() -> Result<()> {
    let p = paths()?;
    let inputs = collect_inputs(&p.inputs_dir)?;
    fs::create_dir_all(&p.expected_dir)?;
    let mut count = 0usize;
    for (slug, tex) in &inputs {
        let mml = render_one(&p.render_script, &p.root, tex)?;
        let path = p.expected_dir.join(format!("{slug}.mml"));
        fs::write(&path, format!("{mml}\n"))
            .with_context(|| format!("write {}", path.display()))?;
        count += 1;
    }
    eprintln!(
        "snapshot bless: wrote {count} expected file(s) to {}",
        p.expected_dir.display()
    );
    Ok(())
}

fn verify() -> Result<()> {
    let p = paths()?;
    let inputs = collect_inputs(&p.inputs_dir)?;
    let mut mismatches: Vec<String> = Vec::new();
    let mut missing_committed: Vec<String> = Vec::new();
    let mut count = 0usize;
    for (slug, tex) in &inputs {
        let regenerated = render_one(&p.render_script, &p.root, tex)?;
        let committed_path = p.expected_dir.join(format!("{slug}.mml"));
        let committed = match fs::read_to_string(&committed_path) {
            Ok(s) => s,
            Err(_) => {
                missing_committed.push(slug.clone());
                continue;
            }
        };
        let committed_norm = committed.trim_end_matches('\n');
        if committed_norm != regenerated {
            mismatches.push(format!(
                "--- {slug} ---\nexpected: {committed_norm}\nactual:   {regenerated}"
            ));
        }
        count += 1;
    }
    if !missing_committed.is_empty() {
        bail!(
            "snapshot verify: missing committed expected files for: {}\nrun `cargo xtask snapshot bless`",
            missing_committed.join(", ")
        );
    }
    if !mismatches.is_empty() {
        bail!(
            "snapshot verify: {} mismatch(es) between committed expected files and regenerated upstream output:\n{}\nrun `cargo xtask snapshot bless` to update.",
            mismatches.len(),
            mismatches.join("\n")
        );
    }
    eprintln!("snapshot verify: {count} expected file(s) match upstream output");
    Ok(())
}
