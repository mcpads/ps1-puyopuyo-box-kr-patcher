//! BTX family 대표 관측 원장과 source-derived preview를 local HTML로 보여준다.
//!
//! 이 화면은 원본 palette index와 검토 layout만 보여준다. 실제 CLUT,
//! consumer layout, 화면 표시 여부는 확정하지 않으며 browser 자체 상태도
//! 사람 관측으로 취급하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::btx::Layout;
use crate::graphics::btx_family_candidates::{
    BtxCandidateFamily, BtxFamilyCandidate, BtxFamilyCandidateAnalysis, BtxReviewLane,
};

use super::{BtxFamilyReviewAnalysis, BtxFamilyReviewDecision, BtxFamilyReviewDecisionKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyReviewHtmlError(String);

impl fmt::Display for BtxFamilyReviewHtmlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for BtxFamilyReviewHtmlError {}

fn error(message: impl Into<String>) -> BtxFamilyReviewHtmlError {
    BtxFamilyReviewHtmlError(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtxFamilyReviewLayoutBasis {
    ExactDescriptorSize,
    RowPaddedAtlas,
}

impl BtxFamilyReviewLayoutBasis {
    const fn label(self) -> &'static str {
        match self {
            Self::ExactDescriptorSize => "descriptor 크기와 atlas 일치",
            Self::RowPaddedAtlas => "descriptor 행 + atlas padding",
        }
    }
}

/// 정적 소비자 근거로 다시 볼 수 있게 닫힌 한 BTX 후보의 저장 layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyReviewLayout {
    pub family_key: String,
    pub candidate_id: String,
    pub width: usize,
    pub stored_height: usize,
    pub atlas_height: usize,
    pub basis: BtxFamilyReviewLayoutBasis,
}

fn escape_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
    }
    output
}

fn decision_description(decision: BtxFamilyReviewDecisionKind) -> &'static str {
    match decision {
        BtxFamilyReviewDecisionKind::PendingHumanReview => "사람 관측 대기",
        BtxFamilyReviewDecisionKind::JapanesePixelsObserved => "대표에서 일본어 픽셀을 식별함",
        BtxFamilyReviewDecisionKind::NoJapaneseObservedInRepresentative => {
            "대표에서는 일본어를 식별하지 못함 (family 전체 제외 아님)"
        }
        BtxFamilyReviewDecisionKind::ReviewInsufficient => {
            "layout·색·판독성 부족으로 현재 preview만으로 판단 불가"
        }
    }
}

fn checked(
    current: BtxFamilyReviewDecisionKind,
    option: BtxFamilyReviewDecisionKind,
) -> &'static str {
    if current == option {
        " checked"
    } else {
        ""
    }
}

fn family_members<'a>(
    candidates: &'a BtxFamilyCandidateAnalysis,
    family: &BtxCandidateFamily,
) -> Result<Vec<&'a BtxFamilyCandidate>, BtxFamilyReviewHtmlError> {
    let mut members = candidates
        .candidates
        .iter()
        .filter(|candidate| candidate.family_id == family.family_id)
        .collect::<Vec<_>>();
    members.sort_by_key(|candidate| candidate.candidate_id.as_str());
    if members.len() != family.members {
        return Err(error(format!(
            "{} HTML family member 분모 불일치: expected={}, actual={}",
            family.family_id,
            family.members,
            members.len()
        )));
    }
    Ok(members)
}

