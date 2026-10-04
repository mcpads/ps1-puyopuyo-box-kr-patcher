use super::*;

fn test_repertoire() -> Vec<u8> {
    (0u8..56).flat_map(|index| [0x82, 0x40 + index]).collect()
}

fn test_record(index: usize, bytes: usize) -> Vec<u8> {
    assert!(bytes >= 6 && (bytes - 6).is_multiple_of(2));
    let index_byte = u8::try_from(index).unwrap();
    let mut record = vec![0x10 + index_byte, 0x20, 0x30, 0x40];
    for glyph in 0..((bytes - 6) / 2) {
        let glyph = u8::try_from((index + glyph) % 56).unwrap();
        record.extend_from_slice(&[0x82, 0x40 + glyph]);
    }
    record.extend_from_slice(&TERMINATOR);
    record
}

fn drac_repack_fixture() -> (
    Vec<u8>,
    SubgameSeqDracTextAnalysis,
    SubgameSeqDracRelinkAnalysis,
    Vec<Vec<u8>>,
) {
    let spec = BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS[0];
    let mut raw = vec![0; spec.footer_offset + SUBGAME_SEQ_OVERLAY_FOOTER_LEN];
    let repertoire = test_repertoire();
    raw[spec.repertoire_offset..spec.repertoire_offset + REPERTOIRE_BYTES]
        .copy_from_slice(&repertoire);
    raw[spec.repertoire_offset + REPERTOIRE_BYTES..spec.message_start_offset]
        .copy_from_slice(&TERMINATOR);

    let record_bytes = vec![14, 60, 50, 60, 56];
    let records = record_bytes
        .iter()
        .enumerate()
        .map(|(index, &bytes)| test_record(index, bytes))
        .collect::<Vec<_>>();
    let mut offsets = Vec::new();
    let mut cursor = spec.message_start_offset;
    for record in &records {
        offsets.push(cursor);
        raw[cursor..cursor + record.len()].copy_from_slice(record);
        cursor += record.len();
    }
    assert_eq!(cursor, spec.original_message_end_offset);

    let targets = std::iter::once(spec.repertoire_offset)
        .chain(offsets.iter().copied())
        .collect::<Vec<_>>();
    for (&field, &target) in spec.pointer_fields.iter().zip(&targets) {
        let target = LOAD_BASE + u32::try_from(target).unwrap();
        raw[field..field + 4].copy_from_slice(&target.to_le_bytes());
    }

    let make_text_row =
        |kind, index, field, offset, bytes: Vec<u8>| crate::script::SubgameSeqDracTextRow {
            scope: spec.scope,
            archive: spec.archive,
            sequence: spec.sequence,
            kind,
            index,
            pointer_field_offset: field,
            pointer_field_runtime: LOAD_BASE + u32::try_from(field).unwrap(),
            record_offset: offset,
            record_runtime: LOAD_BASE + u32::try_from(offset).unwrap(),
            raw_sha256: Sha256::digest(&bytes).into(),
            raw: bytes,
            spans: Vec::new(),
            protected_bytes: 0,
            kana_candidate_overlaps: 0,
            missed_sjis_spans: 0,
        };
    let mut text_rows = vec![make_text_row(
        SubgameSeqDracTextKind::Repertoire,
        0,
        spec.pointer_fields[0],
        spec.repertoire_offset,
        repertoire,
    )];
    for (index, ((&field, &offset), record)) in spec.pointer_fields[1..]
        .iter()
        .zip(&offsets)
        .zip(&records)
        .enumerate()
    {
        text_rows.push(make_text_row(
            SubgameSeqDracTextKind::Message,
            index + 1,
            field,
            offset,
            record.clone(),
        ));
    }
    let drac_text = SubgameSeqDracTextAnalysis {
        rows: text_rows,
        message_records: 5,
        message_spans: 0,
        message_code_units: 0,
        message_protected_bytes: 0,
        repertoires: 1,
        repertoire_code_units: 56,
        kana_candidates_in_messages: 0,
        kana_candidates_in_repertoires: 0,
        missed_message_spans: 0,
    };

    let raw_len = u32::try_from(raw.len()).unwrap();
    let map = RuntimeAssetMap::new(LOAD_BASE, raw_len).unwrap();
    let pointers = scan_region_pointers(
        &raw,
        spec.footer_offset,
        map,
        spec.repertoire_offset,
        spec.capacity_end_offset,
    )
    .unwrap();
    let protected_tail = &raw[spec.capacity_end_offset..spec.footer_offset];
    let relinks = SubgameSeqDracRelinkAnalysis {
        rows: vec![SubgameSeqDracRelinkRow {
            scope: spec.scope,
            archive: spec.archive,
            sequence: spec.sequence,
            repertoire_offset: spec.repertoire_offset,
            repertoire_bytes: REPERTOIRE_BYTES,
            message_start_offset: spec.message_start_offset,
            original_message_end_offset: spec.original_message_end_offset,
            capacity_end_offset: spec.capacity_end_offset,
            original_message_bytes: BOX_SUBGAME_SEQ_DRAC_RELINK_ORIGINAL_MESSAGE_BYTES,
            capacity_bytes: BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_CAPACITY_BYTES,
            slack_bytes: BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_SLACK_BYTES,
            max_extra_two_byte_glyphs: BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_EXTRA_TWO_BYTE_GLYPHS,
            record_bytes,
            pointers,
            footer_offset: spec.footer_offset,
            protected_tail_bytes: protected_tail.len(),
            protected_tail_sha256: hex(&Sha256::digest(protected_tail)),
        }],
        common_capacity_bytes: BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_CAPACITY_BYTES,
        common_slack_bytes: BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_SLACK_BYTES,
        common_extra_two_byte_glyphs: BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_EXTRA_TWO_BYTE_GLYPHS,
    };
    (raw, drac_text, relinks, records)
}

