//! 두 번째 QUEST 한글 표본과 이후 page 공급 선택지 판정.
//!
//! 첫 runtime 표본 `0x21CC`와 다음 표본 `0x22DC`는 같은 번역 batch다.
//! 독립 검토 후보의 batch 글리프 수요가 한 page에 들어가는지 먼저 판정해,
//! 두 번째 표본만을 위해 불필요한 세 번째 raw page를 도입하지 않는다.
//! 승인되지 않은 검토 후보는 용량 측정에만 쓰며 제품 입력으로 승격하지
//! 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::disasm::QuestPageCarrierSummary;
use crate::font::QUEST_FONT_WIDE_SLOTS;
use crate::script::{
    QuestTranslationBatchRole, QuestTranslationIndependentReviewStatus,
    QuestTranslationReviewAnalysis, QuestTranslationReviewRequirement, TOWN00_SEQ_RUNTIME_BASE,
};

use super::{QuestCapacityPocAnalysis, QuestFontAlternativesAnalysis};

pub const QUEST_PAGE_SUPPLY_PLAN_SCHEMA_VERSION: &str = "quest-page-supply-plan-v2";
pub const QUEST_PAGE_SUPPLY_PLAN_FIRST_STREAM_ID: &str = "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x21CC";
pub const QUEST_PAGE_SUPPLY_PLAN_TARGET_STREAM_ID: &str = "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x22DC";
pub const QUEST_PAGE_SUPPLY_PLAN_TARGET_RECORD_OFFSET: u32 = 0x22DC;
pub const QUEST_PAGE_SUPPLY_PLAN_TARGET_RUNTIME: u32 =
    TOWN00_SEQ_RUNTIME_BASE + QUEST_PAGE_SUPPLY_PLAN_TARGET_RECORD_OFFSET;
pub const BOX_QUEST_PAGE_SUPPLY_PLAN_ROWS: usize = 4;
pub const BOX_QUEST_PAGE_SUPPLY_PLAN_TARGET_HANGUL: usize = 35;
pub const BOX_QUEST_PAGE_SUPPLY_PLAN_BATCH_HANGUL: usize = 132;
pub const BOX_QUEST_PAGE_SUPPLY_PLAN_REPORT_SHA256: &str =
    "b4b6fe130a6fdd55bd1d5e88c255a8a878ebab2d0a9c2524495ceed6f6e9e018";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestPageSupplyMethod {
    SameBatchTwoRawPages,
    ThirdRawResidentPage,
    PlainCatalogAndActivePage,
    CnxCatalogAndActivePage,
}