fn render_candidate(
    output: &mut String,
    candidate: &BtxFamilyCandidate,
    representative: bool,
    source_layout: Option<&BtxFamilyReviewLayout>,
) {
    let (width, height) = source_layout
        .map(|layout| (layout.width, layout.stored_height))
        .unwrap_or((candidate.review_width, candidate.review_height));
    writeln!(
        output,
        "<section class=asset><h4>{}{} </h4><img src=\"/preview/{}.bmp\" alt=\"{} palette-index preview\"><div class=badges><span>{}×{}</span><span>{:?}</span><span>배경 후보 {}</span><span>물리 사본 {}</span></div><p><code>{}</code></p>",
        if representative { "대표 · " } else { "구성원 · " },
        escape_html(&candidate.candidate_id),
        escape_html(&candidate.candidate_id),
        escape_html(&candidate.candidate_id),
        width,
        height,
        candidate.format,
        candidate.candidate_background_index,
        candidate.physical_copies.len(),
        escape_html(&candidate.decompressed_sha256),
    )
    .expect("String write는 실패하지 않음");
    if let Some(layout) = source_layout {
        writeln!(
            output,
            "<p class=layout-basis><strong>정적 저장 layout:</strong> {}×{} / atlas {}행 · {}</p>",
            layout.width,
            layout.stored_height,
            layout.atlas_height,
            escape_html(layout.basis.label()),
        )
        .expect("String write는 실패하지 않음");
    }
    output.push_str("<ul>");
    for copy in &candidate.physical_copies {
        writeln!(
            output,
            "<li><code>{}:{}</code> · resource {} · {}</li>",
            escape_html(&copy.archive),
            escape_html(&copy.name),
            copy.resource_index,
            escape_html(&copy.static_evidence),
        )
        .expect("String write는 실패하지 않음");
    }
    output.push_str("</ul></section>");
}

fn validate_source_layouts(
    candidates: &BtxFamilyCandidateAnalysis,
    review: &BtxFamilyReviewAnalysis,
    layouts: &[BtxFamilyReviewLayout],
) -> Result<(), BtxFamilyReviewHtmlError> {
    if layouts.is_empty() {
        return Err(error("BTX 재판독 layout이 비었음"));
    }
    let mut candidate_ids = std::collections::BTreeSet::new();
    let mut family_counts = std::collections::BTreeMap::new();
    for layout in layouts {
        if !candidate_ids.insert(layout.candidate_id.as_str()) {
            return Err(error(format!(
                "BTX 재판독 candidate 중복: {}",
                layout.candidate_id
            )));
        }
        if layout.width == 0
            || layout.stored_height == 0
            || layout.stored_height > layout.atlas_height
        {
            return Err(error(format!(
                "BTX 재판독 layout 범위 오류: {}: {}x{}-in-{}",
                layout.candidate_id, layout.width, layout.stored_height, layout.atlas_height
            )));
        }
        let basis_matches = match layout.basis {
            BtxFamilyReviewLayoutBasis::ExactDescriptorSize => {
                layout.stored_height == layout.atlas_height
            }
            BtxFamilyReviewLayoutBasis::RowPaddedAtlas => {
                layout.stored_height < layout.atlas_height
            }
        };
        if !basis_matches {
            return Err(error(format!(
                "BTX 재판독 layout basis 불일치: {}",
                layout.candidate_id
            )));
        }
        let candidate = candidates
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == layout.candidate_id)
            .ok_or_else(|| {
                error(format!(
                    "BTX 재판독 candidate가 현재 원본 후보에 없음: {}",
                    layout.candidate_id
                ))
            })?;
        if candidate.family_key != layout.family_key {
            return Err(error(format!(
                "BTX 재판독 family key 불일치: {}",
                layout.candidate_id
            )));
        }
        let bytes = Layout {
            width: layout.width,
            height: layout.stored_height,
            format: candidate.format,
        }
        .byte_len()
        .map_err(|error| BtxFamilyReviewHtmlError(error.to_string()))?;
        if bytes != candidate.decompressed_bytes {
            return Err(error(format!(
                "BTX 재판독 저장 크기 불일치: {}: layout={}, source={}",
                layout.candidate_id, bytes, candidate.decompressed_bytes
            )));
        }
        *family_counts
            .entry(layout.family_key.as_str())
            .or_insert(0usize) += 1;
    }
    for (family_key, selected_members) in family_counts {
        let family = candidates
            .families
            .iter()
            .find(|family| family.family_key == family_key)
            .ok_or_else(|| error(format!("BTX 재판독 family가 없음: {family_key}")))?;
        if family.review_lane != BtxReviewLane::UiFontTextCandidate {
            return Err(error(format!(
                "BTX 재판독 family가 UI·font·text lane 밖임: {family_key}"
            )));
        }
        if selected_members != family.members {
            return Err(error(format!(
                "BTX 재판독 family 일부 구성원만 노출됨: {family_key}: selected={selected_members}, family={}",
                family.members
            )));
        }
        if !review
            .decisions
            .iter()
            .any(|decision| decision.family_key == family_key)
        {
            return Err(error(format!(
                "BTX 재판독 family 결정이 없음: {family_key}"
            )));
        }
    }
    Ok(())
}

