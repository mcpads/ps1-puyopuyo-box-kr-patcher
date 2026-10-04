//! VDD 제자리 교체를 raw 이미지 Expected Write로 옮기는 공통 경계.

use crate::archive::VddArchive;
use crate::coords::LogicalFile;
use crate::disc::IsoEntry;
use crate::patch::{plan_logical_write, ImageRegion, WriteIntent, WritePlan};

pub(crate) fn logical_file(entry: &IsoEntry) -> Result<LogicalFile, String> {
    let [extent] = entry.extents() else {
        return Err(format!(
            "{}: 제자리 쓰기는 단일 extent만 지원: actual={}",
            entry.path(),
            entry.extents().len()
        ));
    };
    if u64::from(extent.len) != entry.len() {
        return Err(format!(
            "{}: extent 길이와 ISO 파일 길이 불일치",
            entry.path()
        ));
    }
    Ok(LogicalFile::new(extent.lba, extent.len))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn add_logical_span(
    plan: &mut WritePlan,
    file: LogicalFile,
    logical_offset: usize,
    expected: &[u8],
    replacement: &[u8],
    id: &str,
    actor: &str,
    purpose: &str,
    intent: WriteIntent,
) -> Result<(), String> {
    if expected == replacement {
        return Ok(());
    }
    let writes = plan_logical_write(
        file,
        logical_offset as u64,
        expected,
        replacement,
        id,
        actor,
        purpose,
        |_| intent.clone(),
    )
    .ok_or_else(|| format!("{id}: 논리 파일 쓰기 계획 생성 실패"))?;
    for write in writes {
        plan.regions.push(ImageRegion::new(
            format!("region:{}", write.id),
            write.range(),
            write.intent.region_kind(),
            "pipeline이 논리 파일 쓰기에서 계산한 정확한 raw 이미지 범위",
        ));
        plan.writes.push(write);
    }
    Ok(())
}

/// VDD 엔트리 하나의 size field와 새 payload 범위를 계획에 추가한다.
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_vdd_entry_replacement(
    plan: &mut WritePlan,
    file: LogicalFile,
    archive: &VddArchive,
    original: &[u8],
    replacement: &[u8],
    entry_name: &str,
    replacement_size: usize,
    id: &str,
    actor: &str,
) -> Result<(), String> {
    if original.len() != replacement.len() || original.len() != file.len as usize {
        return Err(format!("{id}: VDD 또는 ISO 파일 길이 불일치"));
    }
    let index = archive
        .entries()
        .iter()
        .position(|entry| entry.name().eq_ignore_ascii_case(entry_name))
        .ok_or_else(|| format!("{id}: VDD entry 없음: {entry_name}"))?;
    let entry = &archive.entries()[index];
    let size_offset = 4 + index * 24 + 20;
    add_logical_span(
        plan,
        file,
        size_offset,
        &original[size_offset..size_offset + 4],
        &replacement[size_offset..size_offset + 4],
        &format!("{id}:size"),
        actor,
        "VDD 엔트리의 CNX 압축 크기 갱신",
        WriteIntent::Metadata,
    )?;
    let payload_offset = entry.byte_offset();
    let payload_end = payload_offset
        .checked_add(replacement_size)
        .ok_or_else(|| format!("{id}: payload 범위 overflow"))?;
    add_logical_span(
        plan,
        file,
        payload_offset,
        &original[payload_offset..payload_end],
        &replacement[payload_offset..payload_end],
        &format!("{id}:payload"),
        actor,
        "재압축한 CNX payload 제자리 교체",
        WriteIntent::Data,
    )
}

/// 고정 크기 VDD 재패킹의 표와 모든 새 payload를 계획에 추가한다.
pub(crate) fn add_vdd_repack(
    plan: &mut WritePlan,
    file: LogicalFile,
    original: &[u8],
    replacement: &[u8],
    id: &str,
    actor: &str,
) -> Result<(), String> {
    if original.len() != replacement.len() || original.len() != file.len as usize {
        return Err(format!("{id}: VDD 또는 ISO 파일 길이 불일치"));
    }
    let archive =
        VddArchive::parse(replacement).map_err(|source| format!("{id}: 재패킹 VDD: {source}"))?;
    let table_end = 4usize
        .checked_add(
            archive
                .entry_count()
                .checked_mul(24)
                .ok_or_else(|| format!("{id}: VDD 표 길이 overflow"))?,
        )
        .ok_or_else(|| format!("{id}: VDD 표 끝 overflow"))?;
    add_logical_span(
        plan,
        file,
        0,
        &original[..table_end],
        &replacement[..table_end],
        &format!("{id}:table"),
        actor,
        "VDD 재패킹 엔트리 offset·size 표",
        WriteIntent::Metadata,
    )?;

    for (index, entry) in archive.entries().iter().enumerate() {
        let start = entry.byte_offset();
        let end = start
            .checked_add(entry.size as usize)
            .ok_or_else(|| format!("{id}:{} payload 범위 overflow", entry.name()))?;
        if start == end {
            continue;
        }
        add_logical_span(
            plan,
            file,
            start,
            &original[start..end],
            &replacement[start..end],
            &format!("{id}:payload-{index:04}"),
            actor,
            "VDD 재패킹 후 엔트리 payload",
            WriteIntent::Data,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "vdd_plan_tests.rs"]
mod tests;
