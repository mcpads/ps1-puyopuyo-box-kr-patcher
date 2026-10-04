//! 수동 조판 QUEST SEQ를 원본 `RPG_SEQ.VDD`와 결속해 다시 구성한다.

use std::collections::BTreeMap;

use crate::archive::VddArchive;
use crate::compress::{compress, decompress, parse_header};
use crate::script::QUEST_ITEM_TEXT_SEQUENCE;

use super::{error, sha256, QuestDevelopmentDiscError};
use crate::pipeline::quest_item_text_writer::QuestItemTextWriterAnalysis;
use crate::pipeline::quest_manual_stream_writer::{
    QuestManualSequenceWrite, QuestManualStreamWriterAnalysis,
};

pub(super) struct PreparedDevelopmentSeqArchive {
    pub(super) replacement_vdd: Vec<u8>,
    pub(super) seq_vdd_repacked: bool,
    pub(super) seq_vdd_relocated: bool,
    pub(super) target_sequence_entries: usize,
    pub(super) non_target_sequence_entries: usize,
    pub(super) source_target_cnx_bytes: usize,
    pub(super) output_target_cnx_bytes: usize,
    pub(super) target_plain_exact: bool,
    pub(super) target_cnx_roundtrip_exact: bool,
    pub(super) non_target_plain_exact: bool,
    pub(super) non_target_cnx_exact: bool,
}

fn is_target(sequence: &QuestManualSequenceWrite) -> bool {
    sequence.selected_routes + sequence.retained_source_routes > 0
}

