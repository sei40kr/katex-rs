//! Phase-6 manual smoke check for [`katex::render_to_mathml_string`].
//!
//! Usage:
//! ```text
//! cargo run --example mathml_demo -- '\frac{1}{2}'
//! ```
//!
//! Reads the TeX source from the first CLI argument and prints the
//! corresponding MathML markup to stdout. Suitable for piping into a
//! browser via a tiny HTML wrapper.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(tex) = args.next() else {
        eprintln!("usage: mathml_demo '<tex source>'");
        return ExitCode::from(2);
    };
    let settings = katex::Settings::default();
    match katex::render_to_mathml_string(&tex, &settings) {
        Ok(mml) => {
            println!("{mml}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("render error: {err}");
            ExitCode::FAILURE
        }
    }
}
