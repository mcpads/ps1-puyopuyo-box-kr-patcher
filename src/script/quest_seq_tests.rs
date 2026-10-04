use super::*;

fn synthetic_source() -> Vec<u8> {
    let mut raw = vec![0u8; TOWN00_SEQ_LEN];
    for index in 0..TOWN00_PREFIX_RECORDS {
        let offset = index * QUEST_SEQ_PREFIX_RECORD_LEN;
        raw[offset] = PREFIX_MARKER;
        raw[offset + 1] = index as u8;
        for pair in 0..5 {
            raw[offset + 2 + pair * 2] = (index * 5 + pair) as u8;
        }
    }
    raw[TOWN00_PREFIX_RECORDS * QUEST_SEQ_PREFIX_RECORD_LEN] = 0x03;
    raw
}

#[test]
fn prefix_record는_op85가_소비하는_폭으로_왕복한다() {
    let raw = [
        0x85, 0x03, 0x0D, 0x00, 0x0E, 0x00, 0x0F, 0x00, 0x17, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let record = QuestSeqPrefixRecord::parse(&raw, 1).expect("20바이트 레코드");
    assert_eq!(record.seq_resource_id(), 3);
    assert_eq!(record.map_resource_ids(), [13, 14, 15]);
    assert_eq!(record.grp_resource_ids(), [23, 8]);
    assert_eq!(record.renderer_data_pointers(), [0, 0]);
    assert_eq!(record.encode(), raw);
}

#[test]
fn marker와_trailing_zero_drift를_거부한다() {
    let mut raw = [0u8; QUEST_SEQ_PREFIX_RECORD_LEN];
    raw[0] = 0x84;
    assert_eq!(
        QuestSeqPrefixRecord::parse(&raw, 2),
        Err(QuestSeqError::InvalidPrefixMarker {
            index: 2,
            actual: 0x84,
        })
    );

    raw[0] = PREFIX_MARKER;
    raw[19] = 1;
    assert_eq!(
        QuestSeqPrefixRecord::parse(&raw, 2),
        Err(QuestSeqError::NonZeroPrefixTail {
            index: 2,
            offset: 19,
            actual: 1,
        })
    );
}

#[test]
fn source_길이와_digest가_다르면_구조를_승인하지_않는다() {
    assert_eq!(
        Town00Seq::parse_source(&[]),
        Err(QuestSeqError::SourceLength {
            expected: TOWN00_SEQ_LEN,
            actual: 0,
        })
    );

    let raw = synthetic_source();
    assert!(matches!(
        Town00Seq::parse_source(&raw),
        Err(QuestSeqError::SourceDigest { .. })
    ));
}

#[test]
fn 선언한_16개_뒤에도_같은_record가_이어지면_경계를_거부한다() {
    let mut raw = [0u8; QUEST_SEQ_PREFIX_RECORD_LEN];
    raw[0] = PREFIX_MARKER;
    assert!(looks_like_prefix_record(&raw));
    raw[12] = 1;
    assert!(!looks_like_prefix_record(&raw));
}

#[test]
fn town00_root_op03은_미소비_byte를_보존하고_내부_target으로_왕복한다() {
    let raw = [0x03, 0xA1, 0xB2, 0xC3, 0xB8, 0x9B, 0x19, 0x80];
    let map =
        RuntimeAssetMap::new(TOWN00_SEQ_RUNTIME_BASE, TOWN00_SEQ_LEN as u32).expect("적재 사상");
    let record = QuestSeqDirectBranchRecord::parse(&raw, map).expect("OP03 root");
    assert_eq!(record.unconsumed_parameters(), [0xA1, 0xB2, 0xC3]);
    assert_eq!(record.target_runtime(), 0x8019_9BB8);
    assert_eq!(record.target_offset(), 0x148);
    assert_eq!(record.encode(), raw);
}

#[test]
fn town00_root는_opcode와_target_범위_drift를_거부한다() {
    let map =
        RuntimeAssetMap::new(TOWN00_SEQ_RUNTIME_BASE, TOWN00_SEQ_LEN as u32).expect("적재 사상");
    let mut raw = [0u8; QUEST_SEQ_DIRECT_BRANCH_RECORD_LEN];
    raw[0] = 0x04;
    assert_eq!(
        QuestSeqDirectBranchRecord::parse(&raw, map),
        Err(QuestSeqError::InvalidRootOpcode { actual: 0x04 })
    );

    raw[0] = 0x03;
    raw[4..8].copy_from_slice(&(TOWN00_SEQ_RUNTIME_BASE - 4).to_le_bytes());
    assert_eq!(
        QuestSeqDirectBranchRecord::parse(&raw, map),
        Err(QuestSeqError::RootTargetOutOfRange {
            target_runtime: TOWN00_SEQ_RUNTIME_BASE - 4,
            base: TOWN00_SEQ_RUNTIME_BASE,
            len: TOWN00_SEQ_LEN as u32,
        })
    );
}

fn synthetic_town00_root_body() -> Vec<u8> {
    let mut body = vec![0u8; 0x100];
    body[0] = 0x03;
    body[4..8].copy_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 0x148).to_le_bytes());
    body[8..12].copy_from_slice(&[0x0D, 0x40, 0x04, 0x00]);
    body[12..16].copy_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 0x15C).to_le_bytes());
    body[16..20].copy_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 0x381C).to_le_bytes());
    body[0x1C..0x30].copy_from_slice(&[
        0x85, 0x00, 0x0D, 0x00, 0x0E, 0x00, 0x0F, 0x00, 0x17, 0x00, 0x08, 0x00, 0x1C, 0xEC, 0x19,
        0x80, 0x30, 0x9E, 0x19, 0x80,
    ]);
    body[0x30..0x38].copy_from_slice(&[0x88, 0x04, 0x00, 0x00, 0xC4, 0x9D, 0x19, 0x80]);
    body[0x38..0x40].copy_from_slice(&[0x8F, 0x02, 0x00, 0x00, 0xBC, 0x9D, 0x19, 0x80]);
    body
}

