//! `cargo xtask vendor-update --upstream-rev <sha> [--version <ver>]`
//!
//! Rotates the `crates/katex/vendor/UPSTREAM` pin and renames the
//! `katex-<version>/` directory holding the JSON snapshots that
//! `build.rs` consumes. The actual JSON tables are produced by the
//! upstream-aware extractor at `tools/extract-vendor-json.mjs` (which
//! must be run separately when the snapshot format changes).
//!
//! The Phase 2 build already shipped JSON for `katex@0.16.45`; this
//! command exists so future bumps don't have to hand-edit the pin.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};

pub fn run(args: &[String]) -> Result<()> {
    let mut rev: Option<String> = None;
    let mut version: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--upstream-rev" => {
                rev = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--upstream-rev requires a value"))?
                        .clone(),
                );
            }
            "--version" => {
                version = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--version requires a value"))?
                        .clone(),
                );
            }
            other => bail!("vendor-update: unknown argument {other:?}"),
        }
    }
    let rev = rev.ok_or_else(|| anyhow!("vendor-update: --upstream-rev <sha> is required"))?;

    let root = workspace_root()?;
    let vendor_dir = root.join("crates/katex/vendor");
    let upstream_pin = vendor_dir.join("UPSTREAM");

    let resolved_version = match version {
        Some(v) => v,
        None => read_pkg_version(&root.join("package.json"))?,
    };

    let timestamp = iso8601_now();
    let body = format!("commit={rev}\nversion={resolved_version}\nfetched={timestamp}\n");
    fs::write(&upstream_pin, &body).with_context(|| format!("write {}", upstream_pin.display()))?;
    eprintln!(
        "vendor-update: wrote {}\n  commit={rev}\n  version={resolved_version}",
        upstream_pin.display()
    );

    let target_dir = vendor_dir.join(format!("katex-{resolved_version}"));
    if !target_dir.exists() {
        eprintln!(
            "vendor-update: note: {} does not yet exist — regenerate the JSON snapshots with `node tools/extract-vendor-json.mjs --version {resolved_version}` (not yet implemented; see issue #8)",
            target_dir.display()
        );
    }
    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .context("CARGO_MANIFEST_DIR not set; run via `cargo xtask`")?;
    Ok(PathBuf::from(manifest)
        .parent()
        .ok_or_else(|| anyhow!("CARGO_MANIFEST_DIR has no parent"))?
        .to_path_buf())
}

fn read_pkg_version(pkg_json: &std::path::Path) -> Result<String> {
    let body =
        fs::read_to_string(pkg_json).with_context(|| format!("read {}", pkg_json.display()))?;
    // Parse the dependencies.katex field with a tiny hand-rolled scan to
    // avoid pulling serde_json into the xtask just for this.
    let key = "\"katex\"";
    let i = body
        .find(key)
        .ok_or_else(|| anyhow!("no \"katex\" entry in {}", pkg_json.display()))?;
    let after = &body[i + key.len()..];
    let colon = after
        .find(':')
        .ok_or_else(|| anyhow!("malformed \"katex\" entry in package.json"))?;
    let after_colon = &after[colon + 1..];
    let q1 = after_colon
        .find('"')
        .ok_or_else(|| anyhow!("expected quoted version in package.json"))?;
    let rest = &after_colon[q1 + 1..];
    let q2 = rest
        .find('"')
        .ok_or_else(|| anyhow!("unterminated version string in package.json"))?;
    Ok(rest[..q2].trim_start_matches(['^', '~', '=']).to_string())
}

fn iso8601_now() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    let (y, mo, d, h, mi, s) = epoch_to_civil(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{millis:03}Z")
}

// Howard Hinnant's days_from_civil, inverted.
fn epoch_to_civil(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let z = (secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    let day_secs = secs % 86_400;
    let h = (day_secs / 3_600) as u32;
    let mi = ((day_secs % 3_600) / 60) as u32;
    let s = (day_secs % 60) as u32;
    (y, m, d, h, mi, s)
}
