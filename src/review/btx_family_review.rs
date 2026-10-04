//! 미분류 BTX family 대표의 사람 관측을 원본 결속 원장으로 관리한다.
//!
//! `ui-font-text-candidate`는 자동 분류 결과일 뿐이다. 이 원장은 대표에서
//! 일본어 픽셀이 보였는지 기록하며, `보이지 않음`을 family 전체의 비텍스트
//! 판정으로 확대하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::btx::PixelFormat;
use crate::graphics::btx_family_candidates::{
    BtxCandidateFamily, BtxFamilyCandidate, BtxFamilyCandidateAnalysis, BtxReviewLane,
};

pub const BTX_FAMILY_REVIEW_DECISION_SCHEMA_VERSION: &str = "btx-family-review-decision-v2";

const HEADER: &str = "schema_version\tfamily_key\tbaseline_sha256\tdecision\tnotes";
const MANIFEST: &str = include_str!("../../assets/reviews/btx_family_decisions.tsv");

/// 대표 이미지에서 사람이 직접 남긴 관측 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtxFamilyReviewDecisionKind {
    PendingHumanReview,
    JapanesePixelsObserved,
    NoJapaneseObservedInRepresentative,
    ReviewInsufficient,
}

impl BtxFamilyReviewDecisionKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PendingHumanReview => "pending-human-review",
            Self::JapanesePixelsObserved => "japanese-pixels-observed",
            Self::NoJapaneseObservedInRepresentative => "no-japanese-observed-in-representative",
            Self::ReviewInsufficient => "review-insufficient",
        }
    }

    fn parse(value: &str, line: usize) -> Result<Self, BtxFamilyReviewError> {
        match value {
            "pending-human-review" => Ok(Self::PendingHumanReview),
            "japanese-pixels-observed" => Ok(Self::JapanesePixelsObserved),
            "no-japanese-observed-in-representative" => {
                Ok(Self::NoJapaneseObservedInRepresentative)
            }
            "review-insufficient" => Ok(Self::ReviewInsufficient),
            _ => Err(error(format!(
                "{line}행 BTX family 관측 상태를 지원하지 않음: {value}"
            ))),
        }
    }
}

/// 한 family 대표에 결속된 사람 관측.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyReviewDecision {
    pub family_key: String,
    pub baseline_sha256: String,
    pub decision: BtxFamilyReviewDecisionKind,
    pub notes: String,
}

/// 현재 source-derived family와 결속한 관측 기준선.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyReviewBaseline {
    pub family_key: String,
    pub baseline_sha256: String,
}

/// 현재 원장 전수 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyReviewAnalysis {
    pub decisions: Vec<BtxFamilyReviewDecision>,
    pub families: usize,
    pub pending_human_review: usize,
    pub japanese_pixels_observed: usize,
    pub no_japanese_observed_in_representative: usize,
    pub review_insufficient: usize,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyReviewError(String);

impl fmt::Display for BtxFamilyReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "BTX family 검토 원장 실패: {}", self.0)
    }
}

impl Error for BtxFamilyReviewError {}

fn error(message: impl Into<String>) -> BtxFamilyReviewError {
    BtxFamilyReviewError(message.into())
}

fn pixel_format_label(format: PixelFormat) -> &'static str {
    match format {
        PixelFormat::Indexed4 => "indexed4",
        PixelFormat::Indexed8 => "indexed8",
    }
}

fn append_candidate_baseline(output: &mut String, candidate: &BtxFamilyCandidate) {
    writeln!(
        output,
        "candidate\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        candidate.decompressed_sha256,
        pixel_format_label(candidate.format),
        candidate.review_width,
        candidate.review_height,
        candidate.candidate_background_index,
        candidate.physical_copies.len(),
        candidate.dimension_candidates,
    )
    .expect("String write는 실패하지 않음");
    for copy in &candidate.physical_copies {
        writeln!(
            output,
            "copy\t{}\t{}\t{}\t{}\t{}\t0x{:08X}",
            copy.archive,
            copy.name,
            copy.resource_index,
            copy.static_evidence,
            copy.static_reference_sites,
            copy.descriptor_runtime,
        )
        .expect("String write는 실패하지 않음");
    }
}

