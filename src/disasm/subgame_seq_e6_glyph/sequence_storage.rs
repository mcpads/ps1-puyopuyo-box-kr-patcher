//! E6 sequence resource 선택과 VDD/CNX/overlay 저장 계보 검증.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::decompress;
use crate::coords::ExeMap;
use crate::script::{SubgameSeqLayoutAnalysis, SubgameSeqLayoutKind};

use super::binary_checks::read_u32;
use super::{
    error, E6SequenceSelectionCensus, E6SequenceSelectionRecord, E6SequenceStorageCensus,
    SubgameSeqE6GlyphError, SubgameSeqE6GlyphSpec, SubgameSeqE6StorageLink, DESCRIPTOR_BYTES,
    E6_SEQUENCE_RESOURCE_FIRST, E6_SEQUENCE_RESOURCE_LAST,
};

/// `OP10/type 1`의 8바이트 SEQ 자산 선택 record를 정렬 위치에서만 찾는다.
///
/// 실행 여부나 동적 생성 record를 주장하지 않는 원본 census다. 마지막 u16은
/// 지원 원본 selector가 요구하는 reserved 0으로 제한한다.
pub(super) fn e6_sequence_selection_records(raw: &[u8]) -> Vec<(usize, u8, u16)> {
    if raw.len() < 8 {
        return Vec::new();
    }
    (0..=raw.len() - 8)
        .step_by(4)
        .filter_map(|offset| {
            let record = &raw[offset..offset + 8];
            if record[0] != 0x28 || record[1] != 0x10 || record[3] != 0x01 || record[6..8] != [0, 0]
            {
                return None;
            }
            Some((
                offset,
                record[2],
                u16::from_le_bytes([record[4], record[5]]),
            ))
        })
        .collect()
}

fn descriptor_name(
    map: &ExeMap,
    text: &[u8],
    descriptor_runtime: u32,
) -> Result<String, SubgameSeqE6GlyphError> {
    let pointer_runtime = descriptor_runtime
        .checked_add(4)
        .ok_or_else(|| error("E6 sequence descriptor pointer overflow"))?;
    let pointer_offset = map
        .text_offset(pointer_runtime)
        .ok_or_else(|| error("E6 sequence descriptor가 EXE 밖임"))?;
    let name_runtime = read_u32(text, pointer_offset)?;
    let name_offset = map
        .text_offset(name_runtime)
        .ok_or_else(|| error("E6 sequence descriptor 이름표가 EXE 밖임"))?;
    let bytes = text
        .get(name_offset..)
        .ok_or_else(|| error("E6 sequence descriptor 이름표 범위 이탈"))?;
    let end = bytes
        .iter()
        .take(32)
        .position(|byte| *byte == 0)
        .ok_or_else(|| error("E6 sequence descriptor 이름표 NUL 누락"))?;
    let name = bytes[..end]
        .iter()
        .copied()
        .map(char::from)
        .collect::<String>();
    if name.is_empty() || !name.bytes().all(|byte| (0x20..=0x7E).contains(&byte)) {
        return Err(error("E6 sequence descriptor 이름표 ASCII 불일치"));
    }
    Ok(name)
}

