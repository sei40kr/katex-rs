# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Vision

A Rust reimplementation of KaTeX with the following goals:

- **Core**: Implement the parser, builder, and renderer in Rust as a reusable library.
- **Web**: Build a WASM target with `wasm-bindgen` + `wasm-pack` to accelerate parsing and rendering in the browser. (Other-language bindings via C ABI / FFI are a possible future direction but are explicitly out of scope for now.)
- **Examples**: Ship usage samples (e.g. under `examples/web/`) demonstrating browser integration.
- **Output formats**: MathML first; HTML+CSS rendering is a deliberate later milestone, since it requires porting font metrics and glue/spacing layout that MathML output sidesteps.

## Architectural Principles

- **Mirror upstream KaTeX**: Follow the module layout and data structures of upstream [KaTeX](https://github.com/KaTeX/KaTeX) (`ParseNode`, `Token`, `Settings`, builder / dom-tree, MathML/HTML renderers, etc.) — names and responsibilities included — so upstream fixes and new features can be ported with minimal friction.
- **Deviate deliberately**: Prefer Rust idioms (ownership, the type system, `Result`-based error handling) where they yield a more robust design, and replace JS-era patterns (class hierarchies, mutable globals) when a Rust-native approach is clearly better. **When deviating, record the reason in a code comment or a short ADR-style note.**
- **Crate boundaries**: Keep core logic (parser / builder / renderer) free of OS- and environment-specific dependencies. `crates/katex` must not depend on `std::io`, `std::fs`, `wasm-bindgen`, `tracing`/`log`, `anyhow`/`eyre`, or wall-clock types like `std::time::Instant`. Public errors are concrete (typically `thiserror`); avoid erased error crates. `wasm-bindgen` lives only in `crates/katex-wasm`. Enforce with `#![forbid(unsafe_code)]` in the core crate and a `cargo deny` rule banning `wasm-bindgen` under `crates/katex`.

## Repository Layout

A Cargo workspace with a minimal surface: one core crate, one WASM binding, plus an `xtask` runner for offline tooling. No FFI crate, no other-language bindings, and no vendored upstream tree (only minimal JSON snapshots consumed by build-time codegen).

```
katex-rs/
├── Cargo.toml              # workspace root
├── CLAUDE.md
├── README.md
├── flake.nix               # see Toolchain
├── flake.lock
├── .envrc                  # direnv: `use flake`
├── package.json            # pins upstream `katex` for the snapshot oracle
├── package-lock.json
│
├── nix/                    # blueprint reads from here (prefix = "nix")
│   ├── devshell.nix
│   ├── formatter.nix
│   ├── treefmt.nix
│   └── checks/
│       └── pre-commit-check.nix
│
├── crates/
│   ├── katex/              # core library — env-independent
│   │   ├── Cargo.toml
│   │   ├── build.rs        # codegen of static tables from vendored JSON
│   │   ├── src/            # module names mirror upstream KaTeX
│   │   ├── tests/
│   │   │   └── snapshots/  # inputs/*.tex + expected/*.{mml,html}
│   │   └── vendor/
│   │       ├── UPSTREAM    # commit / version / fetched-at
│   │       └── katex-<sha>/ # JSON snapshots from upstream KaTeX
│   │
│   └── katex-wasm/         # wasm-bindgen + wasm-pack target
│       ├── Cargo.toml
│       └── src/lib.rs
│
├── xtask/                  # workspace member: snapshot bless/verify, vendor-update
│   ├── Cargo.toml
│   └── src/main.rs
│
├── tools/
│   └── render-mathml.mjs   # Node script invoking upstream katex
│
├── examples/
│   └── web/                # browser integration sample (consumes katex-wasm)
│
└── assets/
    └── fonts/              # KaTeX fonts shipped with the web example
```

Module names and responsibilities inside `crates/katex/src/` mirror upstream KaTeX so ports stay mechanical. The exact module breakdown is established as the port progresses, not fixed up front in this document.

## Toolchain

The Rust toolchain and dev tooling are provided through Nix. Enter the dev shell with `nix develop`, or via direnv using the checked-in `.envrc`.

- **Nix flake** (`flake.nix`) defines the dev environment. The flake is bootstrapped from the [`treefmt-and-git-hooks`](https://github.com/numtide/blueprint/tree/main/templates/treefmt-and-git-hooks) blueprint template, which brings:
  - **[blueprint](https://github.com/numtide/blueprint)** with `prefix = "nix"` (auto-loads `nix/devshell.nix`, `nix/formatter.nix`, `nix/checks/*`).
  - **[treefmt-nix](https://github.com/numtide/treefmt-nix)** as the formatter aggregator (`nix/treefmt.nix`).
  - **[git-hooks.nix](https://github.com/cachix/git-hooks.nix)** wired into the dev shell to install pre-commit hooks on shell entry.
- **[fenix](https://github.com/nix-community/fenix)** is added on top of the template to provide the Rust toolchain (including the `wasm32-unknown-unknown` target). Channel and component selection live in `nix/devshell.nix` and are not pinned in this document.
- `nix/treefmt.nix` is extended beyond the template default to cover Rust/JS/etc. as needed.

## Static Data Tables

Tables (`symbols`, `fontMetricsData`, `macros`, `spacingData`, `unicodeAccents`, `unicodeSymbols`, `unicodeScripts`, `unicodeSupOrSub`) are **generated at build time** by `crates/katex/build.rs` from JSON snapshots vendored under `crates/katex/vendor/katex-<sha>/`.

- **No network in `build.rs`.** Builds must be reproducible offline; vendored JSON is the only input.
- Generated Rust source lands in `$OUT_DIR` (not committed); typed wrappers in `crates/katex/src/` `include!` the generated files.
- The pinned upstream commit and version are recorded in `crates/katex/vendor/UPSTREAM`.
- Refreshing vendored snapshots is the responsibility of `cargo xtask vendor-update --upstream-rev <sha>` — the only place that touches the network or invokes Node.
- Build deps stay narrow (e.g. `serde_json`, `phf_codegen`); runtime data deps stay narrower still (e.g. `phf`).

## Testing

Parity with upstream KaTeX is anchored by **snapshot tests against upstream** (Node):

- Reference inputs live under `crates/katex/tests/snapshots/inputs/*.tex`; expected outputs under `crates/katex/tests/snapshots/expected/*.{mml,html}`. Both are committed.
- Expected files are generated via `cargo xtask snapshot bless`, which calls `tools/render-mathml.mjs` (and later an HTML counterpart). The Node script imports the upstream `katex` version pinned in repo-root `package.json` / `package-lock.json`.
- **The Rust test driver never shells out to Node.** It reads expected files, renders via the public API, normalizes both, and asserts equality — so `cargo test` works without Node installed.
- A separate CI job runs `npm ci && cargo xtask snapshot verify` to catch silent edits to expected files.

Module-level unit tests live alongside their modules (`#[cfg(test)] mod tests`).

## Task Tracking

Multi-session work is tracked as **GitHub issues** in this repo, not in plan files or in-conversation todo lists. Open issues are the source of truth for current scope and roadmap. Check `gh issue list` when starting work; open or update an issue when proposing new work rather than maintaining a parallel checklist.

## Commit Conventions

Use the [Angular conventional commit](https://www.conventionalcommits.org/) format for every commit: `<type>(<optional scope>): <subject>`. Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, `build`, `ci`, `style`. Pick the type from the nature of the change (new docs → `docs:`, new feature code → `feat:`, etc.).
