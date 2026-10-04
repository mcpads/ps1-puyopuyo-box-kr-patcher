//! 최상위 CLI 명령군을 역할별 router에 전달한다.

use std::env;

use super::{dispatch_build, dispatch_report, dispatch_review, dispatch_verify, usage};

pub(super) fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let command = args.next();
    let subcommand = args.next();
    let boundary = args.next();

    match command.as_deref() {
        Some(command) if command == "build" => {
            dispatch_build(subcommand.as_deref(), boundary.as_deref(), args)
        }
        Some(command) if command == "report" => {
            dispatch_report(subcommand.as_deref(), boundary.as_deref(), args)
        }
        Some(command) if command == "verify" => {
            dispatch_verify(subcommand.as_deref(), boundary.as_deref(), args)
        }
        Some(command) if command == "review" => {
            dispatch_review(subcommand.as_deref(), boundary.as_deref(), args)
        }
        _ => Err(usage().to_string()),
    }
}
