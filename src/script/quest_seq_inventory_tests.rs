use super::*;

fn pointer(base: u32, target_offset: usize) -> [u8; 4] {
    (base + target_offset as u32).to_le_bytes()
}

#[test]
fn 정렬된_자기범위_주소와_target중복을_센다() {
    let mut raw = vec![0u8; 0x40];
    raw[0x00..0x04].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x20));
    raw[0x04..0x08].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x20));
    raw[0x08..0x0C].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x04));
    let analysis = audit_quest_seq_addresses([("TEST.SEQ", raw.as_slice())]).unwrap();

    assert_eq!(analysis.candidates.len(), 3);
    assert_eq!(
        analysis.summaries[0].runtime_base_evidence,
        QuestSeqRuntimeBaseEvidence::StaticCommonLoader
    );
    assert!(analysis
        .candidates
        .iter()
        .all(|candidate| candidate.runtime_base_evidence.base_binding_proven()));
    assert_eq!(analysis.summaries[0].targets, 2);
    assert_eq!(analysis.candidates[0].target_references, 2);
    assert_eq!(analysis.candidates[1].target_references, 2);
    assert_eq!(analysis.candidates[2].direction(), "backward");
}

#[test]
fn a1_a2_record형태와_runtime증거를_분리한다() {
    let mut raw = vec![0u8; 0x40];
    raw[0x08] = 0xA1;
    raw[0x10..0x14].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x20));
    raw[0x20] = 0xA2;
    raw[0x28..0x2C].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x30));
    let analysis = audit_quest_seq_addresses([("TOWN00.SEQ", raw.as_slice())]).unwrap();

    assert_eq!(analysis.summaries[0].pointer_records_for(0xA1), Some(1));
    assert_eq!(analysis.summaries[0].pointer_records_for(0xA2), Some(1));
    assert!(analysis
        .candidates
        .iter()
        .all(|candidate| candidate.evidence == QuestSeqAddressEvidence::StaticPointerShape));
    assert!(analysis
        .candidates
        .iter()
        .all(|candidate| candidate.runtime_consumer.is_none()));
    assert_eq!(
        analysis.candidates[0]
            .pointer_spec
            .expect("A1 사양")
            .pointer_consumer_runtime,
        0x8007_8A80
    );
    assert_eq!(
        analysis.candidates[1]
            .pointer_spec
            .expect("A2 사양")
            .pointer_consumer_runtime,
        0x8007_8B04
    );
}

#[test]
fn 등록한_pointer_record사양의_offset과조건을_적용한다() {
    let mut raw = vec![0u8; 0x120];
    let records = [
        (0x00, 0x80, 4, 0xA0, None),
        (0xA8, 0x82, 8, 0x108, Some((1, 2))),
        (0x08, 0x83, 8, 0xA4, Some((1, 5))),
        (0x14, 0x85, 12, 0xA8, None),
        (0x14, 0x85, 16, 0xAC, None),
        (0x28, 0x88, 4, 0xB0, None),
        (0xA0, 0x8F, 4, 0x100, None),
        (0x30, 0x89, 4, 0xB4, None),
        (0x38, 0x93, 4, 0xB8, None),
        (0x38, 0x93, 12, 0xBC, None),
        (0x48, 0x95, 4, 0xC0, None),
        (0x50, 0x9D, 4, 0xC4, None),
        (0x80, 0x9F, 4, 0xF0, None),
        (0x58, 0xA1, 8, 0xC8, None),
        (0x64, 0xA2, 8, 0xCC, None),
        (0x70, 0xA8, 4, 0xD0, None),
        (0x78, 0xA9, 4, 0xD4, None),
        (0x88, 0xC1, 4, 0xF4, Some((1, 1))),
        (0x90, 0xE3, 8, 0xF8, None),
    ];
    for (start, opcode, pointer_offset, target, required) in records {
        raw[start] = opcode;
        if let Some((offset, value)) = required {
            raw[start + offset] = value;
        }
        raw[start + pointer_offset..start + pointer_offset + 4]
            .copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, target));
    }
    let analysis = audit_quest_seq_addresses([("TEST.SEQ", raw.as_slice())]).unwrap();

    for spec in QUEST_SEQ_POINTER_RECORD_SPECS {
        assert_eq!(
            analysis.summaries[0].pointer_records_for_spec(spec.opcode, spec.pointer_offset),
            Some(1),
            "opcode 0x{:02X} +{}",
            spec.opcode,
            spec.pointer_offset
        );
    }
    assert_eq!(analysis.summaries[0].pointer_records_for(0x85), Some(2));
    assert!(analysis
        .candidates
        .iter()
        .all(|candidate| candidate.evidence == QuestSeqAddressEvidence::StaticPointerShape));
}

#[test]
fn op82는_지원_subtype만_child_stream_record로_센다() {
    let mut raw = vec![0u8; 0x30];
    raw[0] = 0x82;
    raw[8..12].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x20));

    raw[1] = 26;
    let supported = audit_quest_seq_addresses([("TEST.SEQ", raw.as_slice())]).unwrap();
    assert_eq!(supported.summaries[0].pointer_records_for(0x82), Some(1));

    raw[1] = 27;
    let unsupported = audit_quest_seq_addresses([("TEST.SEQ", raw.as_slice())]).unwrap();
    assert_eq!(unsupported.summaries[0].pointer_records_for(0x82), Some(0));
}

