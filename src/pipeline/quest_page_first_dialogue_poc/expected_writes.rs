//! 불변 원본에서 resident page PoC의 전체 Expected Write를 먼저 구성한다.

use crate::archive::VddArchive;
use crate::coords::{exe_map, LogicalFile, PSX_EXE_HEADER_LEN};
use crate::disasm::{verify_box_quest_hook_space_candidates, verify_box_quest_page_carrier};
use crate::disc::{
    add_single_extent_record_update, partition_extent_with_remainder, verify_box_32m_reservation,
    Extent, Iso9660, PsxExe, VerifiedSourceImage, BOX_32M_PATH,
};
use crate::isa::VerifiedMachineCode;
use crate::patch::{
    plan_logical_copy, plan_logical_write, RegionKind, UnchangedBoundary, WriteIntent, WritePlan,
};
use crate::source::{FORM1_DATA_LEN, SECTOR_LEN};

use super::super::quest_exe_relocation_control::add_region_write;
use super::super::quest_hook_poc::{
    verify_quest_hook_noop_sources, QUEST_HOOK_BODY_RUNTIME, QUEST_HOOK_RESERVED_BYTES,
};
use super::super::quest_poc::PreparedQuestPocAssets;
use super::super::vdd_plan::{add_logical_span, add_vdd_entry_replacement, logical_file};
use super::super::QUEST_EXE_ISO_PATH;
use super::machine_code::{
    assemble_body, assemble_transition_body, assemble_two_dialogue_transition_body,
};
use super::{
    error, QuestPageFirstDialogueError, QuestPageFirstDialogueSources, QuestResidentPagesMode,
    ACTOR, QUEST_PAGE_CARRIER_CAPACITY_BYTES, QUEST_PAGE_CARRIER_END_RUNTIME,
    QUEST_PAGE_CARRIER_START_RUNTIME, QUEST_PAGE_FIRST_DIALOGUE_BODY_BYTES,
    QUEST_PAGE_HOOK_STATE_RUNTIME, QUEST_PAGE_RESTORE_SOURCE_RUNTIME,
    QUEST_PAGE_TRANSITION_BODY_BYTES, QUEST_PAGE_TWO_DIALOGUES_BODY_BYTES, RPG_SEQ_PATH,
    TOWN00_NAME,
};

#[derive(Debug)]
pub(super) struct QuestResidentPagesPlan {
    pub(super) plan: WritePlan,
    pub(super) sources: QuestPageFirstDialogueSources,
    pub(super) relocated_quest: Extent,
    pub(super) relocated_32m: Extent,
    pub(super) relocated_text_bytes: u32,
    pub(super) relocated_quest_bytes: u32,
    pub(super) relocated_quest_sectors: u32,
    pub(super) original_extent_start: u64,
    pub(super) original_extent_end: u64,
    pub(super) site_offset: u64,
    pub(super) body_offset: u64,
    pub(super) resident_page_bytes: usize,
    pub(super) relocation_copy_writes: usize,
    pub(super) machine_code_writes: usize,
    pub(super) data_writes: usize,
    pub(super) metadata_writes: usize,
    pub(super) touched_sectors: usize,
    pub(super) final_expected_writes: usize,
}

fn add_machine_code_write(
    plan: &mut WritePlan,
    file: LogicalFile,
    file_offset: u64,
    expected: &[u8],
    verified: &VerifiedMachineCode,
    id: &str,
    purpose: &str,
) -> Result<(), QuestPageFirstDialogueError> {
    let writes = plan_logical_write(
        file,
        file_offset,
        expected,
        verified.bytes(),
        id,
        ACTOR,
        purpose,
        |_| WriteIntent::MachineCode {
            verified: Box::new(verified.clone()),
        },
    )
    .ok_or_else(|| error(format!("{id}: 논리 기계어 쓰기 계획 생성 실패")))?;
    if writes.len() != 1 {
        return Err(error(format!(
            "{id}: 기계어가 Mode2 sector 경계에서 {}개로 분할됨",
            writes.len()
        )));
    }
    add_region_write(
        plan,
        writes.into_iter().next().expect("길이 1을 확인함"),
        "지원 QUEST.EXE의 scene-bound typed MIPS hook",
    );
    Ok(())
}

fn add_copy_range(
    plan: &mut WritePlan,
    baseline: &[u8],
    source_file: LogicalFile,
    destination_file: LogicalFile,
    start: u64,
    end: u64,
    id: &str,
) -> Result<usize, QuestPageFirstDialogueError> {
    if start >= end {
        return Ok(0);
    }
    let writes = plan_logical_copy(
        baseline,
        source_file,
        start,
        destination_file,
        start,
        end - start,
        id,
        ACTOR,
        "hook 두 구간을 제외한 QUEST text의 byte-identical relocation",
        QUEST_EXE_ISO_PATH,
    )
    .ok_or_else(|| error(format!("{id}: QUEST relocation copy 계획 생성 실패")))?;
    let count = writes.len();
    for write in writes {
        add_region_write(
            plan,
            write,
            "지원 원본 QUEST text의 byte-identical ISO file relocation",
        );
    }
    Ok(count)
}

