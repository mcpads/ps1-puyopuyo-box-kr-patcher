use super::*;
use crate::coords::LogicalFile;

#[test]
fn object_zero_helper는_word_단위_반복을_검증한다() {
    let runtime = 0x8001_0000;
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: runtime,
        text_len: 32,
        entry_pc: runtime,
        file: LogicalFile::new(0, 32),
    };
    let kinds = [
        K::Beq {
            rs: Reg::A2,
            rt: Reg::ZERO,
            offset: 6,
        },
        K::Addu {
            rd: Reg::V1,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Sw {
            rt: Reg::A1,
            base: Reg::A0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: 1,
        },
        K::Sltu {
            rd: Reg::V0,
            rs: Reg::V1,
            rt: Reg::A2,
        },
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: -4,
        },
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 4,
        },
        K::Jr { rs: Reg::RA },
    ];
    let mut instructions = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| Instruction::new(runtime + index as u32 * 4, 0, kind))
        .collect::<Vec<_>>();
    verify_zero_words_function(&instructions, &map, runtime).unwrap();
    instructions[2].kind = K::Sb {
        rt: Reg::A1,
        base: Reg::A0,
        offset: 0,
    };
    assert!(verify_zero_words_function(&instructions, &map, runtime).is_err());
}

#[test]
fn descriptor_field_access는_base_접근과_pointer_escape를_분리한다() {
    let runtime = 0x8001_0000;
    let instructions = [
        Instruction::new(
            runtime,
            0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x10,
            },
        ),
        Instruction::new(
            runtime + 4,
            0,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x14,
            },
        ),
        Instruction::new(
            runtime + 8,
            0,
            K::Sw {
                rt: Reg::S1,
                base: Reg::V0,
                offset: 4,
            },
        ),
        Instruction::new(
            runtime + 12,
            0,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S1,
                offset: 0x20,
            },
        ),
    ];
    let (reads, writes) =
        p1_memory_accesses_with_base(&instructions, runtime, runtime + 16, Reg::S1);
    assert_eq!(reads, vec![runtime, runtime + 12]);
    assert_eq!(writes, vec![runtime + 4]);
}

#[test]
fn e6_sequence_selection은_정렬된_op10_type1_reserved0만_고른다() {
    let mut raw = vec![0; 36];
    raw[4..12].copy_from_slice(&[0x28, 0x10, 0x97, 0x01, 0x3F, 0x00, 0x00, 0x00]);
    raw[12..20].copy_from_slice(&[0x28, 0x10, 0x80, 0x00, 0x40, 0x00, 0x00, 0x00]);
    raw[20..28].copy_from_slice(&[0x28, 0x10, 0x81, 0x01, 0x41, 0x00, 0x01, 0x00]);
    raw[29..36].copy_from_slice(&[0x28, 0x10, 0x82, 0x01, 0x42, 0x00, 0x00]);
    assert_eq!(e6_sequence_selection_records(&raw), vec![(4, 0x97, 0x003F)]);
}

