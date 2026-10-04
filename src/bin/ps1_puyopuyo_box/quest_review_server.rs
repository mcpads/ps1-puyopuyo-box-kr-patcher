//! QUEST 사람 승인 화면을 loopback HTTP로만 제공한다.
//!
//! server는 정적 file server가 아니며 알려진 route만 처리한다. 쓰기는
//! `save_quest_translation_approval` 한 경로로만 전달하고, browser가 보낸
//! 현재 원장 digest와 결정 기준선을 다시 검사한다.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use ps1_puyopuyo_box::review::{
    audit_quest_manual_composition_catalog, build_quest_review_queue_source_layouts,
    build_quest_translation_review_catalog, build_quest_translation_review_queue,
    render_quest_review_page, save_quest_translation_approval, QuestApprovalSelection,
};
use ps1_puyopuyo_box::script::{
    audit_quest_translation_approval_manifest, QuestTranslationApprovalAnalysis,
    QuestTranslationApprovalKind,
};

use super::local_review_http::{
    bind_loopback, open_browser, parse_form, read_http_request, required_field, session_token,
    verify_session, write_http_response, HttpRequest, HttpResponse,
};
use super::quest_analysis_inputs::{read_quest_review_inputs, QuestReviewInputs};
use super::usage::usage;

const APPROVAL_MANIFEST_PATH: &str = "assets/translations/quest_approvals.tsv";

struct QuestReviewServer<'a> {
    inputs: &'a QuestReviewInputs,
    approval_path: &'a Path,
    session_token: &'a str,
}

fn approval_manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(APPROVAL_MANIFEST_PATH)
}

