//! Expected Write 적용 결과를 다시 파싱해 코드·asset·무변경 경계를 검증한다.

use crate::archive::VddArchive;
use crate::compress::decompress;
use crate::coords::{exe_map, PSX_EXE_HEADER_LEN};
use crate::disc::{
    apply_form1_write_plan, DiscImage, Iso9660, PsxExe, VerifiedSourceImage, BOX_32M_PATH,
};
use crate::isa::{verify_bytes, IsaProfile};

use super::super::quest_exe_relocation_control::{
    count_extent_overlaps, extent_intervals, verify_header_identity,
};
use super::super::quest_hook_poc::QUEST_HOOK_BODY_RUNTIME;
use super::super::quest_poc::PreparedQuestPocAssets;
use super::super::QUEST_EXE_ISO_PATH;
use super::expected_writes::QuestResidentPagesPlan;
use super::{
    digest, error, QuestPageFirstDialogueError, QuestPageFirstDialogueReport,
    QuestResidentPagesBuild, QuestResidentPagesMode, QUEST_PAGE_CARRIER_START_RUNTIME,
    QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME, QUEST_PAGE_FIRST_DIALOGUE_SCHEMA_VERSION,
    QUEST_PAGE_HOOK_STATE_RUNTIME, RPG_GRP_PATH, RPG_SEQ_PATH, TOWN00_NAME,
};

fn verify_original_text_except_hook(
    original: &[u8],
    relocated: &[u8],
    patched_ranges: &[(usize, usize)],
) -> bool {
    original.len() == relocated.len()
        && original
            .iter()
            .zip(relocated)
            .enumerate()
            .all(|(offset, (a, b))| {
                patched_ranges
                    .iter()
                    .any(|(start, end)| (*start..*end).contains(&offset))
                    || a == b
            })
}

