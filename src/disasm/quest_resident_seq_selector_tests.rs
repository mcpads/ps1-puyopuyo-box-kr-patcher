use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn selection_fixture() -> QuestResidentSeqRootSelection {
    let handlers = [
        0x8007_3E9C,
        0x8007_3EA8,
        0x8007_3EE0,
        0x8007_3EF0,
        0x8007_3F08,
        0x8007_3FAC,
        0x8007_4060,
        0x8007_4130,
    ];
    QuestResidentSeqRootSelection {
        state_base_runtime: 0x800F_BDD0,
        selector_runtime: QUEST_RESIDENT_SELECTOR_RUNTIME,
        dispatch_table_runtime: 0x8001_3050,
        handlers,
        farewell_selector: 4,
        farewell_handler_runtime: handlers[4],
        farewell_stream_store_runtime: 0x8007_3FA8,
        farewell_root_runtime: 0x8008_97DC,
        defeat_selector: QUEST_RESIDENT_DEFEAT_SELECTOR,
        defeat_handler_runtime: handlers[5],
        defeat_stream_store_runtime: 0x8007_4048,
        defeat_root_runtime: 0x8008_C498,
    }
}

fn write_instruction(map: &ExeMap, text: &mut [u8], runtime: u32, kind: K) {
    let offset = map.text_offset(runtime).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
}

fn selector_fixture() -> (&'static ExeMap, Vec<u8>, QuestResidentSeqRootSelection) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    for (runtime, register) in [
        (0x8005_359C, Reg::A1),
        (0x8005_4468, Reg::A1),
        (0x8005_4498, Reg::V0),
        (0x8005_4DDC, Reg::V0),
        (0x8005_5270, Reg::V0),
        (0x8005_72B4, Reg::V0),
        (0x8005_803C, Reg::V0),
        (0x8007_3F08, Reg::V0),
        (0x8007_3FAC, Reg::V0),
        (0x8007_4088, Reg::V0),
    ] {
        write_instruction(
            map,
            &mut text,
            runtime,
            K::Lui {
                rt: register,
                imm: 0x8010,
            },
        );
    }
    for (runtime, register) in [(0x8005_5274, Reg::V0), (0x8005_8040, Reg::V0)] {
        write_instruction(
            map,
            &mut text,
            runtime,
            K::Lbu {
                rt: Reg::V0,
                base: register,
                offset: QUEST_RESIDENT_SELECTOR_RUNTIME as u16 as i16,
            },
        );
    }
    for spec in SELECTOR_WRITE_SPECS {
        if let (QuestResidentSeqSelectorWriteValue::Constant(value), Some(load_runtime)) =
            (spec.value, spec.constant_load_runtime)
        {
            write_instruction(
                map,
                &mut text,
                load_runtime,
                K::Addiu {
                    rt: spec.source_register,
                    rs: Reg::ZERO,
                    imm: i16::from(value),
                },
            );
        }
        write_instruction(
            map,
            &mut text,
            spec.store_runtime,
            K::Sb {
                rt: spec.source_register,
                base: spec.base_register,
                offset: QUEST_RESIDENT_SELECTOR_RUNTIME as u16 as i16,
            },
        );
    }
    for (runtime, kind) in [
        (
            0x8007_1CC8,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S3,
                imm: 0x00FF,
            },
        ),
        (
            0x8007_1CD0,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8007_1CD4,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 3,
            },
        ),
        (
            0x8007_1CD8,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8005,
            },
        ),
        (
            0x8007_1CDC,
            K::J {
                target: 0x8007_1CEC,
            },
        ),
        (
            0x8007_1CE0,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x4744,
            },
        ),
        (
            0x8007_1CE4,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8005,
            },
        ),
        (
            0x8007_1CE8,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x5F44,
            },
        ),
        (
            0x8007_1CEC,
            K::Jal {
                target: 0x8001_4B14,
            },
        ),
        (
            0x8007_1CF0,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 4,
            },
        ),
    ] {
        write_instruction(map, &mut text, runtime, kind);
    }
    for spec in DEFEAT_WRITER_SPECS {
        for (runtime, kind) in [
            (
                spec.predicate_call_runtime,
                K::Jal {
                    target: QUEST_RESIDENT_SELECTOR_PREDICATE_RUNTIME,
                },
            ),
            (
                spec.predicate_mask_runtime,
                K::Andi {
                    rt: Reg::V0,
                    rs: Reg::V0,
                    imm: 0x00FF,
                },
            ),
            (
                spec.predicate_branch_runtime,
                K::Beq {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: spec.predicate_branch_offset,
                },
            ),
            (
                spec.prerequisite_lui_runtime,
                K::Lui {
                    rt: Reg::V0,
                    imm: 0x8010,
                },
            ),
            (
                spec.prerequisite_load_runtime,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::V0,
                    offset: QUEST_RESIDENT_DEFEAT_PREREQUISITE_RUNTIME as u16 as i16,
                },
            ),
            (
                spec.prerequisite_branch_runtime,
                K::Bne {
                    rs: Reg::V0,
                    rt: Reg::ZERO,
                    offset: spec.prerequisite_branch_offset,
                },
            ),
            (
                spec.selector_lui_runtime,
                K::Lui {
                    rt: Reg::V0,
                    imm: 0x8010,
                },
            ),
        ] {
            write_instruction(map, &mut text, runtime, kind);
        }
    }
    (map, text, selection_fixture())
}

