use super::*;

#[test]
fn synthetic_record는_op21_mode2와_mutable_pointer를_분리한다() {
    assert_eq!(
        SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES[..4],
        [0x21, 0x02, 0x00, 0x00]
    );
    assert_eq!(SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES[4..8], [0; 4]);
    assert_eq!(
        count_canonical_mode2_records(&SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES),
        1
    );
}

#[test]
fn raw_pointer_scan은_비정렬_reference도_숨기지_않는다() {
    let mut raw = vec![0; 16];
    raw[3..7].copy_from_slice(&0x8002_9A14u32.to_le_bytes());
    assert_eq!(count_raw_pointer(&raw, 0x8002_9A14), 1);
}

#[test]
fn expanded_jalr_source는_direct_rooted_outside_text를_분리한다() {
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x100,
        entry_pc: 0x8001_0000,
        file: crate::coords::LogicalFile::new(0, 0x900),
    };
    let mut text = vec![0; map.text_len as usize];
    text[0x80..0x84].copy_from_slice(&0x8001_0060u32.to_le_bytes());
    text[0x84..0x88].copy_from_slice(&0x8001_0090u32.to_le_bytes());
    text[0x94..0x98].copy_from_slice(&0x8001_0064u32.to_le_bytes());
    let instruction = |offset: u32, kind: K| Instruction::new(map.load_addr + offset, 0, kind);
    let instructions = vec![
        instruction(
            0x00,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        instruction(
            0x04,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0x80,
            },
        ),
        instruction(
            0x08,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        instruction(
            0x0C,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        instruction(
            0x10,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0x84,
            },
        ),
        instruction(
            0x14,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        instruction(
            0x18,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        instruction(
            0x1C,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8002,
            },
        ),
        instruction(
            0x20,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        instruction(
            0x24,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
    ];
    let mut expanded = super::super::Trace::default();
    for &index in &[2usize, 6, 9] {
        expanded
            .instructions
            .insert(instructions[index].addr, instructions[index]);
        expanded.indirect_sites.insert(instructions[index].addr);
    }

    let sources = classify_expanded_jalr_sources(&map, &text, &instructions, &expanded)
        .expect("synthetic jalr source 분류");
    assert_eq!(
        sources.initialized_direct_sources,
        BTreeMap::from([((0x8001_0080, 0x8001_0060), vec![0x8001_0008])])
    );
    assert_eq!(
        sources.initialized_rooted_sources,
        BTreeMap::from([((0x8001_0094, 0x8001_0064), vec![0x8001_0018])])
    );
    assert_eq!(
        sources.outside_text_sources,
        BTreeMap::from([(0x8002_0000, vec![0x8001_0024])])
    );
    assert!(sources.unclassified_sites.is_empty());
}

#[test]
fn expanded_jalr_computed_source는_runtime_field_load를_보존한다() {
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x40,
        entry_pc: 0x8001_0000,
        file: crate::coords::LogicalFile::new(0, 0x840),
    };
    let instruction = |offset: u32, kind: K| Instruction::new(map.load_addr + offset, 0, kind);
    let instructions = vec![
        instruction(
            0x00,
            K::Addu {
                rd: Reg::S0,
                rs: Reg::A0,
                rt: Reg::A1,
            },
        ),
        instruction(
            0x04,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        instruction(
            0x08,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
    ];
    let mut expanded = super::super::Trace::default();
    expanded
        .instructions
        .insert(instructions[2].addr, instructions[2]);
    expanded.indirect_sites.insert(instructions[2].addr);

    let sources = classify_expanded_jalr_sources(&map, &[0; 0x40], &instructions, &expanded)
        .expect("runtime field jalr source 분류");
    assert_eq!(
        sources.computed_memory_sources,
        vec![SubgameSeqTextJalrRuntimeFieldSource {
            call_runtime: 0x8001_0008,
            load_runtime: 0x8001_0004,
            target_reg: Reg::V0,
            base_reg: Reg::S0,
            field_offset: 0x18,
        }]
    );
}

#[test]
fn outside_text_source는_direct_read_write와_address_escape를_분리한다() {
    let instruction = |offset: u32, kind: K| Instruction::new(0x8001_0000 + offset, 0, kind);
    let instructions = vec![
        instruction(
            0x00,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8002,
            },
        ),
        instruction(
            0x04,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        instruction(
            0x08,
            K::Lui {
                rt: Reg::AT,
                imm: 0x8002,
            },
        ),
        instruction(
            0x0C,
            K::Sw {
                rt: Reg::A0,
                base: Reg::AT,
                offset: 0,
            },
        ),
        instruction(
            0x10,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8002,
            },
        ),
        instruction(
            0x14,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0,
            },
        ),
    ];
    let sources = analyze_outside_text_sources(
        &instructions,
        BTreeMap::from([(0x8002_0000, vec![0x8001_0020])]),
    );
    assert_eq!(
        sources,
        vec![SubgameSeqTextJalrRuntimeSource {
            source_runtime: 0x8002_0000,
            call_sites: vec![0x8001_0020],
            direct_read_sites: vec![0x8001_0004],
            direct_write_sites: vec![0x8001_000C],
            address_value_sites: vec![0x8001_0014],
        }]
    );
}

#[test]
fn expanded_jalr_source는_bios_forwarded_unclassified를_분리한다() {
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x40,
        entry_pc: 0x8001_0000,
        file: crate::coords::LogicalFile::new(0, 0x840),
    };
    let instruction = |offset: u32, kind: K| Instruction::new(map.load_addr + offset, 0, kind);
    let instructions = vec![
        instruction(
            0x00,
            K::Addiu {
                rt: Reg::T2,
                rs: Reg::ZERO,
                imm: 0xB0,
            },
        ),
        instruction(
            0x04,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::T2,
            },
        ),
        instruction(
            0x08,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::S6,
            },
        ),
        instruction(
            0x0C,
            K::Addiu {
                rt: Reg::T2,
                rs: Reg::ZERO,
                imm: 7,
            },
        ),
        instruction(
            0x10,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::T2,
            },
        ),
    ];
    let mut expanded = super::super::Trace::default();
    for &index in &[1usize, 2, 4] {
        expanded
            .instructions
            .insert(instructions[index].addr, instructions[index]);
        expanded.indirect_sites.insert(instructions[index].addr);
    }

    let sources = classify_expanded_jalr_sources(&map, &[0; 0x40], &instructions, &expanded)
        .expect("synthetic jalr source 분류");
    assert_eq!(sources.bios_vector_sites, vec![0x8001_0004]);
    assert_eq!(sources.forwarded_register_sites, vec![0x8001_0008]);
    assert_eq!(sources.unclassified_sites, vec![0x8001_0010]);
}

