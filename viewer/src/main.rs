//! The `oikos` binary — the G2d read-only debug viewer.
//!
//! It does nothing but parse the program arguments, hand them to
//! [`viewer::run`], and print the result: the rendered text on success, or the
//! error message (with its usage block) on failure. All the logic — dispatch,
//! the seeded run, and the deterministic renderers — lives in the library so it
//! is unit-testable without touching stdout.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match viewer::run(&args) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprint!("{message}");
            ExitCode::FAILURE
        }
    }
}
