# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Vision

A Rust reimplementation of KaTeX with the following goals:

- **Core**: Implement the parser, builder, and renderer in Rust as a reusable library.
- **Web**: Build a WASM target with `wasm-bindgen` + `wasm-pack` to accelerate parsing and rendering in the browser. (Other-language bindings via C ABI / FFI are a possible future direction but are explicitly out of scope for now.)
- **Examples**: Ship usage samples (e.g. under `examples/web/`) demonstrating browser integration.

## Architectural Principles

- **Mirror upstream KaTeX**: Follow the module layout and data structures of upstream [KaTeX](https://github.com/KaTeX/KaTeX) (`ParseNode`, `Token`, `Settings`, builder / dom-tree, MathML/HTML renderers, etc.) — names and responsibilities included — so upstream fixes and new features can be ported with minimal friction.
- **Deviate deliberately**: Prefer Rust idioms (ownership, the type system, `Result`-based error handling) where they yield a more robust design, and replace JS-era patterns (class hierarchies, mutable globals) when a Rust-native approach is clearly better. **When deviating, record the reason in a code comment or a short ADR-style note.**
- **Crate boundaries**: Keep core logic (parser / builder / renderer) free of OS- and environment-specific dependencies. Avoid pulling `std::io` or `wasm-bindgen` into the core; isolate the WASM binding into a separate crate within a Cargo workspace.

## Repository Layout

A Cargo workspace with a minimal surface: one core crate plus one WASM binding. No FFI crate, no other-language bindings, and no vendored upstream tree.

```
katex-rs/
├── Cargo.toml              # workspace root
├── CLAUDE.md
├── README.md
├── flake.nix               # see Toolchain
├── flake.lock
├── .envrc                  # direnv: `use flake`
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
│   │   └── src/            # module names mirror upstream KaTeX
│   │
│   └── katex-wasm/         # wasm-bindgen + wasm-pack target
│       ├── Cargo.toml
│       └── src/lib.rs
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

## Commit Conventions

Use the [Angular conventional commit](https://www.conventionalcommits.org/) format for every commit: `<type>(<optional scope>): <subject>`. Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, `build`, `ci`, `style`. Pick the type from the nature of the change (new docs → `docs:`, new feature code → `feat:`, etc.).