fn render_decision_form(
    output: &mut String,
    decision: &BtxFamilyReviewDecision,
    manifest_sha256: &str,
    session_token: &str,
) {
    writeln!(
        output,
        "<form method=post action=/decide class=decision-form><input type=hidden name=token value=\"{}\"><input type=hidden name=expected_manifest_sha256 value=\"{}\"><input type=hidden name=family_key value=\"{}\"><input type=hidden name=baseline_sha256 value=\"{}\"><fieldset><legend>대표 관측</legend>",
        escape_html(session_token),
        escape_html(manifest_sha256),
        escape_html(&decision.family_key),
        escape_html(&decision.baseline_sha256),
    )
    .expect("String write는 실패하지 않음");
    for option in [
        BtxFamilyReviewDecisionKind::JapanesePixelsObserved,
        BtxFamilyReviewDecisionKind::NoJapaneseObservedInRepresentative,
        BtxFamilyReviewDecisionKind::ReviewInsufficient,
    ] {
        writeln!(
            output,
            "<label class=choice><input type=radio name=decision value=\"{}\"{} required><span><strong>{}</strong><small>{}</small></span></label>",
            option.label(),
            checked(decision.decision, option),
            escape_html(option.label()),
            escape_html(decision_description(option)),
        )
        .expect("String write는 실패하지 않음");
    }
    writeln!(
        output,
        "</fieldset><label>메모 (선택)<textarea name=notes>{}</textarea></label><button type=submit>이 관측 저장</button></form>",
        escape_html(&decision.notes),
    )
    .expect("String write는 실패하지 않음");
}