pub(super) fn prepare_development_seq_archive(
    original_vdd: &[u8],
    writer: &QuestManualStreamWriterAnalysis,
    item_text: Option<&QuestItemTextWriterAnalysis>,
) -> Result<PreparedDevelopmentSeqArchive, QuestDevelopmentDiscError> {
    let archive = VddArchive::parse(original_vdd).map_err(|source| error(source.to_string()))?;
    let sequences = writer
        .sequences
        .iter()
        .map(|sequence| (sequence.sequence.as_str(), sequence))
        .collect::<BTreeMap<_, _>>();
    if sequences.len() != writer.sequences.len() || sequences.len() != archive.entry_count() {
        return Err(error(
            "QUEST development disc의 writer/VDD 엔트리 분모 불일치",
        ));
    }

    let mut replacements = BTreeMap::new();
    let mut target_sequence_entries = 0usize;
    let mut non_target_sequence_entries = 0usize;
    let mut source_target_cnx_bytes = 0usize;
    let mut output_target_cnx_bytes = 0usize;
    let mut item_text_entries = 0usize;
    for entry in archive.entries() {
        let sequence = sequences.get(entry.name()).copied().ok_or_else(|| {
            error(format!(
                "QUEST development disc writer 엔트리 누락: {}",
                entry.name()
            ))
        })?;
        let source_cnx = archive.read_file(entry);
        let header = parse_header(source_cnx).map_err(|source| {
            error(format!(
                "{}: 원본 CNX header 검증 실패: {source}",
                entry.name()
            ))
        })?;
        let source_plain = decompress(source_cnx)
            .map_err(|source| error(format!("{}: 원본 CNX 해제 실패: {source}", entry.name())))?;
        if source_plain.len() != sequence.source_bytes
            || sha256(&source_plain) != sequence.source_sha256
        {
            return Err(error(format!(
                "{}: writer와 원본 VDD plain 결속 불일치",
                entry.name()
            )));
        }

        let item_text_output = item_text
            .filter(|_| entry.name() == QUEST_ITEM_TEXT_SEQUENCE)
            .map(|item| {
                item_text_entries += 1;
                if sequence.output != source_plain
                    || item.relink.control.source_bytes != source_plain.len()
                    || item.source_sequence_sha256 != sha256(&source_plain)
                    || item.relink.control.output_bytes != item.relink.control.output.len()
                    || item.output_sequence_sha256 != sha256(&item.relink.control.output)
                {
                    return Err(error(
                        "QUEST ITEM_DAT manual writer와 item text writer가 충돌함",
                    ));
                }
                Ok(item.relink.control.output.as_slice())
            })
            .transpose()?;
        let expected_plain = item_text_output.unwrap_or(sequence.output.as_slice());
        let target = is_target(sequence) || item_text_output.is_some();

        if target {
            target_sequence_entries += 1;
            let replacement_cnx = compress(expected_plain, header.subtype).map_err(|source| {
                error(format!("{}: 검토문 CNX 압축 실패: {source}", entry.name()))
            })?;
            let replacement_header = parse_header(&replacement_cnx).map_err(|source| {
                error(format!(
                    "{}: 검토문 CNX header 검증 실패: {source}",
                    entry.name()
                ))
            })?;
            let roundtrip_plain = decompress(&replacement_cnx).map_err(|source| {
                error(format!(
                    "{}: 검토문 CNX 재해제 실패: {source}",
                    entry.name()
                ))
            })?;
            if replacement_header.subtype != header.subtype
                || replacement_header.format_byte != header.format_byte
                || roundtrip_plain != expected_plain
            {
                return Err(error(format!(
                    "{}: 검토문 CNX roundtrip 불일치",
                    entry.name()
                )));
            }
            source_target_cnx_bytes += source_cnx.len();
            output_target_cnx_bytes += replacement_cnx.len();
            replacements.insert(entry.name().to_string(), replacement_cnx);
        } else {
            non_target_sequence_entries += 1;
            if sequence.output != source_plain {
                return Err(error(format!(
                    "{}: 비대상 writer 출력이 원본과 다름",
                    entry.name()
                )));
            }
        }
    }
    if item_text.is_some() != (item_text_entries == 1) {
        return Err(error("QUEST ITEM_DAT item text target 분모 불일치"));
    }

    let (replacement_vdd, seq_vdd_repacked, seq_vdd_relocated) =
        match archive.replace_in_place(&replacements) {
            Ok(replacement) => (replacement, false, false),
            Err(in_place_error) => match archive.repack_in_container(&replacements) {
                Ok(replacement) => (replacement, true, false),
                Err(repack_error) => {
                    let required = archive.rebuild(&replacements).map_err(|source| {
                        error(format!(
                            "QUEST development RPG_SEQ.VDD 확장 크기 계산 실패: {source}"
                        ))
                    })?;
                    let mut expanded_container = original_vdd.to_vec();
                    expanded_container.resize(required.len(), 0);
                    let expanded_archive = VddArchive::parse(&expanded_container)
                        .map_err(|source| error(source.to_string()))?;
                    let replacement = expanded_archive
                        .repack_in_container(&replacements)
                        .map_err(|source| {
                            error(format!(
                                "QUEST development RPG_SEQ.VDD 제자리/재패킹/확장 실패: \
                             in-place={in_place_error}; repack={repack_error}; expanded={source}"
                            ))
                        })?;
                    (replacement, true, true)
                }
            },
        };
    let rebuilt =
        VddArchive::parse(&replacement_vdd).map_err(|source| error(source.to_string()))?;
    let mut target_plain_exact = true;
    let mut target_cnx_roundtrip_exact = true;
    let mut non_target_plain_exact = true;
    let mut non_target_cnx_exact = true;
    for source_entry in archive.entries() {
        let sequence = sequences[source_entry.name()];
        let item_text_output = item_text
            .filter(|_| source_entry.name() == QUEST_ITEM_TEXT_SEQUENCE)
            .map(|item| item.relink.control.output.as_slice());
        let expected_plain = item_text_output.unwrap_or(sequence.output.as_slice());
        let target = is_target(sequence) || item_text_output.is_some();
        let rebuilt_entry = rebuilt.find(source_entry.name()).ok_or_else(|| {
            error(format!(
                "QUEST development 재패킹 엔트리 누락: {}",
                source_entry.name()
            ))
        })?;
        let rebuilt_cnx = rebuilt.read_file(rebuilt_entry);
        let rebuilt_plain = decompress(rebuilt_cnx).map_err(|source| {
            error(format!(
                "{}: 재패킹 CNX 해제 실패: {source}",
                source_entry.name()
            ))
        })?;
        if target {
            target_plain_exact &= rebuilt_plain == expected_plain;
            target_cnx_roundtrip_exact &= parse_header(rebuilt_cnx).is_ok();
        } else {
            non_target_plain_exact &= rebuilt_plain == sequence.output;
            non_target_cnx_exact &= rebuilt_cnx == archive.read_file(source_entry);
        }
    }
    if !target_plain_exact
        || !target_cnx_roundtrip_exact
        || !non_target_plain_exact
        || !non_target_cnx_exact
    {
        return Err(error("QUEST development VDD 재패킹 readback 불일치"));
    }

    Ok(PreparedDevelopmentSeqArchive {
        replacement_vdd,
        seq_vdd_repacked,
        seq_vdd_relocated,
        target_sequence_entries,
        non_target_sequence_entries,
        source_target_cnx_bytes,
        output_target_cnx_bytes,
        target_plain_exact,
        target_cnx_roundtrip_exact,
        non_target_plain_exact,
        non_target_cnx_exact,
    })
}