#[test]
fn forwarded_jalr_source는_함수인자와_direct_caller_target을_연결한다() {
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x100,
        entry_pc: 0x8001_0000,
        file: crate::coords::LogicalFile::new(0, 0x900),
    };
    let instruction = |offset: u32, kind: K| Instruction::new(map.load_addr + offset, 0, kind);
    let instructions = vec![
        instruction(
            0x00,
            K::Lui {
                rt: Reg::A2,
                imm: 0x8001,
            },
        ),
        instruction(
            0x04,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::A2,
                imm: 0x0080,
            },
        ),
        instruction(
            0x08,
            K::Jal {
                target: 0x8001_0040,
            },
        ),
        instruction(
            0x0C,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        instruction(
            0x10,
            K::Lui {
                rt: Reg::A2,
                imm: 0x8001,
            },
        ),
        instruction(
            0x14,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::A2,
                imm: 0x0090,
            },
        ),
        instruction(
            0x18,
            K::Jal {
                target: 0x8001_0040,
            },
        ),
        instruction(
            0x1C,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        instruction(
            0x40,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -24,
            },
        ),
        instruction(
            0x44,
            K::Addu {
                rd: Reg::S6,
                rs: Reg::A2,
                rt: Reg::ZERO,
            },
        ),
        instruction(
            0x48,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::S6,
            },
        ),
    ];

    let sources = analyze_forwarded_jalr_sources(
        &map,
        &[0; 0x100],
        &instructions,
        &[0x8001_0048],
        0x8001_00A0,
    )
    .expect("forwarded jalr direct caller 분석");
    assert_eq!(
        sources,
        vec![SubgameSeqTextJalrForwardedSource {
            call_runtime: 0x8001_0048,
            target_reg: Reg::S6,
            capture_runtime: 0x8001_0044,
            function_entry_runtime: 0x8001_0040,
            source_argument_reg: Reg::A2,
            direct_call_sites: vec![0x8001_0008, 0x8001_0018],
            direct_call_targets: vec![0x8001_0080, 0x8001_0090],
            entry_address_load_refs: 0,
            entry_raw_pointer_refs: 0,
            constructor_targets: 0,
        }]
    );
}