/// E6 overlay 이름→resource ID와 canonical `OP10/type 1` 선택 분모를 묶는다.
///
/// PUYO1의 0건은 실행 불가 증명이 아니다. initialized EXE가 runtime에 record를
/// 만들거나 다른 간접 경로를 사용할 가능성은 동적 경계로 남긴다.
pub(super) fn verify_e6_sequence_selection_census(
    map: &ExeMap,
    text: &[u8],
    sources: &BTreeMap<(&str, &str), &[u8]>,
    e6_sequences: &BTreeSet<&str>,
    spec: SubgameSeqE6GlyphSpec,
) -> Result<E6SequenceSelectionCensus, SubgameSeqE6GlyphError> {
    if e6_sequences.len() != usize::from(E6_SEQUENCE_RESOURCE_LAST - E6_SEQUENCE_RESOURCE_FIRST + 1)
    {
        return Err(error(format!(
            "{} E6 sequence/resource 분모 불일치: {}",
            spec.scope,
            e6_sequences.len()
        )));
    }
    if usize::from(E6_SEQUENCE_RESOURCE_LAST) >= spec.seq_descriptor_records {
        return Err(error(format!(
            "{} E6 sequence descriptor 범위가 선언 record 수를 넘음",
            spec.scope
        )));
    }

    let resources = (E6_SEQUENCE_RESOURCE_FIRST..=E6_SEQUENCE_RESOURCE_LAST).collect::<Vec<_>>();
    let mut descriptor_names = BTreeSet::new();
    for &resource in &resources {
        let descriptor_runtime = spec
            .seq_descriptor_base_runtime
            .checked_add(u32::from(resource) * DESCRIPTOR_BYTES)
            .ok_or_else(|| error("E6 sequence descriptor runtime overflow"))?;
        let actual_name = descriptor_name(map, text, descriptor_runtime)?;
        if !e6_sequences.contains(actual_name.as_str()) {
            return Err(error(format!(
                "{} E6 sequence resource 이름 불일치: resource=0x{resource:04X}, actual={actual_name}",
                spec.scope
            )));
        }
        descriptor_names.insert(actual_name);
    }
    let expected_names = e6_sequences
        .iter()
        .map(|name| (*name).to_string())
        .collect::<BTreeSet<_>>();
    if descriptor_names != expected_names {
        return Err(error(format!(
            "{} E6 sequence descriptor 이름 모집단 불일치",
            spec.scope
        )));
    }

    let resource_set = resources.iter().copied().collect::<BTreeSet<_>>();
    let mut records = Vec::new();
    for (&(archive, sequence), raw) in sources {
        if archive != spec.seq_archive {
            continue;
        }
        for (offset, flags, resource) in e6_sequence_selection_records(raw) {
            if resource_set.contains(&resource) {
                records.push(E6SequenceSelectionRecord {
                    sequence: sequence.to_string(),
                    offset,
                    flags,
                    resource,
                });
            }
        }
    }
    records.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.offset.cmp(&right.offset))
    });

    let mut per_resource = BTreeMap::<u16, usize>::new();
    for record in &records {
        *per_resource.entry(record.resource).or_default() += 1;
    }
    for &resource in &resources {
        let actual = per_resource.get(&resource).copied().unwrap_or(0);
        if actual != spec.expected_e6_selection_records_per_resource {
            return Err(error(format!(
                "{} E6 sequence selection multiplicity 불일치: resource=0x{resource:04X}, expected={}, actual={actual}",
                spec.scope, spec.expected_e6_selection_records_per_resource
            )));
        }
    }

    let selected_resources = per_resource.keys().copied().collect::<Vec<_>>();
    let sources = records
        .iter()
        .map(|record| record.sequence.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let flags = records
        .iter()
        .map(|record| record.flags)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let expected_sources = spec
        .expected_e6_selection_sources
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    if sources != expected_sources || flags != spec.expected_e6_selection_flags {
        return Err(error(format!(
            "{} E6 sequence selection source/flags 불일치: sources={sources:?}, flags={flags:02X?}",
            spec.scope
        )));
    }

    Ok(E6SequenceSelectionCensus {
        resources,
        records,
        selected_resources,
        sources,
        flags,
    })
}