impl QuestPageSupplyMethod {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SameBatchTwoRawPages => "same-batch-two-raw-pages",
            Self::ThirdRawResidentPage => "third-raw-resident-page",
            Self::PlainCatalogAndActivePage => "plain-catalog-plus-active-page",
            Self::CnxCatalogAndActivePage => "cnx-catalog-plus-active-page",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageSupplyMethodRow {
    pub method: QuestPageSupplyMethod,
    pub storage_bytes: usize,
    pub carrier_remaining_bytes: isize,
    pub storage_verdict: &'static str,
    pub lifecycle_verdict: &'static str,
    pub build_eligible: bool,
    pub notes: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageSupplyPlanAnalysis {
    pub schema_version: &'static str,
    pub first_stream_id: &'static str,
    pub target_stream_id: &'static str,
    pub target_runtime: u32,
    pub target_batch_id: String,
    pub target_review_status: QuestTranslationIndependentReviewStatus,
    pub target_translation_approved: bool,
    pub target_source_scalars: usize,
    pub target_reviewed_scalars: usize,
    pub target_fixed_slot_remaining: usize,
    pub target_unique_hangul: usize,
    pub draft_batch_unique_hangul: usize,
    pub active_wide_slots: usize,
    pub draft_batch_page_fits: bool,
    pub target_shares_first_batch: bool,
    pub next_target_requires_third_page: bool,
    pub raw_page_bytes: usize,
    pub carrier_capacity_bytes: usize,
    pub two_raw_pages_bytes: usize,
    pub two_raw_pages_remaining: usize,
    pub three_raw_pages_bytes: usize,
    pub three_raw_pages_shortfall: usize,
    pub glyph_catalog_plain_bytes: usize,
    pub glyph_catalog_cnx_bytes: usize,
    pub plain_catalog_active_page_shortfall: usize,
    pub cnx_catalog_active_page_remaining: usize,
    pub rows: Vec<QuestPageSupplyMethodRow>,
    pub build_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageSupplyPlanError(String);

impl fmt::Display for QuestPageSupplyPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestPageSupplyPlanError {}

fn error(message: impl Into<String>) -> QuestPageSupplyPlanError {
    QuestPageSupplyPlanError(message.into())
}

fn unique_hangul(text: &str) -> BTreeSet<char> {
    text.chars()
        .filter(|glyph| matches!(glyph, '\u{ac00}'..='\u{d7a3}'))
        .collect()
}

fn remaining(capacity: usize, required: usize) -> Result<isize, QuestPageSupplyPlanError> {
    let capacity = isize::try_from(capacity)
        .map_err(|_| error("QUEST page 공급 carrier 용량 변환 overflow"))?;
    let required =
        isize::try_from(required).map_err(|_| error("QUEST page 공급 저장량 변환 overflow"))?;
    Ok(capacity - required)
}

/// 검토 후보와 현재 carrier를 결합해 다음 page 공급 선택지를 판정한다.
pub fn audit_quest_page_supply_plan(
    reviews: &QuestTranslationReviewAnalysis,
    capacity: &QuestCapacityPocAnalysis,
    alternatives: &QuestFontAlternativesAnalysis,
    carrier: QuestPageCarrierSummary,
) -> Result<QuestPageSupplyPlanAnalysis, QuestPageSupplyPlanError> {
    if !carrier.static_unreferenced || carrier.runtime_ownership_proven || carrier.build_eligible {
        return Err(error(
            "QUEST page 공급 carrier의 정적 후보/runtime 미승인 경계가 바뀜",
        ));
    }
    let first = reviews
        .entries
        .iter()
        .find(|entry| entry.batch_entry.stream_id == QUEST_PAGE_SUPPLY_PLAN_FIRST_STREAM_ID)
        .ok_or_else(|| error("QUEST 첫 page 표본 review entry가 없음"))?;
    let target = reviews
        .entries
        .iter()
        .find(|entry| entry.batch_entry.stream_id == QUEST_PAGE_SUPPLY_PLAN_TARGET_STREAM_ID)
        .ok_or_else(|| error("QUEST 다음 page 표본 review entry가 없음"))?;
    for entry in [first, target] {
        if entry.requirement != QuestTranslationReviewRequirement::RuntimeSampleIndependentReview {
            return Err(error(format!(
                "{}: runtime 표본 독립 검토 경계가 아님",
                entry.batch_entry.stream_id
            )));
        }
    }
    if first.batch_entry.batch_id != target.batch_entry.batch_id {
        return Err(error("QUEST 첫 표본과 다음 표본의 번역 batch가 다름"));
    }
    let target_review_status = target
        .independent_review_status
        .ok_or_else(|| error("QUEST 다음 표본의 독립 검토 결과가 없음"))?;
    if target.reviewed_korean.is_empty() {
        return Err(error("QUEST 다음 표본의 검토 번역 후보가 비어 있음"));
    }

    let batch_entries = reviews
        .entries
        .iter()
        .filter(|entry| {
            entry.batch_entry.batch_id == target.batch_entry.batch_id
                && entry.batch_entry.role == QuestTranslationBatchRole::Translate
        })
        .collect::<Vec<_>>();
    if batch_entries.is_empty() {
        return Err(error("QUEST 다음 표본 batch의 번역 entry가 없음"));
    }
    if batch_entries
        .iter()
        .any(|entry| entry.batch_entry.korean_draft.is_empty())
    {
        return Err(error(
            "QUEST 다음 표본 batch의 초벌 번역 분모가 완성되지 않음",
        ));
    }
    let draft_batch_hangul = batch_entries
        .iter()
        .flat_map(|entry| unique_hangul(&entry.batch_entry.korean_draft))
        .collect::<BTreeSet<_>>();
    let capacity_row = capacity
        .batches
        .iter()
        .find(|row| row.batch_id == target.batch_entry.batch_id)
        .ok_or_else(|| error("QUEST 다음 표본 batch의 capacity row가 없음"))?;
    if capacity_row.fixed_slot_overflow_entries != 0 {
        return Err(error(
            "QUEST 다음 표본 batch의 초벌 fixed-slot overflow가 있음",
        ));
    }

    let target_source_scalars = target.batch_entry.protected_source.chars().count();
    let target_reviewed_scalars = target.reviewed_korean.chars().count();
    let target_fixed_slot_remaining = target_source_scalars
        .checked_sub(target_reviewed_scalars)
        .ok_or_else(|| error("QUEST 다음 표본의 검토 후보가 원본 scalar slot을 넘음"))?;
    let target_unique_hangul = unique_hangul(&target.reviewed_korean).len();
    let draft_batch_unique_hangul = draft_batch_hangul.len();
    let active_wide_slots = capacity.current_wide_slots;
    if active_wide_slots != alternatives.current_wide_slots
        || active_wide_slots != QUEST_FONT_WIDE_SLOTS
    {
        return Err(error("QUEST page 공급 active wide slot 분모 불일치"));
    }
    let draft_batch_page_fits = draft_batch_unique_hangul <= active_wide_slots;
    if !draft_batch_page_fits {
        return Err(error("QUEST 다음 표본 batch의 초벌 번역이 한 page를 넘음"));
    }

    let raw_page_bytes = alternatives
        .batch_page_plain_total
        .checked_div(alternatives.batch_pages)
        .ok_or_else(|| error("QUEST page 공급 batch page 수가 0임"))?;
    if raw_page_bytes.checked_mul(alternatives.batch_pages)
        != Some(alternatives.batch_page_plain_total)
    {
        return Err(error(
            "QUEST page 공급 raw page 크기가 batch 합계와 나누어지지 않음",
        ));
    }
    let two_raw_pages_bytes = raw_page_bytes
        .checked_mul(2)
        .ok_or_else(|| error("QUEST 두 raw page 크기 overflow"))?;
    let three_raw_pages_bytes = raw_page_bytes
        .checked_mul(3)
        .ok_or_else(|| error("QUEST 세 raw page 크기 overflow"))?;
    let two_raw_pages_remaining = carrier
        .capacity_bytes
        .checked_sub(two_raw_pages_bytes)
        .ok_or_else(|| error("QUEST carrier에 raw page 두 장이 들어가지 않음"))?;
    let three_raw_pages_shortfall = three_raw_pages_bytes
        .checked_sub(carrier.capacity_bytes)
        .ok_or_else(|| error("QUEST carrier에 raw page 세 장이 들어가 기존 판정과 충돌"))?;
    let plain_catalog_and_page = alternatives
        .glyph_catalog_plain_bytes
        .checked_add(raw_page_bytes)
        .ok_or_else(|| error("QUEST plain catalog와 active page 합계 overflow"))?;
    let plain_catalog_active_page_shortfall = plain_catalog_and_page
        .checked_sub(carrier.capacity_bytes)
        .ok_or_else(|| error("QUEST plain catalog와 active page가 carrier에 들어감"))?;
    let cnx_catalog_and_page = alternatives
        .glyph_catalog_cnx_bytes
        .checked_add(raw_page_bytes)
        .ok_or_else(|| error("QUEST CNX catalog와 active page 합계 overflow"))?;
    let cnx_catalog_active_page_remaining = carrier
        .capacity_bytes
        .checked_sub(cnx_catalog_and_page)
        .ok_or_else(|| error("QUEST CNX catalog와 active page가 carrier를 넘음"))?;

    let rows = vec![
        QuestPageSupplyMethodRow {
            method: QuestPageSupplyMethod::SameBatchTwoRawPages,
            storage_bytes: two_raw_pages_bytes,
            carrier_remaining_bytes: remaining(carrier.capacity_bytes, two_raw_pages_bytes)?,
            storage_verdict: "fits-current-carrier",
            lifecycle_verdict: "batch-page-build-and-record-mapping-open",
            build_eligible: false,
            notes: "0x21CC와 0x22DC는 같은 batch page를 공유하므로 다음 표본에 세 번째 page가 필요하지 않음",
        },
        QuestPageSupplyMethodRow {
            method: QuestPageSupplyMethod::ThirdRawResidentPage,
            storage_bytes: three_raw_pages_bytes,
            carrier_remaining_bytes: remaining(carrier.capacity_bytes, three_raw_pages_bytes)?,
            storage_verdict: "rejected-capacity",
            lifecycle_verdict: "not-reached",
            build_eligible: false,
            notes: "다른 Korean batch page를 동시에 상주시킬 때 현재 carrier가 부족함",
        },
        QuestPageSupplyMethodRow {
            method: QuestPageSupplyMethod::PlainCatalogAndActivePage,
            storage_bytes: plain_catalog_and_page,
            carrier_remaining_bytes: remaining(carrier.capacity_bytes, plain_catalog_and_page)?,
            storage_verdict: "rejected-capacity",
            lifecycle_verdict: "lookup-and-eviction-unproven",
            build_eligible: false,
            notes: "plain catalog와 32KiB active page는 함께 상주할 수 없음",
        },
        QuestPageSupplyMethodRow {
            method: QuestPageSupplyMethod::CnxCatalogAndActivePage,
            storage_bytes: cnx_catalog_and_page,
            carrier_remaining_bytes: remaining(carrier.capacity_bytes, cnx_catalog_and_page)?,
            storage_verdict: "fits-current-carrier",
            lifecycle_verdict: "decode-source-cache-lifetime-unproven",
            build_eligible: false,
            notes: "저장량은 맞지만 CNX 해제 source, miss upload, eviction과 window 수명을 새로 증명해야 함",
        },
    ];

    Ok(QuestPageSupplyPlanAnalysis {
        schema_version: QUEST_PAGE_SUPPLY_PLAN_SCHEMA_VERSION,
        first_stream_id: QUEST_PAGE_SUPPLY_PLAN_FIRST_STREAM_ID,
        target_stream_id: QUEST_PAGE_SUPPLY_PLAN_TARGET_STREAM_ID,
        target_runtime: QUEST_PAGE_SUPPLY_PLAN_TARGET_RUNTIME,
        target_batch_id: target.batch_entry.batch_id.clone(),
        target_review_status,
        target_translation_approved: false,
        target_source_scalars,
        target_reviewed_scalars,
        target_fixed_slot_remaining,
        target_unique_hangul,
        draft_batch_unique_hangul,
        active_wide_slots,
        draft_batch_page_fits,
        target_shares_first_batch: true,
        next_target_requires_third_page: false,
        raw_page_bytes,
        carrier_capacity_bytes: carrier.capacity_bytes,
        two_raw_pages_bytes,
        two_raw_pages_remaining,
        three_raw_pages_bytes,
        three_raw_pages_shortfall,
        glyph_catalog_plain_bytes: alternatives.glyph_catalog_plain_bytes,
        glyph_catalog_cnx_bytes: alternatives.glyph_catalog_cnx_bytes,
        plain_catalog_active_page_shortfall,
        cnx_catalog_active_page_remaining,
        rows,
        build_eligible: false,
    })
}

/// 다음 page 공급 판정의 결정적 TSV를 만든다.
pub fn render_quest_page_supply_plan_tsv(analysis: &QuestPageSupplyPlanAnalysis) -> String {
    let mut out = String::from(
        "schema_version\tmethod\ttarget_stream_id\ttarget_runtime\treview_status\t\
         target_hangul\tbatch_id\tbatch_hangul\tactive_slots\tstorage_bytes\t\
         carrier_capacity\tcarrier_remaining\tstorage_verdict\tlifecycle_verdict\t\
         translation_approved\tbuild_eligible\tnotes\n",
    );
    for row in &analysis.rows {
        writeln!(
            out,
            "{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            analysis.schema_version,
            row.method.label(),
            analysis.target_stream_id,
            analysis.target_runtime,
            analysis.target_review_status.label(),
            analysis.target_unique_hangul,
            analysis.target_batch_id,
            analysis.draft_batch_unique_hangul,
            analysis.active_wide_slots,
            row.storage_bytes,
            analysis.carrier_capacity_bytes,
            row.carrier_remaining_bytes,
            row.storage_verdict,
            row.lifecycle_verdict,
            analysis.target_translation_approved,
            row.build_eligible,
            row.notes,
        )
        .expect("String write는 실패하지 않음");
    }
    out
}

/// 지원 원본에서 파생한 다음 page 공급 수치와 report를 고정한다.
pub fn verify_box_quest_page_supply_plan(
    reviews: &QuestTranslationReviewAnalysis,
    capacity: &QuestCapacityPocAnalysis,
    alternatives: &QuestFontAlternativesAnalysis,
    carrier: QuestPageCarrierSummary,
) -> Result<QuestPageSupplyPlanAnalysis, QuestPageSupplyPlanError> {
    let analysis = audit_quest_page_supply_plan(reviews, capacity, alternatives, carrier)?;
    if analysis.target_review_status
        != QuestTranslationIndependentReviewStatus::ReviewProposesChange
        || analysis.target_source_scalars != 63
        || analysis.target_reviewed_scalars != 57
        || analysis.target_fixed_slot_remaining != 6
        || analysis.target_unique_hangul != BOX_QUEST_PAGE_SUPPLY_PLAN_TARGET_HANGUL
        || analysis.draft_batch_unique_hangul != BOX_QUEST_PAGE_SUPPLY_PLAN_BATCH_HANGUL
        || analysis.active_wide_slots != 326
        || !analysis.draft_batch_page_fits
        || !analysis.target_shares_first_batch
        || analysis.next_target_requires_third_page
        || analysis.raw_page_bytes != 32_768
        || analysis.carrier_capacity_bytes != 67_592
        || analysis.two_raw_pages_bytes != 65_536
        || analysis.two_raw_pages_remaining != 2_056
        || analysis.three_raw_pages_bytes != 98_304
        || analysis.three_raw_pages_shortfall != 30_712
        || analysis.glyph_catalog_plain_bytes != 63_648
        || analysis.glyph_catalog_cnx_bytes != 33_493
        || analysis.plain_catalog_active_page_shortfall != 28_824
        || analysis.cnx_catalog_active_page_remaining != 1_331
        || analysis.rows.len() != BOX_QUEST_PAGE_SUPPLY_PLAN_ROWS
        || analysis.target_translation_approved
        || analysis.build_eligible
    {
        return Err(error(format!(
            "QUEST 다음 page 공급 source gate drift: {analysis:?}"
        )));
    }
    let report = render_quest_page_supply_plan_tsv(&analysis);
    let columns = report
        .lines()
        .map(|line| line.split('\t').count())
        .collect::<BTreeSet<_>>();
    if report.lines().count() != BOX_QUEST_PAGE_SUPPLY_PLAN_ROWS + 1 || columns.len() != 1 {
        return Err(error(format!(
            "QUEST 다음 page 공급 TSV shape 불일치: rows={}, columns={columns:?}",
            report.lines().count()
        )));
    }
    let actual_digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if actual_digest != BOX_QUEST_PAGE_SUPPLY_PLAN_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST 다음 page 공급 TSV SHA-256 불일치: expected={}, actual={actual_digest}",
            BOX_QUEST_PAGE_SUPPLY_PLAN_REPORT_SHA256
        )));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_page_supply_plan_tests.rs"]
mod tests;
