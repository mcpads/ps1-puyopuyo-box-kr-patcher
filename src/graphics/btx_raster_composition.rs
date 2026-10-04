//! 여러 BTX storage block으로 나뉜 한 문구의 승인 경계.
//!
//! 정적 cell 순서와 언어적인 연결은 runtime draw 순서를 증명하지 않는다.
//! 따라서 조립 단위가 승인되기 전에는 구성 block도 제품 승인할 수 없다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use super::btx_raster_translation_manifest::{
    general_btx_raster_translation_entries, GeneralRasterManifestError,
    GeneralRasterTranslationEntry, GeneralRasterTranslationStatus,
};
use super::text_catalog::is_japanese_translation_target;

pub const BTX_RASTER_COMPOSITION_SCHEMA_VERSION: &str = "btx-raster-composition-unit-v1";

const MANIFEST: &str = include_str!("../../assets/translations/btx/raster_composition_units.tsv");
const HEADER: &str = "schema_version\tunit_id\tsemantic_family\tasset_name\t\
                      member_block_ids\tprotected_source_fragments\t\
                      protected_composed_source\tcomposition_evidence\t\
                      consumer_order_status\tko_composed\tstatus";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositionEvidence {
    StaticCellLinguisticFragment,
    StaticCellLogoMembership,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConsumerOrderStatus {
    RuntimeOrderUnverified,
    RuntimeOrderVerified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RasterCompositionUnit {
    unit_id: String,
    semantic_family: String,
    asset_name: String,
    member_block_ids: Vec<String>,
    protected_source_fragments: Vec<String>,
    protected_composed_source: String,
    consumer_order_status: ConsumerOrderStatus,
    ko_composed: String,
    status: GeneralRasterTranslationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterCompositionReport {
    pub units: usize,
    pub member_blocks: usize,
    pub pending_units: usize,
    pub draft_units: usize,
    pub approved_units: usize,
    pub runtime_order_verified_units: usize,
    pub product_eligible_units: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterCompositionError(String);

impl fmt::Display for RasterCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "raster 조립 단위 manifest 실패: {}", self.0)
    }
}

impl Error for RasterCompositionError {}

impl From<GeneralRasterManifestError> for RasterCompositionError {
    fn from(source: GeneralRasterManifestError) -> Self {
        Self(format!("일반 raster manifest 검증 실패: {source}"))
    }
}

fn error(message: impl Into<String>) -> RasterCompositionError {
    RasterCompositionError(message.into())
}

fn unescape_tsv_field(value: &str, line: usize) -> Result<String, RasterCompositionError> {
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

fn parse_evidence(value: &str, line: usize) -> Result<CompositionEvidence, RasterCompositionError> {
    match value {
        "static-cell-linguistic-fragment" => Ok(CompositionEvidence::StaticCellLinguisticFragment),
        "static-cell-logo-membership" => Ok(CompositionEvidence::StaticCellLogoMembership),
        _ => Err(error(format!(
            "{line}행 composition evidence 미지원: {value}"
        ))),
    }
}

fn parse_consumer_order(
    value: &str,
    line: usize,
) -> Result<ConsumerOrderStatus, RasterCompositionError> {
    match value {
        "runtime-order-unverified" => Ok(ConsumerOrderStatus::RuntimeOrderUnverified),
        "runtime-order-verified" => Ok(ConsumerOrderStatus::RuntimeOrderVerified),
        _ => Err(error(format!(
            "{line}행 consumer order 상태 미지원: {value}"
        ))),
    }
}

fn parse_translation_status(
    value: &str,
    line: usize,
) -> Result<GeneralRasterTranslationStatus, RasterCompositionError> {
    match value {
        "pending-human-translation" => Ok(GeneralRasterTranslationStatus::PendingHumanTranslation),
        "draft-human-review-required" => {
            Ok(GeneralRasterTranslationStatus::DraftHumanReviewRequired)
        }
        "approved" => Ok(GeneralRasterTranslationStatus::Approved),
        _ => Err(error(format!("{line}행 번역 상태 미지원: {value}"))),
    }
}

fn has_hangul_syllable(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

fn without_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn parse_manifest(raw: &str) -> Result<Vec<RasterCompositionUnit>, RasterCompositionError> {
    if !raw.ends_with('\n') || raw.contains('\r') {
        return Err(error("manifest는 LF로 끝나야 하며 CR을 포함할 수 없음"));
    }
    if raw.lines().next() != Some(HEADER) {
        return Err(error("manifest header 불일치"));
    }
    raw.lines()
        .skip(1)
        .enumerate()
        .map(|(index, row)| {
            let line = index + 2;
            let fields = row.split('\t').collect::<Vec<_>>();
            let [schema, unit_id, family, asset, block_ids, source_fragments, composed_source, evidence, consumer_order, ko_composed, status] =
                fields.as_slice()
            else {
                return Err(error(format!("{line}행 field 수 불일치")));
            };
            if *schema != BTX_RASTER_COMPOSITION_SCHEMA_VERSION {
                return Err(error(format!("{line}행 schema 불일치")));
            }
            parse_evidence(evidence, line)?;
            let unit = RasterCompositionUnit {
                unit_id: (*unit_id).to_string(),
                semantic_family: (*family).to_string(),
                asset_name: (*asset).to_string(),
                member_block_ids: unescape_tsv_field(block_ids, line)?
                    .split('\n')
                    .map(str::to_string)
                    .collect(),
                protected_source_fragments: unescape_tsv_field(source_fragments, line)?
                    .split('\n')
                    .map(str::to_string)
                    .collect(),
                protected_composed_source: unescape_tsv_field(composed_source, line)?,
                consumer_order_status: parse_consumer_order(consumer_order, line)?,
                ko_composed: unescape_tsv_field(ko_composed, line)?,
                status: parse_translation_status(status, line)?,
            };
            if unit.unit_id.is_empty()
                || unit.semantic_family.is_empty()
                || unit.asset_name.is_empty()
                || unit.member_block_ids.len() < 2
                || unit.member_block_ids.len() != unit.protected_source_fragments.len()
                || unit
                    .member_block_ids
                    .iter()
                    .chain(unit.protected_source_fragments.iter())
                    .any(String::is_empty)
            {
                return Err(error(format!("{line}행 조립 단위 필수 필드 불일치")));
            }
            if unit.protected_source_fragments.concat() != unit.protected_composed_source
                || !is_japanese_translation_target(&unit.protected_composed_source)
            {
                return Err(error(format!("{line}행 보호 조립 원문 불일치")));
            }
            match unit.status {
                GeneralRasterTranslationStatus::PendingHumanTranslation
                    if !unit.ko_composed.is_empty() =>
                {
                    return Err(error(format!("{line}행 대기 단위에 한국어가 있음")));
                }
                GeneralRasterTranslationStatus::DraftHumanReviewRequired
                | GeneralRasterTranslationStatus::Approved
                    if unit.ko_composed.is_empty()
                        || unit.ko_composed.trim() != unit.ko_composed
                        || !has_hangul_syllable(&unit.ko_composed)
                        || is_japanese_translation_target(&unit.ko_composed) =>
                {
                    return Err(error(format!("{line}행 조립 번역문 내용 계약 불일치")));
                }
                _ => {}
            }
            if unit.status == GeneralRasterTranslationStatus::Approved
                && unit.consumer_order_status != ConsumerOrderStatus::RuntimeOrderVerified
            {
                return Err(error(format!(
                    "{line}행 runtime 순서 미검증 단위가 승인됨"
                )));
            }
            Ok(unit)
        })
        .collect()
}

fn verify_manifest(
    raw: &str,
    translations: &[GeneralRasterTranslationEntry],
) -> Result<RasterCompositionReport, RasterCompositionError> {
    let units = parse_manifest(raw)?;
    let translations = translations
        .iter()
        .map(|entry| (entry.block_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut unit_ids = BTreeSet::new();
    let mut member_ids = BTreeSet::new();
    let mut previous_unit_id = None;
    let mut pending_units = 0usize;
    let mut draft_units = 0usize;
    let mut approved_units = 0usize;
    let mut runtime_order_verified_units = 0usize;
    let mut product_eligible_units = 0usize;

    for unit in &units {
        if previous_unit_id.is_some_and(|previous| previous >= unit.unit_id.as_str()) {
            return Err(error("manifest가 unit ID canonical 순서가 아님"));
        }
        previous_unit_id = Some(unit.unit_id.as_str());
        if !unit_ids.insert(unit.unit_id.as_str()) {
            return Err(error(format!("조립 unit ID 중복: {}", unit.unit_id)));
        }
        let mut member_ko = String::new();
        for (block_id, source_fragment) in unit
            .member_block_ids
            .iter()
            .zip(&unit.protected_source_fragments)
        {
            if !member_ids.insert(block_id.as_str()) {
                return Err(error(format!("조립 member block 중복: {block_id}")));
            }
            let translation = translations
                .get(block_id.as_str())
                .ok_or_else(|| error(format!("일반 raster manifest에 없는 block: {block_id}")))?;
            if translation.semantic_family != unit.semantic_family
                || translation.asset_name != unit.asset_name
                || translation.protected_source != *source_fragment
            {
                return Err(error(format!("{block_id}의 조립 보호 필드 drift")));
            }
            if translation.status != unit.status {
                return Err(error(format!(
                    "{}의 조립 단위와 구성 block 상태 불일치: unit={}, block={}",
                    unit.unit_id,
                    unit.status.label(),
                    translation.status.label()
                )));
            }
            member_ko.push_str(&translation.ko);
        }
        if without_whitespace(&member_ko) != without_whitespace(&unit.ko_composed) {
            return Err(error(format!(
                "{}의 조립 번역문 불일치: members={member_ko:?}, composed={:?}",
                unit.unit_id, unit.ko_composed
            )));
        }

        match unit.status {
            GeneralRasterTranslationStatus::PendingHumanTranslation => pending_units += 1,
            GeneralRasterTranslationStatus::DraftHumanReviewRequired => draft_units += 1,
            GeneralRasterTranslationStatus::Approved => approved_units += 1,
            GeneralRasterTranslationStatus::ScopeExcludedProtectedNoWrite => {
                return Err(error(format!(
                    "{}의 조립 단위에는 제품 범위 제외 상태를 쓸 수 없음",
                    unit.unit_id
                )));
            }
        }
        if unit.consumer_order_status == ConsumerOrderStatus::RuntimeOrderVerified {
            runtime_order_verified_units += 1;
        }
        let product_eligible = unit.status == GeneralRasterTranslationStatus::Approved
            && unit.consumer_order_status == ConsumerOrderStatus::RuntimeOrderVerified;
        if product_eligible {
            product_eligible_units += 1;
        }
    }

    Ok(RasterCompositionReport {
        units: units.len(),
        member_blocks: member_ids.len(),
        pending_units,
        draft_units,
        approved_units,
        runtime_order_verified_units,
        product_eligible_units,
    })
}

/// 일반 raster 번역과 조립 단위의 보호 원문·상태·소비 순서를 함께 검증한다.
pub fn verify_btx_raster_composition_manifest(
) -> Result<RasterCompositionReport, RasterCompositionError> {
    let translations = general_btx_raster_translation_entries()?;
    verify_manifest(MANIFEST, &translations)
}

#[cfg(test)]
#[path = "btx_raster_composition_tests.rs"]
mod tests;