/// E6 resource가 가리키는 EXE descriptor와 VDD/CNX/overlay 원본을 묶는다.
///
/// resource 번호와 VDD 물리 index는 같은 순서가 아니다. descriptor 이름을
/// 키로 유일한 VDD 엔트리를 찾고 실제 index를 증거로 남긴다. 실행·상주·표시는
/// 이 정적 저장 경계 밖이며 별도 runtime 증거가 필요하다.
pub(super) fn verify_e6_sequence_storage_census(
    map: &ExeMap,
    text: &[u8],
    archive: &VddArchive,
    sources: &BTreeMap<(&str, &str), &[u8]>,
    layouts: &SubgameSeqLayoutAnalysis,
    resources: &[u16],
    spec: SubgameSeqE6GlyphSpec,
) -> Result<E6SequenceStorageCensus, SubgameSeqE6GlyphError> {
    if archive.entry_count() + 1 != spec.seq_descriptor_records {
        return Err(error(format!(
            "{} SEQ VDD/descriptor 분모 불일치: entries={}, descriptors={}",
            spec.scope,
            archive.entry_count(),
            spec.seq_descriptor_records
        )));
    }

    let mut stored_bytes = 0usize;
    let mut plain_bytes = 0usize;
    let mut load_bases = BTreeSet::new();
    let mut links = Vec::new();
    for &resource in resources {
        let descriptor_runtime = spec
            .seq_descriptor_base_runtime
            .checked_add(u32::from(resource) * DESCRIPTOR_BYTES)
            .ok_or_else(|| error("E6 sequence descriptor runtime overflow"))?;
        let descriptor_offset = map
            .text_offset(descriptor_runtime)
            .ok_or_else(|| error("E6 sequence descriptor가 EXE 밖임"))?;
        let name = descriptor_name(map, text, descriptor_runtime)?;
        let (vdd_index, entry) = archive
            .entries()
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.name() == name)
            .ok_or_else(|| {
                error(format!(
                    "{} E6 sequence descriptor 이름의 VDD 엔트리 누락: resource=0x{resource:04X}, name={name}",
                    spec.scope
                ))
            })?;

        let compressed = archive.read_file(entry);
        let plain = decompress(compressed).map_err(|source| {
            error(format!(
                "{}:{} E6 sequence CNX 해제 실패: {source}",
                spec.seq_archive,
                entry.name()
            ))
        })?;
        let source = sources
            .get(&(spec.seq_archive, entry.name()))
            .ok_or_else(|| error("E6 sequence 해제 원본 map 누락"))?;
        if plain.as_slice() != *source {
            return Err(error(format!(
                "{}:{} E6 sequence CNX/source 바이트 불일치",
                spec.seq_archive,
                entry.name()
            )));
        }

        let expected_words = [
            0x0001_0000,
            read_u32(text, descriptor_offset + 4)?,
            entry.size,
            u32::try_from(plain.len()).map_err(|_| error("E6 sequence 해제 길이가 u32를 넘음"))?,
            0,
            0,
            0,
            0,
            0,
        ];
        let mut actual_words = [0u32; 9];
        for (index, word) in actual_words.iter_mut().enumerate() {
            *word = read_u32(text, descriptor_offset + index * 4)?;
        }
        if actual_words != expected_words {
            return Err(error(format!(
                "{}:{} E6 sequence descriptor 불일치: expected={expected_words:08X?}, actual={actual_words:08X?}",
                spec.scope,
                entry.name()
            )));
        }

        let layout = layouts
            .rows
            .iter()
            .find(|layout| layout.archive == spec.seq_archive && layout.sequence == entry.name())
            .ok_or_else(|| error("E6 sequence overlay layout 누락"))?;
        if layout.kind != SubgameSeqLayoutKind::Overlay
            || layout.bytes != plain.len()
            || layout.sequence_sha256 != <[u8; 32]>::from(Sha256::digest(&plain))
        {
            return Err(error(format!(
                "{}:{} E6 sequence overlay 원본 계약 불일치",
                spec.seq_archive,
                entry.name()
            )));
        }
        let footer = layout
            .footer
            .ok_or_else(|| error("E6 sequence overlay footer 누락"))?;
        load_bases.insert(footer.load_base);
        stored_bytes = stored_bytes
            .checked_add(compressed.len())
            .ok_or_else(|| error("E6 sequence 저장 길이 합 overflow"))?;
        plain_bytes = plain_bytes
            .checked_add(plain.len())
            .ok_or_else(|| error("E6 sequence 해제 길이 합 overflow"))?;
        links.push(SubgameSeqE6StorageLink {
            resource,
            vdd_index,
            sequence: entry.name().to_string(),
            stored_bytes: compressed.len(),
            plain_bytes: plain.len(),
            sequence_sha256: layout.sequence_sha256,
            load_base: footer.load_base,
            initialized_bytes: usize::try_from(footer.initialized_len)
                .map_err(|_| error("E6 sequence initialized 길이 변환 실패"))?,
            bss_bytes: usize::try_from(footer.bss_len)
                .map_err(|_| error("E6 sequence BSS 길이 변환 실패"))?,
        });
    }

    let expected = resources.len();
    if links.len() != expected {
        return Err(error(format!(
            "{} E6 sequence storage link 분모 불일치: expected={expected}, actual={}",
            spec.scope,
            links.len()
        )));
    }
    Ok(E6SequenceStorageCensus {
        vdd_entries: links.len(),
        descriptor_rows: links.len(),
        cnx_payloads: links.len(),
        overlay_rows: links.len(),
        stored_bytes,
        plain_bytes,
        load_bases: load_bases.into_iter().collect(),
        links,
    })
}
