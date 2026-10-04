//! `32M.BIN`을 분할해 확장 `QUEST.EXE`를 담는 ISO 재배치 control.
//!
//! 원본 QUEST text는 runtime 주소와 byte를 전혀 바꾸지 않고 새 extent로
//! exact copy한다. PS-X EXE header의 text 길이와 두 ISO directory record만
//! Metadata로 갱신하며, 추가 text 영역은 지원 원본 `32M.BIN`의 0을 그대로
//! 사용한다. 아직 page payload나 hook은 넣지 않는다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::{
    exe_map, psx_main_ram_offset, LogicalFile, PSX_EXE_HEADER_LEN, PSX_MAIN_RAM_LEN,
};
use crate::disasm::verify_box_quest_page_carrier;
use crate::disc::{
    add_single_extent_record_update, apply_form1_write_plan, partition_extent_with_remainder,
    verify_box_32m_reservation, DiscImage, Extent, Iso9660, PsxExe, VerifiedSourceImage,
    BOX_32M_PATH,
};
use crate::patch::{
    plan_logical_copy, ImageRegion, RegionKind, UnchangedBoundary, WriteIntent, WritePlan,
};
use crate::source::{FORM1_DATA_LEN, SECTOR_LEN};

use super::quest_page_carrier_plan::{
    QUEST_EXE_ISO_PATH, QUEST_RELOCATION_CONTROL_FILE_BYTES, QUEST_RELOCATION_CONTROL_SECTORS,
    QUEST_RELOCATION_CONTROL_TEXT_BYTES,
};
use super::vdd_plan::{add_logical_span, logical_file};