#[test]
fn expanded_jalr_indexed_table은_target_분모와_constructor를_센다() {
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x100,
        entry_pc: 0x8001_0000,
        file: crate::coords::LogicalFile::new(0, 0x900),
    };
    let mut text = vec![0; map.text_len as usize];
    for (index, target) in [0x8001_0040u32, 0, 0x8002_0000, 0x8001_0060]
        .into_iter()
        .enumerate()
    {
        let offset = 0x80 + index * 4;
        text[offset..offset + 4].copy_from_slice(&target.to_le_bytes());
    }

    let source =
        inspect_initialized_jalr_table(&map, &text, 0x8001_0020, 0x8001_0080, 4, 0x8001_0060)
            .expect("synthetic indexed table 검산");
    assert_eq!(source.table_entries, 4);
    assert_eq!(source.unique_targets, 4);
    assert_eq!(source.zero_targets, 1);
    assert_eq!(source.outside_exe_targets, 1);
    assert_eq!(source.constructor_targets, 1);
}

#[test]
fn stack_arg5_jalr_scan은_callee_save_slot을_제외한다() {
    let instruction = |offset: u32, kind: K| Instruction::new(0x8001_0000 + offset, 0, kind);
    let candidate = vec![
        instruction(
            0,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -32,
            },
        ),
        instruction(
            4,
            K::Sw {
                rt: Reg::RA,
                base: Reg::SP,
                offset: 28,
            },
        ),
        instruction(
            8,
            K::Sw {
                rt: Reg::A0,
                base: Reg::SP,
                offset: 16,
            },
        ),
        instruction(
            12,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        instruction(
            16,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: 32,
            },
        ),
    ];
    assert_eq!(find_stack_arg5_jalr_candidates(&candidate).len(), 1);

    let mut callee_save = candidate;
    callee_save[2].kind = K::Sw {
        rt: Reg::S0,
        base: Reg::SP,
        offset: 16,
    };
    callee_save.insert(
        4,
        instruction(
            16,
            K::Lw {
                rt: Reg::S0,
                base: Reg::SP,
                offset: 16,
            },
        ),
    );
    callee_save[5].addr = 0x8001_0014;
    assert!(find_stack_arg5_jalr_candidates(&callee_save).is_empty());
}