#[test]
fn pointer_scan은_정렬되지_않은_참조와_영역내부_target도_숨기지_않는다() {
    let mut raw = vec![0; 64];
    raw[4..8].copy_from_slice(&0x801B_5280u32.to_le_bytes());
    raw[13..17].copy_from_slice(&0x801B_5285u32.to_le_bytes());
    let map = RuntimeAssetMap::new(LOAD_BASE, raw.len() as u32).unwrap();
    let pointers = scan_region_pointers(&raw, raw.len(), map, 0x10, 0x20).unwrap();
    assert_eq!(
        pointers
            .iter()
            .map(|pointer| (pointer.field_offset, pointer.target_offset))
            .collect::<Vec<_>>(),
        vec![(4, 0x10), (13, 0x15)]
    );
}

#[test]
fn 두_사본의_공통상한은_더_작은_zero_reserve를_쓴다() {
    assert_eq!(BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_CAPACITY_BYTES, 270);
    assert_eq!(BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_SLACK_BYTES, 30);
    assert_eq!(BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_EXTRA_TWO_BYTE_GLYPHS, 15);
}

#[test]
fn report는_정적_relink와_번역_runtime미검증을_분리한다() {
    let analysis = SubgameSeqDracRelinkAnalysis {
        rows: vec![SubgameSeqDracRelinkRow {
            scope: "PUYO1",
            archive: "/PUYO1/PP1_SEQ.VDD;1",
            sequence: "DRAC_SEQ.SEQ",
            repertoire_offset: 1,
            repertoire_bytes: 112,
            message_start_offset: 2,
            original_message_end_offset: 3,
            capacity_end_offset: 4,
            original_message_bytes: 240,
            capacity_bytes: 270,
            slack_bytes: 30,
            max_extra_two_byte_glyphs: 15,
            record_bytes: vec![14, 60, 50, 60, 56],
            pointers: vec![SubgameSeqDracRelinkPointer {
                field_offset: 5,
                field_runtime: 6,
                target_offset: 1,
                target_runtime: 7,
            }],
            footer_offset: 8,
            protected_tail_bytes: 9,
            protected_tail_sha256: "00".repeat(32),
        }],
        common_capacity_bytes: 270,
        common_slack_bytes: 30,
        common_extra_two_byte_glyphs: 15,
    };
    let report = render_subgame_seq_drac_relinks_tsv(&analysis);
    assert!(report.contains("packed-records-five-pointer-relink-within-zero-reserve-static"));
    assert!(report.contains("translation-ux-runtime-unverified"));
}

