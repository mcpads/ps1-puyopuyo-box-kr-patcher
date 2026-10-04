//! 지원 원본 전체 gate의 역할별 orchestration.

#[path = "source_audit/archive_inventory.rs"]
mod archive_inventory;
#[path = "source_audit/btx_catalog.rs"]
mod btx_catalog;
#[path = "source_audit/executable_consumers.rs"]
mod executable_consumers;
#[path = "source_audit/graphics_consumers.rs"]
mod graphics_consumers;
#[path = "source_audit/quest_sequences.rs"]
mod quest_sequences;
#[path = "source_audit/quest_vm.rs"]
mod quest_vm;
#[path = "source_audit/subgame_assets.rs"]
mod subgame_assets;
#[path = "source_audit/subgame_scenes.rs"]
mod subgame_scenes;
#[path = "source_audit/subgame_text.rs"]
mod subgame_text;

use std::path::PathBuf;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::script::{
    SubgameSeqDracTextAnalysis, SubgameSeqLayoutAnalysis, SubgameSeqSceneTextAnalysis,
    SubgameSeqTextCandidateAnalysis, SubgameSeqTextRootAnalysis,
};
use ps1_puyopuyo_box::source::{VOLUME_ID, VOLUME_SECTORS};

use super::disc_commands::read_box_executables;
use super::{BoxExecutable, BOX_ISO_ENTRIES};

use ps1_puyopuyo_box::disasm::bios_font::BiosFontAnalysis;
use ps1_puyopuyo_box::disasm::btx_cell::BtxCellAnalysis;
use ps1_puyopuyo_box::disasm::common_font_draw::CommonFontDrawSiteAnalysis;
use ps1_puyopuyo_box::disasm::quest_font::QuestFontConsumerSummary;
use ps1_puyopuyo_box::disasm::subgame_seq_control::SubgameControlAnalysis;
use ps1_puyopuyo_box::disasm::subgame_seq_text_consumer::SubgameTextConsumerAnalysis;
use ps1_puyopuyo_box::disasm::SubgameSeqDracConsumerAnalysis;
use ps1_puyopuyo_box::disc::reserved::Box32mSummary;

use archive_inventory::audit_archive_inventory;
use btx_catalog::audit_btx_catalog;
use executable_consumers::audit_executable_consumers;
use graphics_consumers::audit_graphics_consumers;
use quest_sequences::audit_quest_sequences;
use quest_vm::audit_quest_vm;
use subgame_assets::audit_subgame_assets;
use subgame_scenes::audit_subgame_scenes;
use subgame_text::audit_subgame_text;

pub(super) struct SourceAuditContext<'a> {
    source: &'a VerifiedSourceImage,
    iso: Iso9660<'a>,
    box_executables: Vec<BoxExecutable>,
    sectors: ps1_puyopuyo_box::disc::SectorCounts,
}

pub(super) struct ArchiveAuditData {
    vdd_sources: Vec<(String, Vec<u8>)>,
    cnx_archives: Vec<(String, VddArchive)>,
}

/// EXE 공통 stage가 한 번 계산하고 후속 source stage에 전달하는 결과.
pub(super) struct ExecutableConsumerAuditData {
    reserved: Box32mSummary,
    bios_font: BiosFontAnalysis,
    subgame_text_consumers: SubgameTextConsumerAnalysis,
    subgame_controls: SubgameControlAnalysis,
    btx_cells: BtxCellAnalysis,
    common_font_draw_sites: CommonFontDrawSiteAnalysis,
    quest_font_consumer: QuestFontConsumerSummary,
}

impl ArchiveAuditData {
    fn archive_copies(&self) -> Vec<(&str, VddArchive)> {
        self.cnx_archives
            .iter()
            .map(|(source_id, archive)| (source_id.as_str(), archive.clone()))
            .collect()
    }
}

pub(super) struct SubgameTextAuditData {
    sources: Vec<(String, String, Vec<u8>)>,
    candidates: SubgameSeqTextCandidateAnalysis,
    layouts: SubgameSeqLayoutAnalysis,
    text_roots: SubgameSeqTextRootAnalysis,
    min_text: SubgameSeqSceneTextAnalysis,
    drac_text: SubgameSeqDracTextAnalysis,
    drac_consumers: SubgameSeqDracConsumerAnalysis,
}

pub(super) struct SubgameAssetAuditData {
    btx_names: ps1_puyopuyo_box::disasm::BtxNameAnalysis,
    btx_descriptor_selections: ps1_puyopuyo_box::disasm::BtxDescriptorSelectionAnalysis,
    e6_text: ps1_puyopuyo_box::disasm::SubgameSeqE6TextAnalysis,
    e6_translation: ps1_puyopuyo_box::pipeline::SubgameSeqE6TranslationPlanAnalysis,
}

/// 지원 원본을 한 번 열고 역할별 gate에 같은 검증 입력을 전달한다.
pub(super) fn audit_source_disc(paths: &[PathBuf]) -> Result<(), String> {
    let [path] = paths else {
        return Err("검증할 원본 BIN 경로는 정확히 하나여야 한다".to_string());
    };
    let source =
        VerifiedSourceImage::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let image = source.image();
    let sectors = image
        .verify_all_sectors()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let iso = Iso9660::parse(image).map_err(|error| error.to_string())?;
    if iso.volume_id() != VOLUME_ID {
        return Err(format!(
            "ISO volume ID 불일치: expected={VOLUME_ID}, actual={}",
            iso.volume_id()
        ));
    }
    if iso.volume_sectors() != VOLUME_SECTORS {
        return Err(format!(
            "ISO volume sector 수 불일치: expected={VOLUME_SECTORS}, actual={}",
            iso.volume_sectors()
        ));
    }
    if iso.entries().len() != BOX_ISO_ENTRIES {
        return Err(format!(
            "ISO 엔트리 수 불일치: expected={BOX_ISO_ENTRIES}, actual={}",
            iso.entries().len()
        ));
    }

    let box_executables = read_box_executables(&iso)?;
    let context = SourceAuditContext {
        source: &source,
        iso,
        box_executables,
        sectors,
    };

    let executable_consumers = audit_executable_consumers(&context)?;
    let archives = audit_archive_inventory(&context)?;
    let subgame_text = audit_subgame_text(
        &context,
        &archives,
        &executable_consumers.subgame_text_consumers,
        &executable_consumers.subgame_controls,
    )?;
    let subgame_assets = audit_subgame_assets(
        &context,
        &archives,
        &subgame_text,
        &executable_consumers.btx_cells,
        &executable_consumers.common_font_draw_sites,
    )?;
    audit_subgame_scenes(
        &context,
        &archives,
        &subgame_text,
        &subgame_assets,
        &executable_consumers.btx_cells,
    )?;
    audit_graphics_consumers(
        &context,
        &archives,
        &subgame_text,
        &subgame_assets,
        &executable_consumers.common_font_draw_sites,
    )?;
    audit_quest_sequences(
        &context,
        &archives,
        &executable_consumers.reserved,
        &executable_consumers.bios_font,
        &executable_consumers.quest_font_consumer,
    )?;
    audit_btx_catalog(&context, &archives, &subgame_text, &subgame_assets)
}