#[test]
fn stack_arg5_jalr_scan은_직전_call뒤_write와_delay_slot만_현재인자로_센다() {
    let instruction = |offset: u32, kind: K| Instruction::new(0x8001_0000 + offset, 0, kind);
    let prologue = [
        instruction(
            0,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -32,
            },
        ),
        instruction(
            4,
            K::Sw {
                rt: Reg::RA,
                base: Reg::SP,
                offset: 28,
            },
        ),
    ];
    let stale = vec![
        prologue[0],
        prologue[1],
        instruction(
            8,
            K::Sw {
                rt: Reg::A0,
                base: Reg::SP,
                offset: 16,
            },
        ),
        instruction(
            12,
            K::Jal {
                target: 0x8001_1000,
            },
        ),
        instruction(
            16,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        instruction(
            20,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        instruction(
            24,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: 32,
            },
        ),
    ];
    assert!(find_stack_arg5_jalr_candidates(&stale).is_empty());

    let delay = vec![
        prologue[0],
        prologue[1],
        instruction(
            8,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        instruction(
            12,
            K::Sw {
                rt: Reg::A0,
                base: Reg::SP,
                offset: 16,
            },
        ),
        instruction(
            16,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: 32,
            },
        ),
    ];
    assert_eq!(
        find_stack_arg5_jalr_candidates(&delay),
        vec![StackArg5JalrCandidate {
            call_runtime: 0x8001_0008,
            target_reg: Reg::V0,
            argument_store_runtime: 0x8001_000C,
        }]
    );
}

#[test]
fn entry_jr_table은_index_bound와_absolute_word_source를_함께_요구한다() {
    let instruction = |offset: u32, kind: K| Instruction::new(0x8001_0000 + offset, 0, kind);
    let mut instructions = vec![
        instruction(
            0,
            K::Sltiu {
                rt: Reg::T0,
                rs: Reg::A0,
                imm: 3,
            },
        ),
        instruction(
            4,
            K::Beq {
                rs: Reg::T0,
                rt: Reg::ZERO,
                offset: 8,
            },
        ),
        instruction(
            8,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 2,
            },
        ),
        instruction(
            12,
            K::Lui {
                rt: Reg::AT,
                imm: 0x8001,
            },
        ),
        instruction(
            16,
            K::Addu {
                rd: Reg::AT,
                rs: Reg::AT,
                rt: Reg::V0,
            },
        ),
        instruction(
            20,
            K::Lw {
                rt: Reg::V0,
                base: Reg::AT,
                offset: 0x100,
            },
        ),
        instruction(
            24,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        instruction(28, K::Jr { rs: Reg::V0 }),
    ];
    assert_eq!(
        jr_table_source(&instructions, 7, Reg::V0),
        Some((0x8001_0100, 3))
    );

    instructions[1].kind = K::Bne {
        rs: Reg::T0,
        rt: Reg::ZERO,
        offset: 8,
    };
    assert_eq!(jr_table_source(&instructions, 7, Reg::V0), None);
}

#[test]
fn entry_jr_bios_vector는_a0_b0_c0_veneer만_받는다() {
    let instruction = |offset: u32, kind: K| Instruction::new(0x8001_0000 + offset, 0, kind);
    let mut instructions = vec![
        instruction(
            0,
            K::Addiu {
                rt: Reg::T2,
                rs: Reg::ZERO,
                imm: 0xB0,
            },
        ),
        instruction(4, K::Jr { rs: Reg::T2 }),
    ];
    assert!(is_bios_vector_jr(&instructions, 1, Reg::T2));
    instructions[0].kind = K::Addiu {
        rt: Reg::T2,
        rs: Reg::ZERO,
        imm: 0x90,
    };
    assert!(!is_bios_vector_jr(&instructions, 1, Reg::T2));
}

#[test]
fn jr_table_seed는_exe안_정렬_target과_밖_target을_분리한다() {
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 16,
        entry_pc: 0x8001_0000,
        file: crate::coords::LogicalFile::new(0, 16),
    };
    let mut text = Vec::new();
    text.extend_from_slice(&0x8001_0008u32.to_le_bytes());
    text.extend_from_slice(&0x0000_0000u32.to_le_bytes());
    text.extend_from_slice(&0xA001_000Cu32.to_le_bytes());
    text.extend_from_slice(&0u32.to_le_bytes());
    let source = SubgameSeqTextJrTableSource {
        call_runtime: 0x8001_1000,
        table_runtime: 0x8001_0000,
        table_entries: 3,
        constructor_targets: 0,
    };
    let (inside, outside) = collect_jr_table_targets(&map, &text, &[source]).unwrap();
    assert_eq!(inside, BTreeSet::from([0x8001_0008, 0x8001_000C]));
    assert_eq!(outside, BTreeSet::from([0]));
}

