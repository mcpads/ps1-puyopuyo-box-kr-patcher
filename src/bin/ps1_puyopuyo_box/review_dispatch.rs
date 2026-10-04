//! 사람이 승인 결정을 남기는 local 검토 명령을 routing한다.

use std::ffi::{OsStr, OsString};

use super::btx_family_review_server::{serve_btx_family_review, serve_btx_layout_review};
use super::quest_review_server::serve_quest_review;
use super::usage::usage;

pub(super) fn dispatch_review(
    subcommand: Option<&OsStr>,
    boundary: Option<&OsStr>,
    args: impl Iterator<Item = OsString>,
) -> Result<(), String> {
    if subcommand == Some("serve".as_ref()) && boundary == Some("btx-families".as_ref()) {
        return serve_btx_family_review(args);
    }
    if subcommand == Some("serve".as_ref()) && boundary == Some("btx-layout-review".as_ref()) {
        return serve_btx_layout_review(args);
    }
    if subcommand == Some("serve".as_ref()) && boundary == Some("quest".as_ref()) {
        return serve_quest_review(args);
    }
    Err(usage().to_string())
}
