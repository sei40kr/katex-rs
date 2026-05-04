//! Workspace task runner. Hosts everything that needs Node, the
//! filesystem, or other env-coupled bits — kept out of `crates/katex`
//! so the core library stays environment-independent.
//!
//! Subcommands:
//!   cargo xtask snapshot bless
//!   cargo xtask snapshot verify
//!   cargo xtask vendor-update --upstream-rev <sha> [--version <ver>]

use std::process::ExitCode;

mod normalize;
mod snapshot;
mod vendor;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let rest: Vec<String> = args.collect();
    let result = match cmd.as_str() {
        "snapshot" => snapshot::run(&rest),
        "vendor-update" => vendor::run(&rest),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        other => {
            eprintln!("xtask: unknown subcommand {other:?}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("xtask: {err:#}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "usage:
  cargo xtask snapshot bless              regenerate committed expected MathML files
  cargo xtask snapshot verify             regenerate into a tempdir and diff vs committed
  cargo xtask vendor-update --upstream-rev <sha> [--version <ver>]
                                          rotate crates/katex/vendor/UPSTREAM
";