#[test]
fn report는_내부_bridge와_caller_runtime을_분리한다() {
    let analysis = SubgameSeqTextBridgeAnalysis {
        rows: vec![SubgameSeqTextBridgeRow {
            exe: "PUYO1.EXE",
            archive: "/PUYO1/PP1_SEQ.VDD;1",
            constructor_runtime: 1,
            synthetic_record_runtime: 2,
            dispatcher_runtime: 3,
            dispatcher_table_runtime: 4,
            op21_handler_runtime: 5,
            op21_mode2_handler_runtime: 6,
            object_consumer_runtime: 7,
            object_consumer_source_runtime: 8,
            exe_direct_flow_refs: 0,
            exe_address_load_refs: 0,
            exe_raw_pointer_refs: 0,
            sequence_files_scanned: 324,
            sequence_direct_jal_refs: 0,
            sequence_direct_jump_refs: 0,
            sequence_address_load_refs: 0,
            sequence_raw_pointer_refs: 0,
            constructor_entry_reached: false,
            entry_indirect_sites: 67,
            entry_jr_bios_sites: vec![12],
            entry_jr_table_sources: vec![SubgameSeqTextJrTableSource {
                call_runtime: 13,
                table_runtime: 14,
                table_entries: 2,
                constructor_targets: 0,
            }],
            expanded_trace: SubgameSeqTextExpandedTrace {
                iterations: 2,
                table_target_seeds: 3,
                table_targets_outside_exe: 0,
                reached_instructions: 100,
                indirect_sites: 4,
                jalr_sites: 2,
                jr_bios_sites: 1,
                jr_table_sites: 1,
                jr_unclassified_sites: Vec::new(),
                jalr_bios_vector_sites: vec![15],
                jalr_initialized_direct_sources: vec![SubgameSeqTextJalrInitializedSource {
                    source_runtime: 16,
                    initialized_target: 17,
                    call_sites: vec![18],
                }],
                jalr_initialized_rooted_sources: Vec::new(),
                jalr_initialized_indexed_sources: Vec::new(),
                jalr_outside_text_sources: Vec::new(),
                jalr_computed_memory_sources: Vec::new(),
                jalr_forwarded_register_sites: Vec::new(),
                jalr_forwarded_register_sources: Vec::new(),
                jalr_unclassified_sites: Vec::new(),
                jalr_fresh_stack_arg5_sites: Vec::new(),
                constructor_reached: false,
            },
            exe_stack_arg5_jalr_sites: 1,
            stack_arg5_jalr: vec![SubgameSeqTextIndirectCandidateSpec {
                call_runtime: 9,
                argument_store_runtime: 10,
                table_runtime: 11,
                table_entries: 48,
                table_unique_targets: 30,
                table_zero_targets: 0,
                table_outside_exe_targets: 0,
                table_sha256: [0; 32],
                table_kind: "text-control-ff00-ff2f",
            }],
            indirect_table_constructor_targets: vec![0],
            exe_canonical_mode2_records: 1,
            sequence_canonical_mode2_records: 0,
            drac_relation: "distinct-from-drac-e0-no-static-link",
        }],
        sequence_files: 324,
    };
    let report = render_subgame_seq_text_bridges_tsv(&analysis);
    assert!(report.contains("fifth-arg-to-single-synthetic-op21-and-object-stream-static"));
    assert!(report.contains(
        "initialized-jalr-targets-exclude-constructor-runtime-source-and-arg5-unresolved"
    ));
    assert!(report.contains("\t1\t0\t"));
    assert!(report.contains("runtime-unverified"));
}
