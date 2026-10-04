//! BTX family 사람 관측 화면을 loopback HTTP로만 제공한다.
//!
//! preview는 version 관리된 파생 PNG가 아니라 지원 원본 VDD에서 매번 읽고
//! CNX subtype·크기·해제 SHA-256을 현재 후보와 대조한 뒤 만든다. 쓰기는
//! source-derived baseline을 재검사하는 관측 원장 저장 경로 하나뿐이다.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use ps1_puyopuyo_box::btx::{Layout, PixelFormat};
use ps1_puyopuyo_box::compress::{decompress, is_cnx, parse_header};
use ps1_puyopuyo_box::graphics::btx_family_candidates::{
    BtxCandidateFamily, BtxFamilyCandidate, BtxReviewLane,
};
use ps1_puyopuyo_box::review::{
    audit_btx_family_review_decisions, render_btx_family_layout_review_page,
    render_btx_family_preview_bmp, render_btx_family_review_page, save_btx_family_review_decision,
    BtxFamilyReviewDecisionKind, BtxFamilyReviewLayout, BtxFamilyReviewSelection,
};

use super::btx_reports::{read_btx_family_source_inputs, BtxFamilySourceInputs};
use super::btx_review_layout_inputs::read_btx_review_layout_source_inputs;
use super::local_review_http::{
    bind_loopback, open_browser, parse_form, read_http_request, required_field, session_token,
    verify_session, write_http_response, HttpRequest, HttpResponse,
};
use super::usage::usage;

const DECISION_MANIFEST_PATH: &str = "assets/reviews/btx_family_decisions.tsv";

struct BtxFamilyReviewServer<'a> {
    inputs: &'a BtxFamilySourceInputs,
    source_layouts: Option<&'a [BtxFamilyReviewLayout]>,
    decision_path: &'a Path,
    session_token: &'a str,
}

fn decision_manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(DECISION_MANIFEST_PATH)
}

fn load_review(
    server: &BtxFamilyReviewServer<'_>,
) -> Result<ps1_puyopuyo_box::review::BtxFamilyReviewAnalysis, String> {
    let manifest = fs::read_to_string(server.decision_path).map_err(|error| {
        format!(
            "BTX family 검토 원장 읽기 실패 ({}): {error}",
            server.decision_path.display()
        )
    })?;
    audit_btx_family_review_decisions(&server.inputs.candidates, &manifest)
        .map_err(|error| error.to_string())
}

fn render_review(server: &BtxFamilyReviewServer<'_>) -> Result<HttpResponse, String> {
    let review = load_review(server)?;
    let html = match server.source_layouts {
        Some(layouts) => render_btx_family_layout_review_page(
            &server.inputs.candidates,
            &review,
            layouts,
            server.session_token,
            None,
        ),
        None => render_btx_family_review_page(
            &server.inputs.candidates,
            &review,
            server.session_token,
            None,
        ),
    }
    .map_err(|error| error.to_string())?;
    Ok(HttpResponse::html("200 OK", html))
}

fn preview_candidate_id(route: &str) -> Option<&str> {
    let candidate_id = route.strip_prefix("/preview/")?.strip_suffix(".bmp")?;
    if candidate_id.is_empty()
        || !candidate_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return None;
    }
    Some(candidate_id)
}

fn is_review_family(families: &[BtxCandidateFamily], family_id: &str) -> bool {
    families.iter().any(|family| {
        family.family_id == family_id && family.review_lane == BtxReviewLane::UiFontTextCandidate
    })
}

fn preview_candidate<'a>(
    server: &'a BtxFamilyReviewServer<'_>,
    candidate_id: &str,
) -> Result<&'a BtxFamilyCandidate, String> {
    let candidate = server
        .inputs
        .candidates
        .candidates
        .iter()
        .find(|candidate| candidate.candidate_id == candidate_id)
        .ok_or_else(|| format!("현재 source-derived 후보에 없는 candidate: {candidate_id}"))?;
    if !is_review_family(&server.inputs.candidates.families, &candidate.family_id) {
        return Err(format!(
            "UI·font·text 사람 검토 lane 밖 candidate: {candidate_id}"
        ));
    }
    if let Some(layouts) = server.source_layouts {
        if !layouts
            .iter()
            .any(|layout| layout.candidate_id == candidate.candidate_id)
        {
            return Err(format!(
                "source-bound layout 재판독 범위 밖 candidate: {candidate_id}"
            ));
        }
    }
    Ok(candidate)
}

