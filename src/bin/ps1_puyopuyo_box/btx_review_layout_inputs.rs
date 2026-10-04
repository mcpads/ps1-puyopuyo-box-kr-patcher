//! BTX source-bound layout 보고서와 사람 재판독 서버가 공유하는 원본 입력.

use std::path::Path;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::disasm::{
    verify_box_btx_descriptor_selections, verify_box_btx_name_consumers,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::pipeline::{
    audit_btx_review_layout_evidence, BtxReviewLayoutEvidenceAnalysis, BtxReviewLayoutResolution,
};
use ps1_puyopuyo_box::review::{
    verify_box_btx_family_review_decisions, BtxFamilyReviewLayout, BtxFamilyReviewLayoutBasis,
};
use ps1_puyopuyo_box::script::{
    verify_box_subgame_seq_layouts, verify_box_subgame_seq_text_candidates,
    BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS,
};

use super::analysis_inputs::read_subgame_seq_sources;
use super::btx_reports::{read_btx_family_source_inputs_from, BtxFamilySourceInputs};
use super::disc_commands::read_box_executables;

pub(super) struct BtxReviewLayoutSourceInputs {
    pub family: BtxFamilySourceInputs,
    pub evidence: BtxReviewLayoutEvidenceAnalysis,
    pub review_layouts: Vec<BtxFamilyReviewLayout>,
}

fn source_bound_review_layouts(
    evidence: &BtxReviewLayoutEvidenceAnalysis,
) -> Result<Vec<BtxFamilyReviewLayout>, String> {
    let mut layouts = Vec::new();
    for candidate in &evidence.candidates {
        let basis = match candidate.resolution {
            BtxReviewLayoutResolution::ExactAcrossCopies => {
                BtxFamilyReviewLayoutBasis::ExactDescriptorSize
            }
            BtxReviewLayoutResolution::RowPaddedAcrossCopies => {
                BtxFamilyReviewLayoutBasis::RowPaddedAtlas
            }
            BtxReviewLayoutResolution::PartialAcrossCopies
            | BtxReviewLayoutResolution::MultipleLayouts
            | BtxReviewLayoutResolution::NoCompatibleSetup => continue,
        };
        let [layout] = candidate.descriptor_storage_layouts.as_slice() else {
            return Err(format!(
                "BTX 재판독 후보의 source-bound layout이 하나가 아님: {}: {}",
                candidate.candidate_id,
                candidate.descriptor_storage_layouts.len()
            ));
        };
        if candidate.copies_with_descriptor_compatible_setup != candidate.physical_copies {
            return Err(format!(
                "BTX 재판독 후보의 물리 사본 layout 근거가 불완전함: {}: {}/{}",
                candidate.candidate_id,
                candidate.copies_with_descriptor_compatible_setup,
                candidate.physical_copies
            ));
        }
        layouts.push(BtxFamilyReviewLayout {
            family_key: candidate.family_key.clone(),
            candidate_id: candidate.candidate_id.clone(),
            width: usize::from(layout.width),
            stored_height: usize::from(layout.stored_height),
            atlas_height: usize::from(layout.atlas_height),
            basis,
        });
    }
    layouts.sort_by(|left, right| {
        left.family_key
            .cmp(&right.family_key)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    if layouts.is_empty() {
        return Err("BTX 재판독 가능한 source-bound layout이 없음".to_owned());
    }
    Ok(layouts)
}

pub(super) fn read_btx_review_layout_source_inputs(
    source_path: &Path,
) -> Result<BtxReviewLayoutSourceInputs, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let family = read_btx_family_source_inputs_from(&iso, &executables)?;
    let names = verify_box_btx_name_consumers(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        family
            .archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    let selections = verify_box_btx_descriptor_selections(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let review = verify_box_btx_family_review_decisions(&family.candidates)
        .map_err(|error| error.to_string())?;

    let seq_archives = BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
        .iter()
        .map(|expectation| {
            let raw = iso
                .read_file(expectation.archive)
                .map_err(|error| error.to_string())?;
            let archive = VddArchive::parse(&raw)
                .map_err(|error| format!("{} VDD 파싱 실패: {error}", expectation.archive))?;
            Ok((expectation.archive, archive))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let sources = read_subgame_seq_sources(&seq_archives)?;
    let text = verify_box_subgame_seq_text_candidates(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let layouts = verify_box_subgame_seq_layouts(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
    )
    .map_err(|error| error.to_string())?;
    let evidence = audit_btx_review_layout_evidence(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &layouts,
        &names,
        &selections,
        &family.candidates,
        &review,
    )
    .map_err(|error| error.to_string())?;
    let review_layouts = source_bound_review_layouts(&evidence)?;
    Ok(BtxReviewLayoutSourceInputs {
        family,
        evidence,
        review_layouts,
    })
}

#[cfg(test)]
#[path = "btx_review_layout_inputs_tests.rs"]
mod tests;