#[test]
fn root_op0d는_state_word와_미해석_byte까지만_header로_승인한다() {
    let body = synthetic_town00_root_body();
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    let header = seq.root_global_word_branch().expect("OP0D frontier");
    assert_eq!(header.global_state_word_index(), 0x40);
    assert_eq!(header.unconsumed_parameters(), [0x04, 0x00]);
    assert_eq!(header.encode(), [0x0D, 0x40, 0x04, 0x00]);
}

#[test]
fn root_op0d는_runtime_index가_주어졌을_때만_pointer를_해석한다() {
    let body = synthetic_town00_root_body();
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    assert_eq!(
        seq.resolve_root_global_word_branch(0),
        Ok(QuestSeqResolvedBranchTarget {
            branch_index: 0,
            pointer_field_offset: 0x14C,
            target_runtime: TOWN00_SEQ_RUNTIME_BASE + 0x15C,
            target_offset: 0x15C,
        })
    );
    assert_eq!(
        seq.resolve_root_global_word_branch(1),
        Ok(QuestSeqResolvedBranchTarget {
            branch_index: 1,
            pointer_field_offset: 0x150,
            target_runtime: TOWN00_SEQ_RUNTIME_BASE + 0x381C,
            target_offset: 0x381C,
        })
    );
    assert!(matches!(
        seq.resolve_root_global_word_branch(0x1000),
        Err(QuestSeqError::TruncatedGlobalWordBranchPointer {
            branch_index: 0x1000,
            ..
        })
    ));
    assert_eq!(
        TOWN00_CONFIRMED_POINTER_FIELDS[0],
        Town00ConfirmedPointerField {
            field_offset: 0x014C,
            target_offset: 0x015C,
            consumer_runtime: 0x8002_2970,
        }
    );
    assert_eq!(
        &TOWN00_CONFIRMED_POINTER_FIELDS[1..5],
        &[
            Town00ConfirmedPointerField {
                field_offset: 0x0168,
                target_offset: 0x51AC,
                consumer_runtime: 0x8007_832C,
            },
            Town00ConfirmedPointerField {
                field_offset: 0x016C,
                target_offset: 0x03C0,
                consumer_runtime: 0x8007_8340,
            },
            Town00ConfirmedPointerField {
                field_offset: 0x0174,
                target_offset: 0x0354,
                consumer_runtime: 0x8007_83E8,
            },
            Town00ConfirmedPointerField {
                field_offset: 0x017C,
                target_offset: 0x034C,
                consumer_runtime: 0x8007_8564,
            },
        ]
    );
}