#[test]
fn p2_cell_table은_256개_16x16_cell을_검증한다() {
    let mut raw = vec![0; P2_CELL_TABLE_BYTES];
    for (offset, value) in [(4, 512u16), (6, 256u16)] {
        raw[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    for index in 0..BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_CELLS {
        let offset = CELL_HEADER_BYTES + index * CELL_BYTES;
        for (delta, value) in [
            (0, u16::try_from(index % 16).unwrap() * 16),
            (2, u16::try_from(index / 16).unwrap() * 16),
            (4, 16),
            (6, 16),
        ] {
            raw[offset + delta..offset + delta + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    assert_eq!(hex(&Sha256::digest(&raw)), P2_TABLE_SHA256);
    assert_eq!(verify_p2_table(&raw, 0).unwrap(), P2_TABLE_SHA256);
    raw[CELL_HEADER_BYTES + 255 * CELL_BYTES + 4] = 0;
    assert!(verify_p2_table(&raw, 0).is_err());
}

#[test]
fn setup_record는_0x97_create만_고른다() {
    let mut raw = vec![0; 48];
    raw[4..8].copy_from_slice(&SETUP_PREFIX);
    raw[8..10].copy_from_slice(&0x88u16.to_le_bytes());
    raw[16..20].copy_from_slice(&0x801B_5280u32.to_le_bytes());
    raw[24..28].copy_from_slice(&[0x28, 0x12, 0x88, 0]);
    let records = setup_records(&raw, raw.len());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].offset, 4);
    assert_eq!(records[0].resource, 0x88);
    assert_eq!(records[0].reserved, 0);
    assert_eq!(records[0].pointer, 0x801B_5280);
}

#[test]
fn setup_record_검색은_짧은_입력에서_부분_record를_만들지_않는다() {
    assert!(setup_records(&[0x28, 0x12, 0x97], 3).is_empty());
    assert!(setup_records(&[0x28, 0x12, 0x97], 32).is_empty());
}

#[test]
fn cell_rect는_atlas_경계를_넘으면_거부한다() {
    assert!(valid_rect((0, 0, 16, 16, 0, 0), 16, 16));
    assert!(!valid_rect((1, 0, 16, 16, 0, 0), 16, 16));
    assert!(!valid_rect((0, 0, 0, 16, 0, 0), 16, 16));
}

#[test]
fn paired_writer는_같은_base의_cell_count_store만_고른다() {
    let kinds = [
        K::Sw {
            rt: Reg::V0,
            base: Reg::S1,
            offset: 0x1C,
        },
        K::Lw {
            rt: Reg::V1,
            base: Reg::S0,
            offset: 0,
        },
        K::Sh {
            rt: Reg::V0,
            base: Reg::S1,
            offset: 0x20,
        },
        K::Sw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 0x1C,
        },
        K::Sh {
            rt: Reg::A1,
            base: Reg::SP,
            offset: 0x20,
        },
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x1C,
        },
        K::Sh {
            rt: Reg::ZERO,
            base: Reg::V0,
            offset: 0x20,
        },
    ];
    let instructions = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| Instruction::new(0x8001_0000 + index as u32 * 4, 0, kind))
        .collect::<Vec<_>>();
    assert_eq!(
        paired_descriptor_cell_count_writers(&instructions),
        vec![DescriptorCellCountWriter {
            cell_store_runtime: 0x8001_0000,
            count_store_runtime: 0x8001_0008,
            base: Reg::S1,
            cell_source: Reg::V0,
            count_source: Reg::V0,
        }]
    );
}

#[test]
fn literal_halfword_20_전수는_stack과_다른_width를_제외한다() {
    let kinds = [
        K::Sh {
            rt: Reg::V0,
            base: Reg::S1,
            offset: 0x20,
        },
        K::Sh {
            rt: Reg::V0,
            base: Reg::SP,
            offset: 0x20,
        },
        K::Sw {
            rt: Reg::V0,
            base: Reg::S1,
            offset: 0x20,
        },
        K::Sh {
            rt: Reg::V0,
            base: Reg::S1,
            offset: 0x1E,
        },
        K::Sh {
            rt: Reg::A0,
            base: Reg::T0,
            offset: 0x20,
        },
    ];
    let instructions = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| Instruction::new(0x8001_0000 + index as u32 * 4, 0, kind))
        .collect::<Vec<_>>();
    assert_eq!(
        literal_non_stack_halfword_20_stores(&instructions),
        vec![0x8001_0000, 0x8001_0010]
    );
}

#[test]
fn literal_resource_006d는_addiu와_ori_zero만_센다() {
    let kinds = [
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 0x006D,
        },
        K::Ori {
            rt: Reg::A0,
            rs: Reg::ZERO,
            imm: 0x006D,
        },
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V0,
            imm: 0x006D,
        },
        K::Ori {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: 0x006D,
        },
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 0x006C,
        },
    ];
    let instructions = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| Instruction::new(0x8001_0000 + index as u32 * 4, 0, kind))
        .collect::<Vec<_>>();
    assert_eq!(
        literal_resource_006d_loads(&instructions),
        vec![0x8001_0000, 0x8001_0004]
    );
}