fn preview_layout(
    server: &BtxFamilyReviewServer<'_>,
    candidate: &BtxFamilyCandidate,
) -> Result<Layout, String> {
    let (width, height) = match server.source_layouts {
        Some(layouts) => {
            let layout = layouts
                .iter()
                .find(|layout| layout.candidate_id == candidate.candidate_id)
                .ok_or_else(|| {
                    format!(
                        "source-bound layout 재판독 범위 밖 candidate: {}",
                        candidate.candidate_id
                    )
                })?;
            (layout.width, layout.stored_height)
        }
        None => (candidate.review_width, candidate.review_height),
    };
    Ok(Layout {
        width,
        height,
        format: candidate.format,
    })
}

fn render_preview(
    server: &BtxFamilyReviewServer<'_>,
    candidate_id: &str,
) -> Result<HttpResponse, String> {
    let candidate = preview_candidate(server, candidate_id)?;
    let copy = candidate
        .physical_copies
        .first()
        .ok_or_else(|| format!("{} physical copy가 없음", candidate.candidate_id))?;
    let archive = server
        .inputs
        .archives
        .iter()
        .find(|(archive_id, _)| *archive_id == copy.archive)
        .map(|(_, archive)| archive)
        .ok_or_else(|| format!("preview archive가 없음: {}", copy.archive))?;
    let entry = archive
        .find(&copy.name)
        .ok_or_else(|| format!("preview VDD entry가 없음: {}:{}", copy.archive, copy.name))?;
    let compressed = archive.read_file(entry);
    if !is_cnx(compressed) {
        return Err(format!(
            "preview source가 CNX v2가 아님: {}:{}",
            copy.archive, copy.name
        ));
    }
    let header = parse_header(compressed).map_err(|error| error.to_string())?;
    let format =
        PixelFormat::from_cnx_subtype(header.subtype).map_err(|error| error.to_string())?;
    if format != candidate.format {
        return Err(format!(
            "{} preview format drift: expected={:?}, actual={format:?}",
            candidate.candidate_id, candidate.format
        ));
    }
    if header.decompressed_size as usize != candidate.decompressed_bytes {
        return Err(format!(
            "{} preview 해제 크기 drift: expected={}, actual={}",
            candidate.candidate_id, candidate.decompressed_bytes, header.decompressed_size
        ));
    }
    let raw = decompress(compressed).map_err(|error| error.to_string())?;
    let digest = format!("{:x}", Sha256::digest(&raw));
    if digest != candidate.decompressed_sha256 {
        return Err(format!(
            "{} preview 원본 SHA-256 drift: expected={}, actual={digest}",
            candidate.candidate_id, candidate.decompressed_sha256
        ));
    }
    let layout = preview_layout(server, candidate)?;
    if layout.format != format {
        return Err(format!(
            "{} preview source-bound format drift: expected={format:?}, actual={:?}",
            candidate.candidate_id, layout.format
        ));
    }
    let bmp = render_btx_family_preview_bmp(&raw, layout).map_err(|error| error.to_string())?;
    Ok(HttpResponse::binary("200 OK", "image/bmp", bmp))
}

fn parse_decision(value: &str) -> Result<BtxFamilyReviewDecisionKind, String> {
    match value {
        "japanese-pixels-observed" => Ok(BtxFamilyReviewDecisionKind::JapanesePixelsObserved),
        "no-japanese-observed-in-representative" => {
            Ok(BtxFamilyReviewDecisionKind::NoJapaneseObservedInRepresentative)
        }
        "review-insufficient" => Ok(BtxFamilyReviewDecisionKind::ReviewInsufficient),
        _ => Err(format!("지원하지 않는 BTX family 사람 관측: {value}")),
    }
}

fn decision_selection(
    fields: &BTreeMap<String, String>,
    session_token: &str,
) -> Result<BtxFamilyReviewSelection, String> {
    verify_session(fields, session_token)?;
    Ok(BtxFamilyReviewSelection {
        family_key: required_field(fields, "family_key")?.to_owned(),
        baseline_sha256: required_field(fields, "baseline_sha256")?.to_owned(),
        decision: parse_decision(required_field(fields, "decision")?)?,
        notes: fields.get("notes").cloned().unwrap_or_default(),
    })
}