#[test]
fn root_op0d는_파일밖_target을_거부한다() {
    let mut body = synthetic_town00_root_body();
    body[12..16].copy_from_slice(&(TOWN00_SEQ_RUNTIME_BASE - 4).to_le_bytes());
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    assert_eq!(
        seq.resolve_root_global_word_branch(0),
        Err(QuestSeqError::GlobalWordBranchTargetOutOfRange {
            branch_index: 0,
            pointer_field_offset: 0x14C,
            target_runtime: TOWN00_SEQ_RUNTIME_BASE - 4,
        })
    );
}

#[test]
fn root_index0의_본문_op85는_nonzero_renderer_pointer를_보존한다() {
    let body = synthetic_town00_root_body();
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    let node = seq.root_op85_node(0).expect("본문 OP85");
    assert_eq!(node.stream_offset, 0x15C);
    assert_eq!(node.next_stream_offset, 0x170);
    assert_eq!(node.record.seq_resource_id(), 0);
    assert_eq!(node.record.map_resource_ids(), [13, 14, 15]);
    assert_eq!(node.record.grp_resource_ids(), [23, 8]);
    assert_eq!(
        node.record.renderer_data_pointers(),
        [0x8019_EC1C, 0x8019_9E30]
    );
    assert_eq!(node.renderer_target_offsets, [Some(0x51AC), Some(0x3C0)]);
    let encoded: [u8; QUEST_SEQ_PREFIX_RECORD_LEN] =
        body[0x1C..0x30].try_into().expect("20바이트 OP85 범위");
    assert_eq!(node.record.encode(), encoded);
}

#[test]
fn 본문_op85의_renderer_pointer가_town00밖이면_거부한다() {
    let mut body = synthetic_town00_root_body();
    body[0x28..0x2C].copy_from_slice(&(TOWN00_SEQ_RUNTIME_BASE - 4).to_le_bytes());
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    assert_eq!(
        seq.root_op85_node(0),
        Err(QuestSeqError::Op85RendererTargetOutOfRange {
            stream_offset: 0x15C,
            pointer_index: 0,
            target_runtime: TOWN00_SEQ_RUNTIME_BASE - 4,
        })
    );
}

#[test]
fn root_op85_다음_op88은_count와_table_전체범위로_왕복한다() {
    let body = synthetic_town00_root_body();
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    let node = seq.root_op88_node(0).expect("본문 OP88");
    assert_eq!(node.stream_offset, 0x170);
    assert_eq!(node.next_stream_offset, 0x178);
    assert_eq!(node.record.raw_entry_count(), 4);
    assert_eq!(node.record.effective_entry_count(), 4);
    assert_eq!(node.record.unconsumed_parameters(), [0, 0]);
    assert_eq!(node.record.table_runtime(), 0x8019_9DC4);
    assert_eq!(node.table_target_offset, 0x354);
    assert_eq!(node.table_end_offset, 0x364);
    let encoded: [u8; QUEST_SEQ_OP88_RECORD_LEN] =
        body[0x30..0x38].try_into().expect("8바이트 OP88 범위");
    assert_eq!(node.record.encode(), encoded);
}

#[test]
fn op88_count는_32로_clamp한다() {
    let record =
        QuestSeqOp88Record::parse(&[0x88, 0xFF, 0xA1, 0xB2, 0xC4, 0x9D, 0x19, 0x80], 0x170)
            .expect("OP88 레코드");
    assert_eq!(record.raw_entry_count(), 0xFF);
    assert_eq!(record.effective_entry_count(), 32);
    assert_eq!(record.unconsumed_parameters(), [0xA1, 0xB2]);
}

#[test]
fn op88은_opcode와_table_범위를_실패시킨다() {
    let mut body = synthetic_town00_root_body();
    body[0x30] = 0x89;
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    assert_eq!(
        seq.root_op88_node(0),
        Err(QuestSeqError::InvalidOp88Opcode {
            stream_offset: 0x170,
            actual: 0x89,
        })
    );

    body[0x30] = 0x88;
    body[0x34..0x38].copy_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 0x5760).to_le_bytes());
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    assert_eq!(
        seq.root_op88_node(0),
        Err(QuestSeqError::Op88TableTargetOutOfRange {
            stream_offset: 0x170,
            target_runtime: TOWN00_SEQ_RUNTIME_BASE + 0x5760,
            effective_entry_count: 4,
        })
    );
}