#[test]
fn async_dispatch는_branch별_incoming_a0_소비를_찾는다() {
    let instructions = [
        Instruction::new(
            0x8001_0000,
            0,
            K::Beq {
                rs: Reg::A1,
                rt: Reg::ZERO,
                offset: 3,
            },
        ),
        Instruction::new(
            0x8001_0004,
            0,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        Instruction::new(
            0x8001_0008,
            0,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        Instruction::new(0x8001_000C, 0, K::Jr { rs: Reg::RA }),
        Instruction::new(
            0x8001_0010,
            0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0,
            },
        ),
    ];
    assert_eq!(
        p1_async_dispatch_incoming_a0_uses(
            &instructions,
            &[0x8001_0000],
            0x8001_0000,
            0x8001_0014,
        )
        .unwrap(),
        vec![0x8001_0010]
    );
}

#[test]
fn async_dispatch_call_delay에서_a0를_덮으면_전달하지_않는다() {
    let instructions = [
        Instruction::new(
            0x8001_0000,
            0,
            K::Jal {
                target: 0x8002_0000,
            },
        ),
        Instruction::new(
            0x8001_0004,
            0,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
    ];
    assert!(p1_async_dispatch_incoming_a0_uses(
        &instructions,
        &[0x8001_0000],
        0x8001_0000,
        0x8001_0008,
    )
    .unwrap()
    .is_empty());
}

#[test]
fn gpu_primitive_판정은_같은_base의_좌표_sibling을_요구한다() {
    let offsets = [0x08, 0x0A, 0x10, 0x12, 0x18, 0x1A, 0x20, 0x22];
    let instructions = offsets
        .into_iter()
        .enumerate()
        .map(|(index, offset)| {
            Instruction::new(
                0x8001_0000 + index as u32 * 4,
                0,
                K::Sh {
                    rt: Reg::V0,
                    base: Reg::T0,
                    offset,
                },
            )
        })
        .collect::<Vec<_>>();
    let center = 6;
    for offset in [0x08, 0x0A, 0x10, 0x12, 0x18, 0x1A, 0x22] {
        assert!(has_nearby_halfword_store(
            &instructions,
            center,
            Reg::T0,
            offset
        ));
        assert!(!has_nearby_halfword_store(
            &instructions,
            center,
            Reg::T1,
            offset
        ));
    }
}

#[test]
fn p1_candidate는_사용_index의_무효와_빈_pixel을_분리한다() {
    let mut table = vec![0; CELL_HEADER_BYTES + 3 * CELL_BYTES];
    for (offset, value) in [(4, 1008u16), (6, 232u16)] {
        table[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    for (index, rect) in [(0, (0u16, 0u16, 1u16, 1u16)), (1, (1, 0, 1, 1))] {
        let offset = CELL_HEADER_BYTES + index * CELL_BYTES;
        for (delta, value) in [(0, rect.0), (2, rect.1), (4, rect.2), (6, rect.3)] {
            table[offset + delta..offset + delta + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    let third = CELL_HEADER_BYTES + 2 * CELL_BYTES;
    for (delta, value) in [(0, 1008u16), (2, 0), (4, 1), (6, 1)] {
        table[third + delta..third + delta + 2].copy_from_slice(&value.to_le_bytes());
    }
    let mut glyph = vec![0; 1008 * 232 / 2];
    glyph[0] = 1;
    let coverage =
        verify_p1_candidate_coverage(&table, 0, &BTreeSet::from([0, 1, 2]), &glyph).unwrap();
    assert_eq!(
        coverage,
        P1CandidateCoverage {
            valid_cells: 2,
            invalid_indices: vec![2],
            nonempty_cells: 1,
            empty_indices: vec![1],
        }
    );
}