pub const QUEST_EXE_RELOCATION_CONTROL_SCHEMA_VERSION: &str = "quest-exe-relocation-control-v1";
pub const QUEST_EXE_RELOCATION_RUNTIME_SCHEMA_VERSION: &str = "quest-exe-relocation-runtime-v1";
const ACTOR: &str = "pipeline::quest_exe_relocation_control";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestExeRelocationControlReport {
    pub schema_version: &'static str,
    pub original_quest_lba: u32,
    pub original_quest_bytes: u32,
    pub original_quest_sectors: u32,
    pub relocated_quest_lba: u32,
    pub relocated_quest_bytes: u32,
    pub relocated_quest_sectors: u32,
    pub relocated_32m_lba: u32,
    pub relocated_32m_bytes: u32,
    pub relocated_32m_sectors: u32,
    pub metadata_writes: usize,
    pub relocation_copy_writes: usize,
    pub touched_sectors: usize,
    pub final_expected_writes: usize,
    pub original_text_exact: bool,
    pub extension_zero: bool,
    pub original_extent_unchanged: bool,
    pub extent_overlaps: usize,
    pub runtime_verified: bool,
    pub build_eligible: bool,
    pub image_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestExeRelocationControlBuild {
    pub image: Vec<u8>,
    pub report: QuestExeRelocationControlReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestExeRelocationRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub ram_sha256: String,
    pub load_addr: u32,
    pub original_text_bytes: usize,
    pub loaded_text_bytes: usize,
    pub extension_bytes: usize,
    pub expected_text_sha256: String,
    pub loaded_text_sha256: String,
    pub extension_sha256: String,
    pub loaded_text_exact: bool,
    pub extension_zero: bool,
}

/// exact build의 재배치·검증 결과를 한 행 TSV로 만든다.
pub fn render_quest_exe_relocation_control_tsv(report: &QuestExeRelocationControlReport) -> String {
    format!(
        "schema_version\toriginal_quest_lba\toriginal_quest_bytes\t\
         original_quest_sectors\trelocated_quest_lba\trelocated_quest_bytes\t\
         relocated_quest_sectors\trelocated_32m_lba\trelocated_32m_bytes\t\
         relocated_32m_sectors\tmetadata_writes\trelocation_copy_writes\t\
         touched_sectors\tfinal_expected_writes\toriginal_text_exact\t\
         extension_zero\toriginal_extent_unchanged\textent_overlaps\t\
         runtime_verified\tbuild_eligible\timage_sha256\n\
         {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.original_quest_lba,
        report.original_quest_bytes,
        report.original_quest_sectors,
        report.relocated_quest_lba,
        report.relocated_quest_bytes,
        report.relocated_quest_sectors,
        report.relocated_32m_lba,
        report.relocated_32m_bytes,
        report.relocated_32m_sectors,
        report.metadata_writes,
        report.relocation_copy_writes,
        report.touched_sectors,
        report.final_expected_writes,
        report.original_text_exact,
        report.extension_zero,
        report.original_extent_unchanged,
        report.extent_overlaps,
        report.runtime_verified,
        report.build_eligible,
        report.image_sha256,
    )
}

/// QUEST entry에서 얻은 main RAM dump의 확장 text 적재를 한 행 TSV로 만든다.
pub fn render_quest_exe_relocation_runtime_tsv(report: &QuestExeRelocationRuntimeReport) -> String {
    format!(
        "schema_version\timage_sha256\tram_sha256\tload_addr\t\
         original_text_bytes\tloaded_text_bytes\textension_bytes\t\
         expected_text_sha256\tloaded_text_sha256\textension_sha256\t\
         loaded_text_exact\textension_zero\n\
         {}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.image_sha256,
        report.ram_sha256,
        report.load_addr,
        report.original_text_bytes,
        report.loaded_text_bytes,
        report.extension_bytes,
        report.expected_text_sha256,
        report.loaded_text_sha256,
        report.extension_sha256,
        report.loaded_text_exact,
        report.extension_zero,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestExeRelocationControlError(String);

impl fmt::Display for QuestExeRelocationControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestExeRelocationControlError {}

fn error(message: impl Into<String>) -> QuestExeRelocationControlError {
    QuestExeRelocationControlError(message.into())
}

pub(super) fn add_region_write(
    plan: &mut WritePlan,
    write: crate::patch::ExpectedWrite,
    reason: &str,
) {
    plan.regions.push(ImageRegion::new(
        format!("region:{}", write.id),
        write.range(),
        write.intent.region_kind(),
        reason,
    ));
    plan.writes.push(write);
}

pub(super) fn verify_header_identity(
    original: &PsxExe<'_>,
    relocated: &PsxExe<'_>,
    expected_text_bytes: usize,
) -> Result<(), QuestExeRelocationControlError> {
    for (field, expected, actual) in [
        ("entry PC", original.entry_pc(), relocated.entry_pc()),
        ("initial GP", original.initial_gp(), relocated.initial_gp()),
        ("load address", original.load_addr(), relocated.load_addr()),
        ("data address", original.data_addr(), relocated.data_addr()),
        ("data length", original.data_len(), relocated.data_len()),
        ("BSS address", original.bss_addr(), relocated.bss_addr()),
        ("BSS length", original.bss_len(), relocated.bss_len()),
        ("stack base", original.stack_base(), relocated.stack_base()),
        ("stack length", original.stack_len(), relocated.stack_len()),
    ] {
        if actual != expected {
            return Err(error(format!(
                "QUEST relocation header {field} 불일치: expected=0x{expected:X}, actual=0x{actual:X}"
            )));
        }
    }
    if relocated.text_len() as usize != expected_text_bytes {
        return Err(error(format!(
            "QUEST relocation text 길이 불일치: expected={}, actual={}",
            expected_text_bytes,
            relocated.text_len()
        )));
    }
    Ok(())
}

pub(super) fn extent_intervals(
    iso: &Iso9660<'_>,
) -> Result<Vec<(u32, u32, String)>, QuestExeRelocationControlError> {
    let mut intervals = Vec::new();
    for entry in iso.entries() {
        for extent in entry.extents() {
            if extent.len == 0 {
                return Err(error(format!("{}: 길이 0 ISO extent", entry.path())));
            }
            let sectors = extent.len.div_ceil(FORM1_DATA_LEN as u32);
            let end = extent
                .lba
                .checked_add(sectors)
                .ok_or_else(|| error(format!("{}: ISO extent 끝 LBA overflow", entry.path())))?;
            if end > iso.volume_sectors() {
                return Err(error(format!(
                    "{}: ISO extent가 volume 밖임: [{},{end})",
                    entry.path(),
                    extent.lba
                )));
            }
            intervals.push((extent.lba, end, entry.path().to_string()));
        }
    }
    intervals.sort_by(|left, right| (left.0, left.1, &left.2).cmp(&(right.0, right.1, &right.2)));
    Ok(intervals)
}

pub(super) fn count_extent_overlaps(
    intervals: &[(u32, u32, String)],
) -> Result<usize, QuestExeRelocationControlError> {
    for pair in intervals.windows(2) {
        let (left_start, left_end, left_path) = &pair[0];
        let (right_start, right_end, right_path) = &pair[1];
        if right_start < left_end {
            return Err(error(format!(
                "ISO extent 겹침: {left_path} [{left_start},{left_end}) / \
                 {right_path} [{right_start},{right_end})"
            )));
        }
    }
    Ok(0)
}

/// 지원 원본에서 확장·재배치만 수행한 QUEST control 이미지를 만든다.
pub fn build_quest_exe_relocation_control(
    source: &VerifiedSourceImage,
) -> Result<QuestExeRelocationControlBuild, QuestExeRelocationControlError> {
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

    let reservation =
        verify_box_32m_reservation(&iso, source.image(), [("QUEST.EXE", original_exe.text())])
            .map_err(|source| error(source.to_string()))?;
    if reservation_entry.len() != reservation.bytes {
        return Err(error("32M reservation entry와 검증 요약 길이 불일치"));
    }

    let relocated_quest_bytes = u32::try_from(QUEST_RELOCATION_CONTROL_FILE_BYTES)
        .map_err(|_| error("QUEST relocation 파일 길이 변환 overflow"))?;
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
    let relocated_32m_lba = relocated_32m.lba;
    let relocated_32m_bytes = relocated_32m.len;
    let relocated_32m_sectors = relocated_32m_bytes.div_ceil(FORM1_DATA_LEN as u32);
    if QUEST_RELOCATION_CONTROL_SECTORS + relocated_32m_sectors != reservation.form1_sectors {
        return Err(error("QUEST/32M 분할 sector 합이 원본 reservation과 다름"));
    }

    let destination_file = LogicalFile::new(relocated_quest.lba, relocated_quest.len);
    let reservation_raw = iso
        .read_file(BOX_32M_PATH)
        .map_err(|source| error(source.to_string()))?;
    let header_len = usize::try_from(PSX_EXE_HEADER_LEN)
        .map_err(|_| error("PS-X EXE header 길이 변환 overflow"))?;
    let mut relocated_header = original_quest_raw[..header_len].to_vec();
    relocated_header[0x1C..0x20]
        .copy_from_slice(&(QUEST_RELOCATION_CONTROL_TEXT_BYTES as u32).to_le_bytes());

    let mut plan = WritePlan::new();
    add_logical_span(
        &mut plan,
        destination_file,
        0,
        &reservation_raw[..header_len],
        &relocated_header,
        "quest-relocation-header",
        ACTOR,
        "재배치한 PS-X EXE header와 확장 text 길이",
        WriteIntent::Metadata,
    )
    .map_err(error)?;

    let copy_writes = plan_logical_copy(
        source.image().raw_bytes(),
        original_quest_file,
        PSX_EXE_HEADER_LEN,
        destination_file,
        PSX_EXE_HEADER_LEN,
        u64::from(map.text_len),
        "quest-relocation-text",
        ACTOR,
        "runtime 주소와 내용이 같은 QUEST text의 ISO extent 이동",
        QUEST_EXE_ISO_PATH,
    )
    .ok_or_else(|| error("QUEST text file relocation 계획 생성 실패"))?;
    let relocation_copy_writes = copy_writes.len();
    for write in copy_writes {
        add_region_write(
            &mut plan,
            write,
            "지원 원본 QUEST text의 byte-identical ISO file relocation",
        );
    }

    for (entry, replacement, id, purpose) in [
        (
            quest_entry,
            relocated_quest,
            "quest-relocation-directory-record",
            "QUEST.EXE extent를 32M reservation 선두로 재배치",
        ),
        (
            reservation_entry,
            relocated_32m,
            "32m-split-directory-record",
            "QUEST.EXE 뒤에서 시작하도록 32M.BIN extent 축소",
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
        "quest-original-extent",
        original_extent_start..original_extent_end,
        "재배치 뒤에도 원본 QUEST raw extent는 폐기하지 않고 byte-identical 보존",
    ));

    let metadata_writes = plan
        .writes
        .iter()
        .filter(|write| write.intent.region_kind() == RegionKind::Metadata)
        .count();
    let touched_sectors = plan.touched_sectors().len();
    let final_expected_writes = plan.writes.len() + touched_sectors;
    let output = apply_form1_write_plan(source.image(), &plan)
        .map_err(|source| error(source.to_string()))?;
    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;

    let final_quest_raw = final_iso
        .read_file_at_extent(QUEST_EXE_ISO_PATH, relocated_quest)
        .map_err(|source| error(source.to_string()))?;
    let extent_overlaps = count_extent_overlaps(&extent_intervals(&final_iso)?)?;

    let final_exe = PsxExe::parse(&final_quest_raw).map_err(|source| error(source.to_string()))?;
    verify_header_identity(
        &original_exe,
        &final_exe,
        QUEST_RELOCATION_CONTROL_TEXT_BYTES,
    )?;
    let original_text_exact = final_exe.text()[..original_exe.text().len()] == *original_exe.text();
    if !original_text_exact {
        return Err(error("재배치한 QUEST 원본 text byte readback 불일치"));
    }
    let extension = &final_exe.text()[original_exe.text().len()..];
    let extension_zero = extension.iter().all(|byte| *byte == 0);
    if !extension_zero {
        return Err(error("확장 QUEST text 영역이 전량 0이 아님"));
    }
    let final_32m_raw = final_iso
        .read_file_at_extent(BOX_32M_PATH, relocated_32m)
        .map_err(|source| error(source.to_string()))?;
    if final_32m_raw.len() != relocated_32m_bytes as usize
        || final_32m_raw.iter().any(|byte| *byte != 0)
    {
        return Err(error("분할 뒤 32M.BIN 길이 또는 전량 0 계약 불일치"));
    }
    let original_extent_unchanged = source.image().raw_bytes()
        [original_extent_start as usize..original_extent_end as usize]
        == final_image.raw_bytes()[original_extent_start as usize..original_extent_end as usize];
    if !original_extent_unchanged {
        return Err(error("QUEST 원본 raw extent 무변경 경계 불일치"));
    }

    let image = final_image.into_bytes();
    let image_sha256 = format!("{:x}", Sha256::digest(&image));
    Ok(QuestExeRelocationControlBuild {
        image,
        report: QuestExeRelocationControlReport {
            schema_version: QUEST_EXE_RELOCATION_CONTROL_SCHEMA_VERSION,
            original_quest_lba: original_quest_file.lba,
            original_quest_bytes: original_quest_file.len,
            original_quest_sectors: original_quest_file.sector_count(),
            relocated_quest_lba: relocated_quest.lba,
            relocated_quest_bytes: relocated_quest.len,
            relocated_quest_sectors: QUEST_RELOCATION_CONTROL_SECTORS,
            relocated_32m_lba,
            relocated_32m_bytes,
            relocated_32m_sectors,
            metadata_writes,
            relocation_copy_writes,
            touched_sectors,
            final_expected_writes,
            original_text_exact,
            extension_zero,
            original_extent_unchanged,
            extent_overlaps,
            runtime_verified: false,
            build_eligible: true,
            image_sha256,
        },
    })
}

/// exact relocation control의 QUEST entry main RAM을 재빌드 결과와 대조한다.
pub fn verify_quest_exe_relocation_control_ram(
    source: &VerifiedSourceImage,
    ram: &[u8],
) -> Result<QuestExeRelocationRuntimeReport, QuestExeRelocationControlError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "PS1 main RAM dump 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            ram.len()
        )));
    }
    let build = build_quest_exe_relocation_control(source)?;
    let final_image =
        DiscImage::from_bytes(build.image).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    let final_quest_raw = final_iso
        .read_file(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let final_exe = PsxExe::parse(&final_quest_raw).map_err(|source| error(source.to_string()))?;
    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    let ram_offset = psx_main_ram_offset(final_exe.load_addr(), final_exe.text().len())
        .ok_or_else(|| error("확장 QUEST text가 PS1 main RAM dump 범위 밖임"))?;
    let ram_end = ram_offset
        .checked_add(final_exe.text().len())
        .ok_or_else(|| error("확장 QUEST RAM 범위 overflow"))?;
    let loaded_text = &ram[ram_offset..ram_end];
    let loaded_text_exact = loaded_text == final_exe.text();
    if !loaded_text_exact {
        return Err(error(
            "QUEST entry RAM의 확장 text가 exact control EXE와 다름",
        ));
    }
    let original_text_bytes =
        usize::try_from(map.text_len).map_err(|_| error("QUEST 원본 text 길이 변환 overflow"))?;
    let extension = loaded_text
        .get(original_text_bytes..)
        .ok_or_else(|| error("QUEST RAM text가 원본 text보다 짧음"))?;
    let extension_zero = extension.iter().all(|byte| *byte == 0);
    if !extension_zero {
        return Err(error("QUEST entry RAM의 확장 text 영역이 전량 0이 아님"));
    }

    Ok(QuestExeRelocationRuntimeReport {
        schema_version: QUEST_EXE_RELOCATION_RUNTIME_SCHEMA_VERSION,
        image_sha256: format!("{:x}", Sha256::digest(final_image.raw_bytes())),
        ram_sha256: format!("{:x}", Sha256::digest(ram)),
        load_addr: final_exe.load_addr(),
        original_text_bytes,
        loaded_text_bytes: loaded_text.len(),
        extension_bytes: extension.len(),
        expected_text_sha256: format!("{:x}", Sha256::digest(final_exe.text())),
        loaded_text_sha256: format!("{:x}", Sha256::digest(loaded_text)),
        extension_sha256: format!("{:x}", Sha256::digest(extension)),
        loaded_text_exact,
        extension_zero,
    })
}

#[cfg(test)]
#[path = "quest_exe_relocation_control_tests.rs"]
mod tests;
