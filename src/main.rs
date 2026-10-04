//! `kr-patch` 명령행 진입점.

#[path = "bin/ps1_puyopuyo_box/mod.rs"]
mod cli;

use std::process::ExitCode;

fn main() -> ExitCode {
    cli::exit_code()
}