#[test]
fn opc1은_byte1이_0이면_pointer_record로_세지_않는다() {
    let mut raw = vec![0u8; 0x20];
    raw[0] = 0xC1;
    raw[4..8].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x10));
    let zero = audit_quest_seq_addresses([("TEST.SEQ", raw.as_slice())]).unwrap();
    assert_eq!(zero.summaries[0].pointer_records_for(0xC1), Some(0));

    raw[1] = 1;
    let nonzero = audit_quest_seq_addresses([("TEST.SEQ", raw.as_slice())]).unwrap();
    assert_eq!(nonzero.summaries[0].pointer_records_for(0xC1), Some(1));
}

#[test]
fn 긴_a1_record안의_op80처럼_보이는_필드를_중복으로_세지_않는다() {
    let mut raw = vec![0u8; 0x20];
    raw[0] = 0xA1;
    raw[4] = 0x80;
    raw[8..12].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x10));
    let analysis = audit_quest_seq_addresses([("TEST.SEQ", raw.as_slice())]).unwrap();

    assert_eq!(analysis.summaries[0].pointer_records_for(0x80), Some(0));
    assert_eq!(analysis.summaries[0].pointer_records_for(0xA1), Some(1));
    assert_eq!(
        analysis.candidates[0]
            .pointer_spec
            .expect("바깥 A1 record")
            .opcode,
        0xA1
    );
}

#[test]
fn 긴_a1_record안의_op93처럼_보이는_필드를_중복으로_세지_않는다() {
    let mut raw = vec![0u8; 0x20];
    raw[0] = 0xA1;
    raw[4] = 0x93;
    raw[8..12].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x10));
    let analysis = audit_quest_seq_addresses([("TEST.SEQ", raw.as_slice())]).unwrap();

    assert_eq!(analysis.summaries[0].pointer_records_for(0x93), Some(0));
    assert_eq!(analysis.summaries[0].pointer_records_for(0xA1), Some(1));
    assert_eq!(
        analysis.candidates[0]
            .pointer_spec
            .expect("바깥 A1 record")
            .opcode,
        0xA1
    );
}

#[test]
fn op89_outer가_가리킨_inner_pointer관계를_별도_보존한다() {
    let mut raw = vec![0u8; 0x60];
    raw[0] = 0x89;
    raw[4..8].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x20));
    raw[0x2C..0x30].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x40));
    raw[0x30..0x34].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x44));
    let analysis = audit_quest_seq_addresses([("TEST.SEQ", raw.as_slice())]).unwrap();

    assert_eq!(analysis.summaries[0].relational_pointer_fields, 2);
    assert_eq!(analysis.summaries[0].relational_pointer_edges, 2);
    for (field, inner_offset, consumer) in [(0x2C, 12, 0x8007_3A34), (0x30, 16, 0x8007_3A48)] {
        let candidate = analysis
            .candidates
            .iter()
            .find(|candidate| candidate.field_offset == field)
            .expect("inner 포인터 후보");
        assert_eq!(
            candidate.evidence,
            QuestSeqAddressEvidence::StaticPointerShape
        );
        assert!(candidate.pointer_spec.is_none());
        assert_eq!(
            candidate.pointer_relations,
            vec![QuestSeqPointerRelation {
                opcode: 0x89,
                outer_record_start: 0,
                outer_pointer_field: 4,
                inner_record_start: 0x20,
                inner_pointer_offset: inner_offset,
                pointer_consumer_runtime: consumer,
            }]
        );
    }
    let tsv = render_quest_seq_address_candidates_tsv(&analysis);
    assert!(tsv.contains("relation_outer_record_starts"));
    assert!(tsv.contains("0x89\t0x0000\t0x0004\t0x0020\t12\t0x80073A34\t1"));
}

#[test]
fn 입력순서와_무관한_결정적_tsv를_만든다() {
    let mut first = vec![0u8; 0x20];
    first[0..4].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x10));
    let mut second = vec![0u8; 0x20];
    second[0..4].copy_from_slice(&pointer(TOWN00_SEQ_RUNTIME_BASE, 0x18));
    let left =
        audit_quest_seq_addresses([("B.SEQ", second.as_slice()), ("A.SEQ", first.as_slice())])
            .unwrap();
    let right =
        audit_quest_seq_addresses([("A.SEQ", first.as_slice()), ("B.SEQ", second.as_slice())])
            .unwrap();

    assert_eq!(left, right);
    let tsv = render_quest_seq_address_candidates_tsv(&left);
    assert!(tsv.starts_with("sequence\truntime_base_evidence\tfield_offset\tfield_runtime"));
    assert!(tsv.find("A.SEQ").unwrap() < tsv.find("B.SEQ").unwrap());
    assert!(tsv.contains("\trange_candidate\t\n"));
    assert!(tsv.contains("A.SEQ\tstatic-common-loader\t"));
}

#[test]
fn 빈이름_빈입력과_중복을_거부한다() {
    let raw = [0u8; 4];
    assert_eq!(
        audit_quest_seq_addresses(std::iter::empty()),
        Err(QuestSeqAddressError::EmptyInput)
    );
    assert_eq!(
        audit_quest_seq_addresses([("", raw.as_slice())]),
        Err(QuestSeqAddressError::EmptyName)
    );
    assert_eq!(
        audit_quest_seq_addresses([("DUP.SEQ", raw.as_slice()), ("DUP.SEQ", raw.as_slice())]),
        Err(QuestSeqAddressError::DuplicateSequence(
            "DUP.SEQ".to_string()
        ))
    );
}