#[test]
fn 값_5의_두_writer를_다른_selector_access와_분리한다() {
    let (map, text, selection) = selector_fixture();

    let summary =
        verify_box_quest_resident_seq_selector(map, &text, &selection).expect("selector 검증");

    assert_eq!(summary.reads, EXPECTED_SELECTOR_READS);
    assert_eq!(summary.writes.len(), 8);
    assert_eq!(
        summary
            .writes
            .iter()
            .filter(|write| write.value == QuestResidentSeqSelectorWriteValue::Constant(5))
            .map(|write| write.store_runtime)
            .collect::<Vec<_>>(),
        [0x8005_4DE4, 0x8005_72BC]
    );
    assert_eq!(summary.defeat_writers.len(), 2);
    assert_eq!(summary.defeat_writers[0].callback_runtime, 0x8005_4744);
    assert_eq!(summary.defeat_writers[1].callback_runtime, 0x8005_5F44);
    assert!(summary.standalone_direct_control_refs.is_empty());
    assert!(summary.standalone_address_load_refs.is_empty());
    assert!(summary.standalone_aligned_pointer_refs.is_empty());
}

#[test]
fn defeat_selector_constant_drift를_거부한다() {
    let (map, mut text, selection) = selector_fixture();
    write_instruction(
        map,
        &mut text,
        0x8005_4DE0,
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 6,
        },
    );

    let failure = verify_box_quest_resident_seq_selector(map, &text, &selection)
        .expect_err("constant drift 거부");

    assert!(failure.to_string().contains("constant load drift"));
}

#[test]
fn 미등록_selector_writer를_거부한다() {
    let (map, mut text, selection) = selector_fixture();
    write_instruction(
        map,
        &mut text,
        0x8001_1000,
        K::Lui {
            rt: Reg::A0,
            imm: 0x8010,
        },
    );
    write_instruction(
        map,
        &mut text,
        0x8001_1004,
        K::Sb {
            rt: Reg::ZERO,
            base: Reg::A0,
            offset: QUEST_RESIDENT_SELECTOR_RUNTIME as u16 as i16,
        },
    );

    let failure = verify_box_quest_resident_seq_selector(map, &text, &selection)
        .expect_err("추가 writer 거부");

    assert!(failure.to_string().contains("direct access 분모 불일치"));
}

#[test]
fn standalone_constructor의_새_aligned_pointer를_거부한다() {
    let (map, mut text, selection) = selector_fixture();
    text[..4].copy_from_slice(&QUEST_RESIDENT_STANDALONE_DEFEAT_CONSTRUCTOR_RUNTIME.to_le_bytes());

    let failure = verify_box_quest_resident_seq_selector(map, &text, &selection)
        .expect_err("새 direct pointer 거부");

    assert!(failure.to_string().contains("direct ref 분모 drift"));
}