#[test]
fn root_op88_다음_op8f는_halfword_pair_table로_왕복한다() {
    let body = synthetic_town00_root_body();
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    let node = seq.root_op8f_node(0).expect("본문 OP8F");
    assert_eq!(node.stream_offset, 0x178);
    assert_eq!(node.next_stream_offset, 0x180);
    assert_eq!(node.record.raw_entry_count(), 2);
    assert_eq!(node.record.effective_entry_count(), 2);
    assert_eq!(node.record.unconsumed_parameters(), [0, 0]);
    assert_eq!(node.record.table_runtime(), 0x8019_9DBC);
    assert_eq!(node.table_target_offset, 0x34C);
    assert_eq!(node.table_end_offset, 0x354);
    let encoded: [u8; QUEST_SEQ_OP8F_RECORD_LEN] =
        body[0x38..0x40].try_into().expect("8바이트 OP8F 범위");
    assert_eq!(node.record.encode(), encoded);
}

#[test]
fn op8f_count는_16으로_clamp하고_table_범위를_실패시킨다() {
    let record =
        QuestSeqOp8fRecord::parse(&[0x8F, 0xFF, 0xA1, 0xB2, 0xBC, 0x9D, 0x19, 0x80], 0x178)
            .expect("OP8F 레코드");
    assert_eq!(record.raw_entry_count(), 0xFF);
    assert_eq!(record.effective_entry_count(), 16);
    assert_eq!(record.unconsumed_parameters(), [0xA1, 0xB2]);

    let mut body = synthetic_town00_root_body();
    body[0x3C..0x40].copy_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 0x5760).to_le_bytes());
    let seq = Town00Seq {
        prefix_records: Vec::new(),
        opaque_body: &body,
    };
    assert_eq!(
        seq.root_op8f_node(0),
        Err(QuestSeqError::Op8fTableTargetOutOfRange {
            stream_offset: 0x178,
            target_runtime: TOWN00_SEQ_RUNTIME_BASE + 0x5760,
            effective_entry_count: 2,
        })
    );
}

#[test]
fn 내부주소_후보는_정렬된_field와_target중복을_보존한다() {
    let base = 0x8000_1000u32;
    let mut raw = vec![0u8; 0x40];
    raw[0x00..0x04].copy_from_slice(&(base + 0x20).to_le_bytes());
    raw[0x04..0x08].copy_from_slice(&(base + 0x20).to_le_bytes());
    raw[0x08..0x0C].copy_from_slice(&(base + 0x04).to_le_bytes());
    raw[0x0C..0x10].copy_from_slice(&(base - 4).to_le_bytes());

    let candidates = scan_internal_address_candidates(&raw, base).unwrap();
    assert_eq!(candidates.len(), 3);
    assert_eq!(candidates[0].field_offset, 0);
    assert_eq!(candidates[0].target_offset, 0x20);
    assert_eq!(candidates[0].target_references, 2);
    assert_eq!(candidates[1].target_references, 2);
    assert_eq!(candidates[2].direction(), "backward");
    assert!(candidates
        .iter()
        .all(|candidate| candidate.runtime_consumer.is_none()));
}

#[test]
fn 내부주소_tsv는_runtime관측과_미관측을_구분한다() {
    let candidates = [
        Town00InternalAddressCandidate {
            field_offset: 0x1B34,
            field_runtime: TOWN00_SEQ_RUNTIME_BASE + 0x1B34,
            target_offset: 0x1B28,
            target_runtime: TOWN00_SEQ_RUNTIME_BASE + 0x1B28,
            target_references: 1,
            runtime_consumer: Some(0x8007_8A80),
        },
        Town00InternalAddressCandidate {
            field_offset: 0x2000,
            field_runtime: TOWN00_SEQ_RUNTIME_BASE + 0x2000,
            target_offset: 0x2100,
            target_runtime: TOWN00_SEQ_RUNTIME_BASE + 0x2100,
            target_references: 2,
            runtime_consumer: None,
        },
    ];
    let tsv = render_town00_internal_address_candidates_tsv(&candidates);
    assert!(tsv
        .contains("0x1B34\t0x8019B5A4\t0x1B28\t0x8019B598\t1\tbackward\truntime_read\t0x80078A80"));
    assert!(tsv.contains("0x2000\t0x8019BA70\t0x2100\t0x8019BB70\t2\tforward\trange_candidate\t\n"));
}