fn encode_query_component(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn requested_batch_id(target: &str) -> Result<Option<String>, String> {
    let Some((_, query)) = target.split_once('?') else {
        return Ok(None);
    };
    if query.is_empty() {
        return Err("QUEST 검토 query가 비어 있음".to_owned());
    }
    let fields = parse_form(query.as_bytes())?;
    if fields.len() != 1 || !fields.contains_key("batch") {
        return Err("QUEST 검토 query는 batch 하나만 허용함".to_owned());
    }
    Ok(Some(required_field(&fields, "batch")?.to_owned()))
}

fn load_review_inputs(
    server: &QuestReviewServer<'_>,
) -> Result<
    (
        QuestTranslationApprovalAnalysis,
        ps1_puyopuyo_box::review::QuestReviewCatalog,
        ps1_puyopuyo_box::review::QuestManualCompositionAnalysis,
    ),
    String,
> {
    let manifest = fs::read_to_string(server.approval_path).map_err(|error| {
        format!(
            "QUEST 승인 원장 읽기 실패 ({}): {error}",
            server.approval_path.display()
        )
    })?;
    let approvals = audit_quest_translation_approval_manifest(&server.inputs.workbook, &manifest)
        .map_err(|error| error.to_string())?;
    let catalog = build_quest_translation_review_catalog(&server.inputs.workbook, &approvals)
        .map_err(|error| error.to_string())?;
    let manual_compositions = audit_quest_manual_composition_catalog(
        &server.inputs.workbook,
        &approvals,
        &server.inputs.extraction.streams,
    )
    .map_err(|error| error.to_string())?;
    Ok((approvals, catalog, manual_compositions))
}

fn render_review(
    server: &QuestReviewServer<'_>,
    requested_batch_id: Option<&str>,
) -> Result<HttpResponse, String> {
    let (approvals, catalog, manual_compositions) = load_review_inputs(server)?;
    let batch_id = match requested_batch_id {
        Some(batch_id) => batch_id,
        None => catalog
            .batches
            .first()
            .map(|batch| batch.batch_id.as_str())
            .ok_or_else(|| "QUEST 검토 색인이 비어 있음".to_owned())?,
    };
    if !catalog
        .batches
        .iter()
        .any(|batch| batch.batch_id == batch_id)
    {
        return Err(format!("QUEST 검토 색인에 없는 batch: {batch_id}"));
    }
    let queue = build_quest_translation_review_queue(&server.inputs.workbook, &approvals, batch_id)
        .map_err(|error| error.to_string())?;
    let source_layouts =
        build_quest_review_queue_source_layouts(&queue, &server.inputs.extraction.streams)
            .map_err(|error| error.to_string())?;
    let html = render_quest_review_page(
        &catalog,
        &queue,
        &source_layouts,
        &manual_compositions.compositions,
        server.session_token,
        None,
    )
    .map_err(|error| error.to_string())?;
    Ok(HttpResponse::html("200 OK", html))
}

fn approve(
    server: &QuestReviewServer<'_>,
    fields: &BTreeMap<String, String>,
) -> Result<HttpResponse, String> {
    verify_session(fields, server.session_token)?;
    let kind = match required_field(fields, "row_kind")? {
        "shared-dependency-decision" => QuestTranslationApprovalKind::SharedDependency,
        "translation-entry-decision" => QuestTranslationApprovalKind::TranslationEntry,
        _ => return Err("승인 row kind 오류".to_owned()),
    };
    let selection = QuestApprovalSelection {
        kind,
        decision_id: required_field(fields, "decision_id")?.to_owned(),
        baseline_sha256: required_field(fields, "baseline_sha256")?.to_owned(),
        selected_korean: required_field(fields, "selected_ko")?.to_owned(),
        notes: required_field(fields, "notes")?.to_owned(),
    };
    let batch_id = required_field(fields, "batch_id")?;
    let manifest = fs::read_to_string(server.approval_path).map_err(|error| {
        format!(
            "QUEST 승인 원장 읽기 실패 ({}): {error}",
            server.approval_path.display()
        )
    })?;
    let approvals = audit_quest_translation_approval_manifest(&server.inputs.workbook, &manifest)
        .map_err(|error| error.to_string())?;
    let queue = build_quest_translation_review_queue(&server.inputs.workbook, &approvals, batch_id)
        .map_err(|error| error.to_string())?;
    let belongs_to_batch = match selection.kind {
        QuestTranslationApprovalKind::TranslationEntry => queue
            .entries
            .iter()
            .any(|entry| entry.stream_id == selection.decision_id),
        QuestTranslationApprovalKind::SharedDependency => queue
            .dependencies
            .iter()
            .any(|dependency| dependency.id == selection.decision_id),
    };
    if !belongs_to_batch {
        return Err(format!(
            "QUEST 승인 결정이 현재 batch에 속하지 않음: batch={batch_id}, decision={}",
            selection.decision_id
        ));
    }
    save_quest_translation_approval(
        server.approval_path,
        &server.inputs.workbook,
        required_field(fields, "expected_manifest_sha256")?,
        selection,
    )
    .map_err(|error| error.to_string())?;
    Ok(HttpResponse::redirect_to(format!(
        "/?batch={}",
        encode_query_component(batch_id)
    )))
}

fn reject_unlaid_preview(
    fields: &BTreeMap<String, String>,
    session_token: &str,
) -> Result<HttpResponse, String> {
    verify_session(fields, session_token)?;
    Err("명시적 사람 조판이 없으므로 QUEST 후보 미리보기를 만들 수 없음".to_owned())
}

fn route_request(server: &QuestReviewServer<'_>, request: &HttpRequest) -> HttpResponse {
    let route = request.target.split('?').next().unwrap_or(&request.target);
    let result = match (request.method.as_str(), route) {
        ("GET", "/") if request.body.is_empty() => requested_batch_id(&request.target)
            .and_then(|batch_id| render_review(server, batch_id.as_deref())),
        ("POST", "/approve") => {
            parse_form(&request.body).and_then(|fields| approve(server, &fields))
        }
        ("POST", "/preview") => parse_form(&request.body)
            .and_then(|fields| reject_unlaid_preview(&fields, server.session_token)),
        ("POST", "/stop") => parse_form(&request.body).and_then(|fields| {
            verify_session(&fields, server.session_token)?;
            let mut response = HttpResponse::html(
                "200 OK",
                "<!doctype html><html lang=ko><meta charset=utf-8><title>종료됨</title><p>QUEST 검토 server를 종료했습니다.</p></html>".to_owned(),
            );
            response.stop_server = true;
            Ok(response)
        }),
        _ => return HttpResponse::text("404 Not Found", "알 수 없는 local 검토 route\n"),
    };
    result.unwrap_or_else(|error| HttpResponse::text("409 Conflict", format!("{error}\n")))
}

pub(super) fn serve_quest_review(args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let args = args.collect::<Vec<_>>();
    let (open_automatically, source_path) = match args.as_slice() {
        [source_path] => (true, PathBuf::from(source_path)),
        [flag, source_path] if flag == "--no-open" => (false, PathBuf::from(source_path)),
        _ => return Err(usage().to_owned()),
    };
    let inputs = read_quest_review_inputs(&source_path)?;
    let approval_path = approval_manifest_path();
    let token = session_token()?;
    let server = QuestReviewServer {
        inputs: &inputs,
        approval_path: &approval_path,
        session_token: &token,
    };

    // 모든 입력·후보·이미지가 유효한 뒤에만 사용자가 접속할 socket을 연다.
    render_review(&server, None)?;
    let listener = bind_loopback()?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("QUEST 검토 local 주소 확인 실패: {error}"))?;
    let url = format!("http://{address}/");
    println!("QUEST translation review: {url}");
    println!("  approval manifest: {}", approval_path.display());
    if open_automatically {
        open_browser("QUEST 검토", &url)?;
    }

    for connection in listener.incoming() {
        let mut stream = connection.map_err(|error| format!("HTTP accept 실패: {error}"))?;
        let response = match read_http_request(&mut stream) {
            Ok(request) => route_request(&server, &request),
            Err(error) => HttpResponse::text("400 Bad Request", format!("{error}\n")),
        };
        let stop_server = response.stop_server;
        write_http_response(&mut stream, &response)?;
        if stop_server {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_review_server_tests.rs"]
mod tests;