fn render_review_page(
    candidates: &BtxFamilyCandidateAnalysis,
    review: &BtxFamilyReviewAnalysis,
    session_token: &str,
    notice: Option<&str>,
    source_layouts: Option<&[BtxFamilyReviewLayout]>,
) -> Result<String, BtxFamilyReviewHtmlError> {
    if session_token.is_empty() {
        return Err(error("BTX family 검토 session token이 비었음"));
    }
    let mut families = candidates
        .families
        .iter()
        .filter(|family| family.review_lane == BtxReviewLane::UiFontTextCandidate)
        .collect::<Vec<_>>();
    families.sort_by_key(|family| family.family_id.as_str());
    if families.len() != review.decisions.len() {
        return Err(error(format!(
            "BTX family HTML 분모 불일치: candidates={}, review={}",
            families.len(),
            review.decisions.len()
        )));
    }
    if let Some(layouts) = source_layouts {
        validate_source_layouts(candidates, review, layouts)?;
        let family_keys = layouts
            .iter()
            .map(|layout| layout.family_key.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        families.retain(|family| family_keys.contains(family.family_key.as_str()));
    }

    let focused = source_layouts.is_some();
    let page_title = if focused {
        "BTX source-bound layout 재판독"
    } else {
        "BTX family 사람 관측"
    };
    let scope_badge = source_layouts
        .map(|layouts| {
            format!(
                "<span>재판독 family {}</span><span>재판독 candidate {}</span>",
                families.len(),
                layouts.len()
            )
        })
        .unwrap_or_default();
    let boundary = if focused {
        "이 BMP는 원본 descriptor 크기와 정적 setup에 결속한 저장 행 배열입니다. 실제 CLUT·setup 실행·VRAM upload·primitive draw·인게임 표시를 재현하지 않으며, 선택은 이 대표 관측에만 적용됩니다."
    } else {
        "이 BMP는 원본 palette index를 명도로 보존한 정적 검토 배열입니다. 실제 CLUT·consumer layout·인게임 표시를 재현하지 않으며, 대표 비관측을 family 전체의 비텍스트 판정으로 확대하지 않습니다."
    };

    let mut output = String::new();
    write!(
        output,
        "<!doctype html><html lang=ko><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>{}</title><style>
        :root{{color-scheme:dark;--bg:#101317;--panel:#1a2028;--line:#394553;--accent:#7fc8ff;--ok:#9be6b4;--warn:#ffd166}}
        *{{box-sizing:border-box}}body{{margin:0;background:var(--bg);color:#edf2f7;font:15px/1.5 system-ui,sans-serif}}header{{position:sticky;top:0;z-index:2;padding:12px 18px;background:#0b0e12;border-bottom:1px solid var(--line)}}h1{{font-size:20px;margin:0}}h2{{font-size:19px}}h3{{font-size:17px}}h4{{font-size:14px;margin:4px 0}}code{{overflow-wrap:anywhere}}.summary,.family{{max-width:1180px;margin:14px auto;padding:16px;background:var(--panel);border:1px solid var(--line);border-radius:10px}}.warning{{color:var(--warn)}}.notice,.layout-basis{{color:var(--ok)}}.badges{{display:flex;flex-wrap:wrap;gap:6px;margin:8px 0}}.badges span{{border:1px solid var(--line);border-radius:999px;padding:2px 8px;font-size:12px}}.representative{{display:grid;grid-template-columns:minmax(360px,1fr) minmax(300px,420px);gap:16px}}.asset{{min-width:0;padding:10px;background:#111820;border:1px solid var(--line);border-radius:8px}}img{{display:block;max-width:100%;height:auto;min-height:64px;background:#000;image-rendering:pixelated;border:1px solid #576678}}ul{{padding-left:20px}}details{{margin-top:12px}}.decision-form{{padding:12px;background:#121c25;border:1px solid #43627d;border-radius:8px}}fieldset{{border:0;padding:0;margin:0 0 10px}}.choice{{display:flex;gap:8px;padding:7px;border-bottom:1px solid #2c3743}}.choice small{{display:block;color:#b8c5d2}}textarea{{display:block;width:100%;min-height:86px;margin:4px 0 10px;padding:8px;background:#0d1117;color:#fff;border:1px solid #596579;border-radius:6px}}button{{padding:8px 12px;background:#155a87;color:white;border:1px solid #4ea8de;border-radius:6px;cursor:pointer}}.stop{{max-width:1180px;margin:14px auto 40px}}@media(max-width:780px){{.representative{{display:block}}.decision-form{{margin-top:12px}}}}
        </style></head><body><header><h1>{} · source-bound ledger</h1></header><section class=summary><h2>현재 상태</h2><div class=badges><span>전체 {}</span><span>대기 {}</span><span>일본어 관측 {}</span><span>대표 비관측 {}</span><span>판단 불충분 {}</span>{}</div><p><code>{}</code></p><p class=warning>{}</p>{}</section>",
        escape_html(page_title),
        escape_html(page_title),
        review.families,
        review.pending_human_review,
        review.japanese_pixels_observed,
        review.no_japanese_observed_in_representative,
        review.review_insufficient,
        scope_badge,
        escape_html(&review.manifest_sha256),
        escape_html(boundary),
        notice
            .map(|notice| format!("<p class=notice>{}</p>", escape_html(notice)))
            .unwrap_or_default(),
    )
    .expect("String write는 실패하지 않음");

    for (index, family) in families.into_iter().enumerate() {
        let decision = review
            .decisions
            .iter()
            .find(|decision| decision.family_key == family.family_key)
            .ok_or_else(|| error(format!("{} 원장 결정이 없음", family.family_id)))?;
        let members = family_members(candidates, family)?;
        let representative = members
            .iter()
            .find(|candidate| candidate.candidate_id == family.representative_candidate_id)
            .ok_or_else(|| error(format!("{} 대표 candidate가 없음", family.family_id)))?;
        writeln!(
            output,
            "<article class=family id=\"{}\"><h2>{}. {}</h2><div class=badges><span>{}</span><span>confidence {}</span><span>고유 구성원 {}</span><span>물리 사본 {}</span><span>{}</span></div><p><strong>family key:</strong> <code>{}</code></p><p><strong>현재 관측:</strong> {}</p><div class=representative><div>",
            escape_html(&family.family_id),
            index + 1,
            escape_html(&family.family_id),
            escape_html(family.review_lane.as_str()),
            escape_html(family.confidence.as_str()),
            family.members,
            family.physical_copies,
            escape_html(decision.decision.label()),
            escape_html(&family.family_key),
            escape_html(decision_description(decision.decision)),
        )
        .expect("String write는 실패하지 않음");
        let representative_layout = source_layouts.and_then(|layouts| {
            layouts
                .iter()
                .find(|layout| layout.candidate_id == representative.candidate_id)
        });
        render_candidate(&mut output, representative, true, representative_layout);
        output.push_str("</div>");
        render_decision_form(
            &mut output,
            decision,
            &review.manifest_sha256,
            session_token,
        );
        output.push_str("</div>");
        if members.len() > 1 {
            output.push_str(if focused {
                "<details open><summary>같은 결정에 포함되는 나머지 family 구성원</summary>"
            } else {
                "<details><summary>일본어 양성일 때 확인할 family 구성원 전체</summary>"
            });
            for member in members {
                if focused && member.candidate_id == family.representative_candidate_id {
                    continue;
                }
                let member_layout = source_layouts.and_then(|layouts| {
                    layouts
                        .iter()
                        .find(|layout| layout.candidate_id == member.candidate_id)
                });
                render_candidate(
                    &mut output,
                    member,
                    member.candidate_id == family.representative_candidate_id,
                    member_layout,
                );
            }
            output.push_str("</details>");
        }
        output.push_str("</article>");
    }
    writeln!(
        output,
        "<form class=stop method=post action=/stop><input type=hidden name=token value=\"{}\"><button type=submit>검토 server 종료</button></form></body></html>",
        escape_html(session_token),
    )
    .expect("String write는 실패하지 않음");
    Ok(output)
}

/// 모든 UI·font·text 후보 family의 원본 결속 검토 화면을 만든다.
pub fn render_btx_family_review_page(
    candidates: &BtxFamilyCandidateAnalysis,
    review: &BtxFamilyReviewAnalysis,
    session_token: &str,
    notice: Option<&str>,
) -> Result<String, BtxFamilyReviewHtmlError> {
    render_review_page(candidates, review, session_token, notice, None)
}

/// 정적 소비자 layout이 닫힌 판단 불충분 family만 재판독 화면에 올린다.
pub fn render_btx_family_layout_review_page(
    candidates: &BtxFamilyCandidateAnalysis,
    review: &BtxFamilyReviewAnalysis,
    layouts: &[BtxFamilyReviewLayout],
    session_token: &str,
    notice: Option<&str>,
) -> Result<String, BtxFamilyReviewHtmlError> {
    render_review_page(candidates, review, session_token, notice, Some(layouts))
}

#[cfg(test)]
#[path = "btx_family_review_html_tests.rs"]
mod tests;