fn family_baseline_sha256(
    family: &BtxCandidateFamily,
    candidates: &[BtxFamilyCandidate],
) -> Result<String, BtxFamilyReviewError> {
    let mut members = candidates
        .iter()
        .filter(|candidate| candidate.family_key == family.family_key)
        .collect::<Vec<_>>();
    members.sort_by_key(|candidate| candidate.decompressed_sha256.as_str());
    if members.len() != family.members {
        return Err(error(format!(
            "{} family member 분모 불일치: family={}, candidates={}",
            family.family_key,
            family.members,
            members.len()
        )));
    }
    let representative = members
        .iter()
        .find(|candidate| candidate.candidate_id == family.representative_candidate_id)
        .ok_or_else(|| {
            error(format!(
                "{} 대표 candidate가 family 구성원에 없음: {}",
                family.family_key, family.representative_candidate_id
            ))
        })?;
    if representative.family_key != family.family_key {
        return Err(error(format!(
            "{} 대표 candidate의 family key 불일치",
            family.family_key
        )));
    }

    let mut baseline = String::new();
    writeln!(
        baseline,
        "family\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        family.family_key,
        family.review_lane.as_str(),
        family.confidence.as_str(),
        family.members,
        family.physical_copies,
        representative.decompressed_sha256,
        family.representative_archive,
        family.representative_name,
    )
    .expect("String write는 실패하지 않음");
    for candidate in members {
        append_candidate_baseline(&mut baseline, candidate);
    }
    Ok(format!("{:x}", Sha256::digest(baseline.as_bytes())))
}

/// 사람 검토 대상인 UI·font·text 후보 family의 현재 기준선을 계산한다.
pub fn btx_family_review_baselines(
    candidates: &BtxFamilyCandidateAnalysis,
) -> Result<Vec<BtxFamilyReviewBaseline>, BtxFamilyReviewError> {
    let mut families = candidates
        .families
        .iter()
        .filter(|family| family.review_lane == BtxReviewLane::UiFontTextCandidate)
        .collect::<Vec<_>>();
    families.sort_by_key(|family| family.family_key.as_str());
    if families.is_empty() {
        return Err(error("UI·font·text 후보 family가 없음"));
    }
    families
        .into_iter()
        .map(|family| {
            Ok(BtxFamilyReviewBaseline {
                family_key: family.family_key.clone(),
                baseline_sha256: family_baseline_sha256(family, &candidates.candidates)?,
            })
        })
        .collect()
}

fn escape_tsv(value: &str) -> String {
    if value.is_empty() {
        return "\\e".to_owned();
    }
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn unescape_tsv(value: &str, line: usize) -> Result<String, BtxFamilyReviewError> {
    if value == "\\e" {
        return Ok(String::new());
    }
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = characters
            .next()
            .ok_or_else(|| error(format!("{line}행 TSV escape가 끝에서 잘림")))?;
        match escaped {
            '\\' => output.push('\\'),
            't' => output.push('\t'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            _ => {
                return Err(error(format!(
                    "{line}행 알 수 없는 TSV escape: \\{escaped}"
                )))
            }
        }
    }
    Ok(output)
}

fn parse_manifest(raw: &str) -> Result<Vec<BtxFamilyReviewDecision>, BtxFamilyReviewError> {
    if !raw.ends_with('\n') || raw.contains('\r') {
        return Err(error("원장은 LF로 끝나야 하며 CR을 포함할 수 없음"));
    }
    let mut lines = raw.lines();
    if lines.next() != Some(HEADER) {
        return Err(error("원장 header 불일치"));
    }
    lines
        .enumerate()
        .map(|(index, row)| {
            let line = index + 2;
            let fields = row.split('\t').collect::<Vec<_>>();
            let [schema, family_key, baseline_sha256, decision, notes] = fields.as_slice() else {
                return Err(error(format!("{line}행 field 수 불일치")));
            };
            if *schema != BTX_FAMILY_REVIEW_DECISION_SCHEMA_VERSION {
                return Err(error(format!("{line}행 schema 불일치")));
            }
            if family_key.is_empty()
                || baseline_sha256.len() != 64
                || !baseline_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(error(format!(
                    "{line}행 family key 또는 baseline 형식 오류"
                )));
            }
            let decision = BtxFamilyReviewDecisionKind::parse(decision, line)?;
            let notes = unescape_tsv(notes, line)?;
            match decision {
                BtxFamilyReviewDecisionKind::PendingHumanReview => {
                    if !notes.is_empty() {
                        return Err(error(format!("{line}행 미결정 상태에 notes가 있음")));
                    }
                }
                _ => {
                    if !notes.is_empty() && notes.trim() != notes {
                        return Err(error(format!("{line}행 선택 메모 앞뒤에 공백이 있음")));
                    }
                }
            }
            Ok(BtxFamilyReviewDecision {
                family_key: (*family_key).to_owned(),
                baseline_sha256: baseline_sha256.to_ascii_lowercase(),
                decision,
                notes,
            })
        })
        .collect()
}