#[test]
fn drac_repack은_공통상한에서_재포장하고_다섯_pointer만_갱신한다() {
    let (raw, drac_text, relinks, mut replacements) = drac_repack_fixture();
    for (index, record) in replacements.iter_mut().enumerate() {
        let insert_at = record.len() - TERMINATOR.len();
        let glyph = u8::try_from(index).unwrap();
        record.splice(insert_at..insert_at, [0x82, 0x40 + glyph]);
    }
    let repack =
        repack_box_subgame_seq_drac_messages("PUYO1", &raw, &replacements, &drac_text, &relinks)
            .unwrap();
    assert_eq!(repack.used_bytes, 250);
    assert_eq!(repack.remaining_bytes, 20);
    assert_eq!(repack.record_bytes, vec![16, 62, 52, 62, 58]);

    let spec = BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS[0];
    let mut expected_offset = spec.message_start_offset;
    for (index, record) in replacements.iter().enumerate() {
        assert_eq!(repack.record_offsets[index], expected_offset);
        assert_eq!(
            &repack.sequence[expected_offset..expected_offset + record.len()],
            record
        );
        assert_eq!(
            read_u32(&repack.sequence, spec.pointer_fields[index + 1]),
            Some(LOAD_BASE + u32::try_from(expected_offset).unwrap())
        );
        expected_offset += record.len();
    }
    let common_end = spec.message_start_offset + BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_CAPACITY_BYTES;
    assert!(repack.sequence[expected_offset..common_end]
        .iter()
        .all(|&byte| byte == 0));
    assert_eq!(
        &repack.sequence[spec.capacity_end_offset..],
        &raw[spec.capacity_end_offset..]
    );
    assert_eq!(
        read_u32(&repack.sequence, spec.pointer_fields[0]),
        read_u32(&raw, spec.pointer_fields[0])
    );
}

#[test]
fn drac_repack은_미등록glyph_header_control과_용량초과를_거부한다() {
    let (raw, drac_text, relinks, records) = drac_repack_fixture();

    let mut unknown_glyph = records.clone();
    unknown_glyph[0][4..6].copy_from_slice(&[0x99, 0x99]);
    assert!(repack_box_subgame_seq_drac_messages(
        "PUYO1",
        &raw,
        &unknown_glyph,
        &drac_text,
        &relinks,
    )
    .unwrap_err()
    .to_string()
    .contains("repertoire에 없음"));

    let mut changed_header = records.clone();
    changed_header[0][0] ^= 1;
    assert!(repack_box_subgame_seq_drac_messages(
        "PUYO1",
        &raw,
        &changed_header,
        &drac_text,
        &relinks,
    )
    .unwrap_err()
    .to_string()
    .contains("header 변경"));

    let mut changed_control = records.clone();
    let insert_at = changed_control[0].len() - TERMINATOR.len();
    changed_control[0].insert(insert_at, 0x72);
    assert!(repack_box_subgame_seq_drac_messages(
        "PUYO1",
        &raw,
        &changed_control,
        &drac_text,
        &relinks,
    )
    .unwrap_err()
    .to_string()
    .contains("control sequence 변경"));

    let mut overflow = records.clone();
    let insert_at = overflow[0].len() - TERMINATOR.len();
    overflow[0].splice(insert_at..insert_at, [0x82, 0x40].repeat(16));
    assert!(
        repack_box_subgame_seq_drac_messages("PUYO1", &raw, &overflow, &drac_text, &relinks,)
            .unwrap_err()
            .to_string()
            .contains("capacity 초과")
    );
}

#[test]
fn drac_repack은_원본record와_보호tail변경을_거부한다() {
    let (raw, drac_text, relinks, records) = drac_repack_fixture();
    let spec = BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS[0];

    let mut changed_record_source = raw.clone();
    changed_record_source[spec.message_start_offset + 4] ^= 1;
    assert!(repack_box_subgame_seq_drac_messages(
        "PUYO1",
        &changed_record_source,
        &records,
        &drac_text,
        &relinks,
    )
    .unwrap_err()
    .to_string()
    .contains("원본 전제 불일치"));

    let mut changed_tail = raw;
    changed_tail[spec.capacity_end_offset] = 1;
    assert!(repack_box_subgame_seq_drac_messages(
        "PUYO1",
        &changed_tail,
        &records,
        &drac_text,
        &relinks,
    )
    .unwrap_err()
    .to_string()
    .contains("보호 tail digest 불일치"));
}