fn source_slice(
    raw: &[u8],
    file_offset: u64,
    len: usize,
) -> Result<&[u8], QuestPageFirstDialogueError> {
    let start = usize::try_from(file_offset).map_err(|_| error("QUEST offset 변환 실패"))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error("QUEST slice 범위 overflow"))?;
    raw.get(start..end)
        .ok_or_else(|| error("QUEST slice가 파일 범위 밖임"))
}

pub(super) fn plan_quest_resident_pages(
    source: &VerifiedSourceImage,
    prepared: &PreparedQuestPocAssets,
    mode: QuestResidentPagesMode,
) -> Result<QuestResidentPagesPlan, QuestPageFirstDialogueError> {
    let include_restore = mode.include_restore();
    let resident_page_bytes = prepared
        .new_font_plain
        .len()
        .checked_add(if include_restore {
            prepared.original_font_plain.len()
        } else {
            0
        })
        .ok_or_else(|| error("QUEST resident page 합계 overflow"))?;
    if resident_page_bytes > QUEST_PAGE_CARRIER_CAPACITY_BYTES {
        return Err(error(format!(
            "QUEST resident raw page가 carrier를 넘음: pages={}B, carrier={}B",
            resident_page_bytes, QUEST_PAGE_CARRIER_CAPACITY_BYTES
        )));
    }

    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let quest_entry = iso
        .find(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let reservation_entry = iso
        .find(BOX_32M_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_quest_file = logical_file(quest_entry).map_err(error)?;
    let original_quest_raw = iso
        .read_file(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_exe =
        PsxExe::parse(&original_quest_raw).map_err(|source| error(source.to_string()))?;
    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    original_exe
        .verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    verify_box_quest_page_carrier(map, original_exe.text())
        .map_err(|source| error(source.to_string()))?;
    let candidates = verify_box_quest_hook_space_candidates(map, original_exe.text())
        .map_err(|source| error(source.to_string()))?;
    let noop = verify_quest_hook_noop_sources(map, original_exe.text(), &candidates)
        .map_err(|source| error(source.to_string()))?;
    let body = match mode {
        QuestResidentPagesMode::FirstOnly => assemble_body()?,
        QuestResidentPagesMode::FirstTransition => assemble_transition_body()?,
        QuestResidentPagesMode::TwoDialogues => assemble_two_dialogue_transition_body()?,
    };
    let expected_body_bytes = match mode {
        QuestResidentPagesMode::FirstOnly => QUEST_PAGE_FIRST_DIALOGUE_BODY_BYTES,
        QuestResidentPagesMode::FirstTransition => QUEST_PAGE_TRANSITION_BODY_BYTES,
        QuestResidentPagesMode::TwoDialogues => QUEST_PAGE_TWO_DIALOGUES_BODY_BYTES,
    };
    if noop.reserved_bytes != QUEST_HOOK_RESERVED_BYTES
        || body.bytes().len() != expected_body_bytes
        || body.base_addr() + body.bytes().len() as u32 > QUEST_PAGE_HOOK_STATE_RUNTIME
    {
        return Err(error(
            "QUEST 첫 대사 page hook body/reserved/state 경계 불일치",
        ));
    }
    let sources = QuestPageFirstDialogueSources {
        site: noop.site,
        body,
    };

    let reservation =
        verify_box_32m_reservation(&iso, source.image(), [("QUEST.EXE", original_exe.text())])
            .map_err(|source| error(source.to_string()))?;
    if reservation_entry.len() != reservation.bytes {
        return Err(error("32M reservation entry와 검증 요약 길이 불일치"));
    }
    let page_end_runtime = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(resident_page_bytes)
                .map_err(|_| error("resident page 길이 변환 overflow"))?,
        )
        .ok_or_else(|| error("QUEST resident page 끝 runtime overflow"))?;
    if page_end_runtime > QUEST_PAGE_CARRIER_END_RUNTIME {
        return Err(error("QUEST resident page가 정적 carrier 경계를 넘음"));
    }
    let unaligned_text_bytes = page_end_runtime
        .checked_sub(map.load_addr)
        .ok_or_else(|| error("QUEST resident page가 EXE load보다 앞섬"))?;
    let relocated_text_bytes = unaligned_text_bytes
        .checked_add(FORM1_DATA_LEN as u32 - 1)
        .ok_or_else(|| error("QUEST 확장 text sector 올림 overflow"))?
        / FORM1_DATA_LEN as u32
        * FORM1_DATA_LEN as u32;
    let relocated_quest_bytes = relocated_text_bytes
        .checked_add(
            u32::try_from(PSX_EXE_HEADER_LEN)
                .map_err(|_| error("PS-X EXE header 변환 overflow"))?,
        )
        .ok_or_else(|| error("QUEST 확장 파일 길이 overflow"))?;
    let relocated_quest_sectors = relocated_quest_bytes.div_ceil(FORM1_DATA_LEN as u32);
    let reservation_extent = Extent {
        lba: reservation.lba,
        len: u32::try_from(reservation.bytes)
            .map_err(|_| error("32M reservation 길이 변환 실패"))?,
    };
    let partition = partition_extent_with_remainder(reservation_extent, &[relocated_quest_bytes])
        .map_err(|source| error(source.to_string()))?;
    let [relocated_quest] = partition.allocations() else {
        return Err(error("QUEST relocation extent 분모 불일치"));
    };
    let relocated_quest = *relocated_quest;
    let relocated_32m = partition.remainder();
    let relocated_32m_bytes = relocated_32m.len;
    if relocated_quest_sectors + relocated_32m_bytes.div_ceil(FORM1_DATA_LEN as u32)
        != reservation.form1_sectors
    {
        return Err(error("QUEST/32M 분할 sector 합이 원본 reservation과 다름"));
    }
    let destination_file = LogicalFile::new(relocated_quest.lba, relocated_quest.len);
    let reservation_raw = iso
        .read_file(BOX_32M_PATH)
        .map_err(|source| error(source.to_string()))?;

    let header_len = usize::try_from(PSX_EXE_HEADER_LEN)
        .map_err(|_| error("PS-X EXE header 길이 변환 overflow"))?;
    let mut relocated_header = original_quest_raw[..header_len].to_vec();
    relocated_header[0x1C..0x20].copy_from_slice(&relocated_text_bytes.to_le_bytes());
    let mut plan = WritePlan::new();
    add_logical_span(
        &mut plan,
        destination_file,
        0,
        &reservation_raw[..header_len],
        &relocated_header,
        "quest-page-first-dialogue-header",
        ACTOR,
        "재배치한 PS-X EXE header와 resident page를 포함한 text 길이",
        WriteIntent::Metadata,
    )
    .map_err(error)?;

    let site_offset = map
        .file_offset(crate::disasm::QUEST_FONT_HOOK_SITE_RUNTIME)
        .ok_or_else(|| error("QUEST hook site 파일 사상 실패"))?;
    let body_offset = map
        .file_offset(QUEST_HOOK_BODY_RUNTIME)
        .ok_or_else(|| error("QUEST hook body 파일 사상 실패"))?;
    let text_start = PSX_EXE_HEADER_LEN;
    let text_end = text_start
        .checked_add(u64::from(map.text_len))
        .ok_or_else(|| error("QUEST 원본 text 파일 끝 overflow"))?;
    let site_end = site_offset
        .checked_add(sources.site.bytes().len() as u64)
        .ok_or_else(|| error("QUEST hook site 끝 overflow"))?;
    let body_end = body_offset
        .checked_add(sources.body.bytes().len() as u64)
        .ok_or_else(|| error("QUEST hook body 끝 overflow"))?;
    if !(text_start <= site_offset && site_end <= body_offset && body_end <= text_end) {
        return Err(error("QUEST hook patch 범위 순서가 원본 text 밖임"));
    }
    let mut relocation_copy_writes = 0usize;
    for (index, (start, end)) in [
        (text_start, site_offset),
        (site_end, body_offset),
        (body_end, text_end),
    ]
    .into_iter()
    .enumerate()
    {
        relocation_copy_writes += add_copy_range(
            &mut plan,
            source.image().raw_bytes(),
            original_quest_file,
            destination_file,
            start,
            end,
            &format!("quest-page-first-dialogue-copy-{index}"),
        )?;
    }

    add_machine_code_write(
        &mut plan,
        destination_file,
        site_offset,
        source_slice(&reservation_raw, site_offset, sources.site.bytes().len())?,
        &sources.site,
        "quest-page-first-dialogue-site",
        "OP21 entry를 scene-bound resident page hook으로 우회",
    )?;
    add_machine_code_write(
        &mut plan,
        destination_file,
        body_offset,
        source_slice(&reservation_raw, body_offset, sources.body.bytes().len())?,
        &sources.body,
        "quest-page-first-dialogue-body",
        "record/active/decode/upload/sync/commit page hook body",
    )?;
    let page_file_offset = PSX_EXE_HEADER_LEN
        .checked_add(u64::from(
            QUEST_PAGE_CARRIER_START_RUNTIME
                .checked_sub(map.load_addr)
                .ok_or_else(|| error("QUEST page source가 load 주소보다 앞섬"))?,
        ))
        .ok_or_else(|| error("QUEST page source 파일 offset overflow"))?;
    add_logical_span(
        &mut plan,
        destination_file,
        usize::try_from(page_file_offset).map_err(|_| error("QUEST page offset 변환 실패"))?,
        source_slice(
            &reservation_raw,
            page_file_offset,
            prepared.new_font_plain.len(),
        )?,
        &prepared.new_font_plain,
        "quest-page-first-dialogue-resident-page",
        ACTOR,
        "재배치 QUEST.EXE에 resident raw 한글 font page 삽입",
        WriteIntent::Data,
    )
    .map_err(error)?;
    if include_restore {
        let restore_page_file_offset = PSX_EXE_HEADER_LEN
            .checked_add(u64::from(
                QUEST_PAGE_RESTORE_SOURCE_RUNTIME
                    .checked_sub(map.load_addr)
                    .ok_or_else(|| error("QUEST restore page source가 load 주소보다 앞섬"))?,
            ))
            .ok_or_else(|| error("QUEST restore page source 파일 offset overflow"))?;
        add_logical_span(
            &mut plan,
            destination_file,
            usize::try_from(restore_page_file_offset)
                .map_err(|_| error("QUEST restore page offset 변환 실패"))?,
            source_slice(
                &reservation_raw,
                restore_page_file_offset,
                prepared.original_font_plain.len(),
            )?,
            &prepared.original_font_plain,
            "quest-page-transition-resident-original-page",
            ACTOR,
            "재배치 QUEST.EXE에 resident raw 원본 font page 삽입",
            WriteIntent::Data,
        )
        .map_err(error)?;
    }

    let seq_entry = iso
        .find(RPG_SEQ_PATH)
        .map_err(|source| error(source.to_string()))?;
    let seq_file = logical_file(seq_entry).map_err(error)?;
    let seq_archive =
        VddArchive::parse(&prepared.seq_vdd).map_err(|source| error(source.to_string()))?;
    add_vdd_entry_replacement(
        &mut plan,
        seq_file,
        &seq_archive,
        &prepared.seq_vdd,
        &prepared.new_seq_vdd,
        TOWN00_NAME,
        prepared.new_town_cnx.len(),
        "quest-page-first-dialogue-town00",
        ACTOR,
    )
    .map_err(error)?;

    for (entry, replacement, id, purpose) in [
        (
            quest_entry,
            relocated_quest,
            "quest-page-first-dialogue-directory-record",
            "QUEST.EXE를 resident page와 함께 32M reservation으로 재배치",
        ),
        (
            reservation_entry,
            relocated_32m,
            "quest-page-first-dialogue-32m-record",
            "resident page QUEST.EXE 뒤에서 시작하도록 32M.BIN 축소",
        ),
    ] {
        add_single_extent_record_update(
            &mut plan,
            source.image(),
            entry,
            replacement,
            id,
            ACTOR,
            purpose,
        )
        .map_err(|source| error(source.to_string()))?;
    }

    let original_extent_start = u64::from(original_quest_file.lba) * SECTOR_LEN as u64;
    let original_extent_end = original_extent_start
        .checked_add(u64::from(original_quest_file.sector_count()) * SECTOR_LEN as u64)
        .ok_or_else(|| error("QUEST 원본 raw extent 경계 overflow"))?;
    plan.unchanged.push(UnchangedBoundary::new(
        "quest-page-first-dialogue-original-extent",
        original_extent_start..original_extent_end,
        "재배치 뒤 원본 QUEST raw extent byte-identical 보존",
    ));

    let machine_code_writes = plan
        .writes
        .iter()
        .filter(|write| write.intent.region_kind() == RegionKind::MachineCode)
        .count();
    let data_writes = plan
        .writes
        .iter()
        .filter(|write| write.intent.region_kind() == RegionKind::Data)
        .count();
    let metadata_writes = plan
        .writes
        .iter()
        .filter(|write| write.intent.region_kind() == RegionKind::Metadata)
        .count();
    let touched_sectors = plan.touched_sectors().len();
    let final_expected_writes = plan.writes.len() + touched_sectors;
    Ok(QuestResidentPagesPlan {
        plan,
        sources,
        relocated_quest,
        relocated_32m,
        relocated_text_bytes,
        relocated_quest_bytes,
        relocated_quest_sectors,
        original_extent_start,
        original_extent_end,
        site_offset,
        body_offset,
        resident_page_bytes,
        relocation_copy_writes,
        machine_code_writes,
        data_writes,
        metadata_writes,
        touched_sectors,
        final_expected_writes,
    })
}