fn decide(
    server: &BtxFamilyReviewServer<'_>,
    fields: &BTreeMap<String, String>,
) -> Result<HttpResponse, String> {
    let selection = decision_selection(fields, server.session_token)?;
    save_btx_family_review_decision(
        server.decision_path,
        &server.inputs.candidates,
        required_field(fields, "expected_manifest_sha256")?,
        selection,
    )
    .map_err(|error| error.to_string())?;
    Ok(HttpResponse::redirect())
}

fn route_request(server: &BtxFamilyReviewServer<'_>, request: &HttpRequest) -> HttpResponse {
    let route = request.target.split('?').next().unwrap_or(&request.target);
    let result = match (request.method.as_str(), route) {
        ("GET", "/") if request.body.is_empty() => render_review(server),
        ("GET", route) if request.body.is_empty() => preview_candidate_id(route)
            .ok_or_else(|| "알 수 없는 BTX family preview route".to_owned())
            .and_then(|candidate_id| render_preview(server, candidate_id)),
        ("POST", "/decide") => {
            parse_form(&request.body).and_then(|fields| decide(server, &fields))
        }
        ("POST", "/stop") => parse_form(&request.body).and_then(|fields| {
            verify_session(&fields, server.session_token)?;
            let mut response = HttpResponse::html(
                "200 OK",
                "<!doctype html><html lang=ko><meta charset=utf-8><title>종료됨</title><p>BTX family 검토 server를 종료했습니다.</p></html>".to_owned(),
            );
            response.stop_server = true;
            Ok(response)
        }),
        _ => return HttpResponse::text("404 Not Found", "알 수 없는 local 검토 route\n"),
    };
    result.unwrap_or_else(|error| HttpResponse::text("409 Conflict", format!("{error}\n")))
}

fn review_server_args(args: impl Iterator<Item = OsString>) -> Result<(bool, PathBuf), String> {
    let args = args.collect::<Vec<_>>();
    let parsed = match args.as_slice() {
        [source_path] => (true, PathBuf::from(source_path)),
        [flag, source_path] if flag == "--no-open" => (false, PathBuf::from(source_path)),
        _ => return Err(usage().to_owned()),
    };
    Ok(parsed)
}

fn run_btx_family_review_server(
    open_automatically: bool,
    inputs: &BtxFamilySourceInputs,
    source_layouts: Option<&[BtxFamilyReviewLayout]>,
) -> Result<(), String> {
    let decision_path = decision_manifest_path();
    let token = session_token()?;
    let server = BtxFamilyReviewServer {
        inputs,
        source_layouts,
        decision_path: &decision_path,
        session_token: &token,
    };

    // 모든 원장 행과 현재 화면 scope의 preview가 유효한 뒤에만 socket을 연다.
    let review = load_review(&server)?;
    render_review(&server)?;
    for candidate in inputs
        .candidates
        .candidates
        .iter()
        .filter(|candidate| is_review_family(&inputs.candidates.families, &candidate.family_id))
        .filter(|candidate| {
            source_layouts.is_none_or(|layouts| {
                layouts
                    .iter()
                    .any(|layout| layout.candidate_id == candidate.candidate_id)
            })
        })
    {
        render_preview(&server, &candidate.candidate_id)?;
    }
    let listener = bind_loopback()?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("BTX family 검토 local 주소 확인 실패: {error}"))?;
    let url = format!("http://{address}/");
    println!("BTX family review: {url}");
    println!("  decision manifest: {}", decision_path.display());
    println!(
        "  ledger families {}, pending {}, visible source-bound layouts {}; runtime/in-game unverified",
        review.families,
        review.pending_human_review,
        source_layouts.map_or(0, |layouts| layouts.len()),
    );
    if open_automatically {
        open_browser("BTX family 검토", &url)?;
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

pub(super) fn serve_btx_family_review(args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let (open_automatically, source_path) = review_server_args(args)?;
    let inputs = read_btx_family_source_inputs(&source_path)?;
    run_btx_family_review_server(open_automatically, &inputs, None)
}

pub(super) fn serve_btx_layout_review(args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let (open_automatically, source_path) = review_server_args(args)?;
    let inputs = read_btx_review_layout_source_inputs(&source_path)?;
    run_btx_family_review_server(
        open_automatically,
        &inputs.family,
        Some(&inputs.review_layouts),
    )
}

#[cfg(test)]
#[path = "btx_family_review_server_tests.rs"]
mod tests;