/// 결정 원장을 stable family key 순서의 canonical TSV로 직렬화한다.
pub fn render_btx_family_review_decisions_tsv(decisions: &[BtxFamilyReviewDecision]) -> String {
    let mut decisions = decisions.to_vec();
    decisions.sort_by(|left, right| left.family_key.cmp(&right.family_key));
    let mut output = format!("{HEADER}\n");
    for decision in decisions {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}",
            BTX_FAMILY_REVIEW_DECISION_SCHEMA_VERSION,
            decision.family_key,
            decision.baseline_sha256,
            decision.decision.label(),
            escape_tsv(&decision.notes),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// 현재 source-derived 대상 전체를 미결정 상태로 둔 원장 초안을 만든다.
pub fn render_pending_btx_family_review_decisions_tsv(
    candidates: &BtxFamilyCandidateAnalysis,
) -> Result<String, BtxFamilyReviewError> {
    let decisions = btx_family_review_baselines(candidates)?
        .into_iter()
        .map(|baseline| BtxFamilyReviewDecision {
            family_key: baseline.family_key,
            baseline_sha256: baseline.baseline_sha256,
            decision: BtxFamilyReviewDecisionKind::PendingHumanReview,
            notes: String::new(),
        })
        .collect::<Vec<_>>();
    Ok(render_btx_family_review_decisions_tsv(&decisions))
}

/// 원장의 canonical 형식·분모·family baseline·사람 관측 필드를 검증한다.
pub fn audit_btx_family_review_decisions(
    candidates: &BtxFamilyCandidateAnalysis,
    raw: &str,
) -> Result<BtxFamilyReviewAnalysis, BtxFamilyReviewError> {
    let decisions = parse_manifest(raw)?;
    if render_btx_family_review_decisions_tsv(&decisions) != raw {
        return Err(error("원장이 family key canonical 순서/표현이 아님"));
    }
    let baselines = btx_family_review_baselines(candidates)?
        .into_iter()
        .map(|baseline| (baseline.family_key, baseline.baseline_sha256))
        .collect::<BTreeMap<_, _>>();
    if decisions.len() != baselines.len() {
        return Err(error(format!(
            "검토 family 분모 불일치: manifest={}, source={}",
            decisions.len(),
            baselines.len()
        )));
    }
    let mut keys = BTreeSet::new();
    for decision in &decisions {
        if !keys.insert(decision.family_key.as_str()) {
            return Err(error(format!("family 결정 중복: {}", decision.family_key)));
        }
        let expected = baselines.get(&decision.family_key).ok_or_else(|| {
            error(format!(
                "현재 UI·font·text 후보에 없는 family 결정: {}",
                decision.family_key
            ))
        })?;
        if &decision.baseline_sha256 != expected {
            return Err(error(format!(
                "{} family 검토 baseline drift: expected={expected}, actual={}",
                decision.family_key, decision.baseline_sha256
            )));
        }
    }
    if keys
        != baselines
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
    {
        return Err(error("현재 UI·font·text 후보 family 결정이 누락됨"));
    }

    let mut pending_human_review = 0;
    let mut japanese_pixels_observed = 0;
    let mut no_japanese_observed_in_representative = 0;
    let mut review_insufficient = 0;
    for decision in &decisions {
        match decision.decision {
            BtxFamilyReviewDecisionKind::PendingHumanReview => pending_human_review += 1,
            BtxFamilyReviewDecisionKind::JapanesePixelsObserved => japanese_pixels_observed += 1,
            BtxFamilyReviewDecisionKind::NoJapaneseObservedInRepresentative => {
                no_japanese_observed_in_representative += 1
            }
            BtxFamilyReviewDecisionKind::ReviewInsufficient => review_insufficient += 1,
        }
    }
    Ok(BtxFamilyReviewAnalysis {
        families: decisions.len(),
        pending_human_review,
        japanese_pixels_observed,
        no_japanese_observed_in_representative,
        review_insufficient,
        manifest_sha256: format!("{:x}", Sha256::digest(raw.as_bytes())),
        decisions,
    })
}

/// version 관리된 현재 BTX family 사람 검토 원장을 검증한다.
pub fn verify_box_btx_family_review_decisions(
    candidates: &BtxFamilyCandidateAnalysis,
) -> Result<BtxFamilyReviewAnalysis, BtxFamilyReviewError> {
    audit_btx_family_review_decisions(candidates, MANIFEST)
}

#[cfg(test)]
#[path = "btx_family_review_tests.rs"]
mod tests;