pub(super) fn apply_and_verify_quest_resident_pages(
    source: &VerifiedSourceImage,
    prepared: &PreparedQuestPocAssets,
    mode: QuestResidentPagesMode,
    planned: QuestResidentPagesPlan,
) -> Result<QuestResidentPagesBuild, QuestPageFirstDialogueError> {
    let QuestResidentPagesPlan {
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
    } = planned;
    let include_restore = mode.include_restore();
    let relocated_32m_lba = relocated_32m.lba;
    let relocated_32m_bytes = relocated_32m.len;
    let source_iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let original_quest_raw = source_iso
        .read_file(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_exe =
        PsxExe::parse(&original_quest_raw).map_err(|source| error(source.to_string()))?;
    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    original_exe
        .verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    let output = apply_form1_write_plan(source.image(), &plan)
        .map_err(|source| error(source.to_string()))?;
    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    let final_quest_raw = final_iso
        .read_file_at_extent(QUEST_EXE_ISO_PATH, relocated_quest)
        .map_err(|source| error(source.to_string()))?;
    let extent_overlaps = count_extent_overlaps(
        &extent_intervals(&final_iso).map_err(|source| error(source.to_string()))?,
    )
    .map_err(|source| error(source.to_string()))?;

    let final_exe = PsxExe::parse(&final_quest_raw).map_err(|source| error(source.to_string()))?;
    verify_header_identity(&original_exe, &final_exe, relocated_text_bytes as usize)
        .map_err(|source| error(source.to_string()))?;
    for (runtime, verified) in [
        (crate::disasm::QUEST_FONT_HOOK_SITE_RUNTIME, &sources.site),
        (QUEST_HOOK_BODY_RUNTIME, &sources.body),
    ] {
        let offset = map
            .text_offset(runtime)
            .ok_or_else(|| error("최종 hook text offset 사상 실패"))?;
        let actual = final_exe
            .text()
            .get(offset..offset + verified.bytes().len())
            .ok_or_else(|| error("최종 hook readback 범위 이탈"))?;
        if actual != verified.bytes() {
            return Err(error(format!("최종 hook readback 불일치: 0x{runtime:08X}")));
        }
        verify_bytes(IsaProfile::DECLARED, runtime, actual)?;
    }
    let site_text_offset = usize::try_from(site_offset - PSX_EXE_HEADER_LEN)
        .map_err(|_| error("site text offset 변환 실패"))?;
    let body_text_offset = usize::try_from(body_offset - PSX_EXE_HEADER_LEN)
        .map_err(|_| error("body text offset 변환 실패"))?;
    let original_text_except_hook_exact = verify_original_text_except_hook(
        original_exe.text(),
        &final_exe.text()[..original_exe.text().len()],
        &[
            (
                site_text_offset,
                site_text_offset + sources.site.bytes().len(),
            ),
            (
                body_text_offset,
                body_text_offset + sources.body.bytes().len(),
            ),
        ],
    );
    if !original_text_except_hook_exact {
        return Err(error("최종 QUEST 원본 text의 hook 밖 byte가 달라짐"));
    }
    let page_text_offset = usize::try_from(
        QUEST_PAGE_CARRIER_START_RUNTIME
            .checked_sub(map.load_addr)
            .ok_or_else(|| error("page text offset underflow"))?,
    )
    .map_err(|_| error("page text offset 변환 실패"))?;
    let page_end = page_text_offset
        .checked_add(prepared.new_font_plain.len())
        .ok_or_else(|| error("page readback 끝 overflow"))?;
    let resident_page = final_exe
        .text()
        .get(page_text_offset..page_end)
        .ok_or_else(|| error("resident page readback 범위 이탈"))?;
    let resident_page_exact = resident_page == prepared.new_font_plain;
    if !resident_page_exact {
        return Err(error("resident raw page 최종 readback 불일치"));
    }
    let original_page_end = page_end
        .checked_add(if include_restore {
            prepared.original_font_plain.len()
        } else {
            0
        })
        .ok_or_else(|| error("original page readback 끝 overflow"))?;
    let original_page_exact = if include_restore {
        final_exe
            .text()
            .get(page_end..original_page_end)
            .ok_or_else(|| error("resident original page readback 범위 이탈"))?
            == prepared.original_font_plain
    } else {
        true
    };
    if !original_page_exact {
        return Err(error("resident raw 원본 page 최종 readback 불일치"));
    }
    let pages_differ = prepared.new_font_plain != prepared.original_font_plain;
    if include_restore && !pages_differ {
        return Err(error("QUEST 전환 PoC의 두 resident page가 같음"));
    }
    let extension = &final_exe.text()[original_exe.text().len()..];
    let extension_page_start = page_text_offset
        .checked_sub(original_exe.text().len())
        .ok_or_else(|| error("resident page가 원본 text 안에 겹침"))?;
    let extension_page_end = extension_page_start
        .checked_add(resident_page_bytes)
        .ok_or_else(|| error("extension resident pages 끝 overflow"))?;
    let extension_outside_page_zero = extension[..extension_page_start]
        .iter()
        .chain(extension[extension_page_end..].iter())
        .all(|byte| *byte == 0);
    if !extension_outside_page_zero {
        return Err(error("resident pages 밖 QUEST extension이 전량 0이 아님"));
    }
    let final_seq_vdd = final_iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|source| error(source.to_string()))?;
    if final_seq_vdd != prepared.new_seq_vdd {
        return Err(error("최종 RPG_SEQ.VDD readback 불일치"));
    }
    let final_seq =
        VddArchive::parse(&final_seq_vdd).map_err(|source| error(source.to_string()))?;
    let final_town = final_seq
        .find(TOWN00_NAME)
        .ok_or_else(|| error("최종 TOWN00.SEQ가 없음"))?;
    if decompress(final_seq.read_file(final_town)).map_err(|source| error(source.to_string()))?
        != prepared.new_town_plain
    {
        return Err(error("최종 TOWN00.SEQ plain readback 불일치"));
    }
    let original_font_archive_unchanged = final_iso
        .read_file(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?
        == prepared.grp_vdd;
    if !original_font_archive_unchanged {
        return Err(error("page PoC가 원본 RPG_GRP.VDD를 변경함"));
    }
    let final_32m_raw = final_iso
        .read_file_at_extent(BOX_32M_PATH, relocated_32m)
        .map_err(|source| error(source.to_string()))?;
    if final_32m_raw.len() != relocated_32m_bytes as usize
        || final_32m_raw.iter().any(|byte| *byte != 0)
    {
        return Err(error("분할 뒤 32M.BIN 길이 또는 전량 0 계약 불일치"));
    }
    let original_quest_extent_unchanged = source.image().raw_bytes()
        [original_extent_start as usize..original_extent_end as usize]
        == final_image.raw_bytes()[original_extent_start as usize..original_extent_end as usize];
    if !original_quest_extent_unchanged {
        return Err(error("QUEST 원본 raw extent 무변경 경계 불일치"));
    }

    let image = final_image.into_bytes();
    let image_sha256 = digest(&image);
    Ok(QuestResidentPagesBuild {
        image,
        report: QuestPageFirstDialogueReport {
            schema_version: QUEST_PAGE_FIRST_DIALOGUE_SCHEMA_VERSION,
            target_record_runtime: QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME,
            site_runtime: crate::disasm::QUEST_FONT_HOOK_SITE_RUNTIME,
            body_runtime: QUEST_HOOK_BODY_RUNTIME,
            body_bytes: sources.body.bytes().len(),
            body_slack_bytes: QUEST_PAGE_HOOK_STATE_RUNTIME as usize
                - (QUEST_HOOK_BODY_RUNTIME as usize + sources.body.bytes().len()),
            page_source_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
            page_bytes: prepared.new_font_plain.len(),
            relocated_quest_lba: relocated_quest.lba,
            relocated_quest_bytes,
            relocated_quest_sectors,
            relocated_32m_lba,
            relocated_32m_bytes,
            town00_cnx_bytes: prepared.new_town_cnx.len(),
            machine_code_writes,
            data_writes,
            metadata_writes,
            relocation_copy_writes,
            touched_sectors,
            final_expected_writes,
            original_text_except_hook_exact,
            resident_page_exact,
            extension_outside_page_zero,
            original_font_archive_unchanged,
            original_quest_extent_unchanged,
            extent_overlaps,
            runtime_verified: false,
            release_eligible: false,
            image_sha256,
        },
        original_page_exact,
        pages_differ,
    })
}
