use crate::coords::LogicalFile;
use crate::isa::encode;

use super::*;

fn fixture_map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x000A_E800,
        entry_pc: 0x800A_A6E8,
        file: LogicalFile::new(0, 0x000A_F000),
    }
}

fn write_instruction(map: &ExeMap, text: &mut [u8], runtime: u32, instruction: K) {
    let offset = map.text_offset(runtime).expect("fixture instruction");
    text[offset..offset + 4].copy_from_slice(&encode(instruction).to_le_bytes());
}

fn fixture_text(map: &ExeMap) -> Vec<u8> {
    let mut text = vec![0; map.text_len as usize];
    for spec in
        LITERAL_SPECS
            .into_iter()
            .chain(QUEST_MENU_LITERAL_SPECS.iter().map(
                |&(role, runtime, end_runtime, source_text)| LiteralSpec {
                    role: QuestUiLiteralRole::MenuText(role),
                    runtime,
                    end_runtime,
                    source_text,
                },
            ))
    {
        let start = map.text_offset(spec.runtime).expect("literal start");
        let end = map
            .text_offset(spec.end_runtime - 1)
            .and_then(|offset| offset.checked_add(1))
            .expect("literal end");
        let mut slot = encode_sjis(spec.source_text).expect("fixture SJIS");
        slot.push(0);
        slot.resize(end - start, 0);
        text[start..end].copy_from_slice(&slot);
    }
    for &(target, refs) in menu::REFERENCES {
        for &(high, low) in refs {
            write_instruction(
                map,
                &mut text,
                high,
                K::Lui {
                    rt: Reg::A1,
                    imm: (target.wrapping_add(0x8000) >> 16) as u16,
                },
            );
            write_instruction(
                map,
                &mut text,
                low,
                K::Addiu {
                    rt: Reg::A1,
                    rs: Reg::A1,
                    imm: target as u16 as i16,
                },
            );
        }
    }
    for call in QUEST_MENU_GLYPH_CALLS {
        write_instruction(
            map,
            &mut text,
            call.runtime,
            K::Jal {
                target: if call.whole_string {
                    QUEST_WHOLE_STRING_RENDERER_RUNTIME
                } else {
                    QUEST_GLYPH_RENDERER_RUNTIME
                },
            },
        );
        write_instruction(map, &mut text, call.runtime + 4, call.delay);
    }
    for (high, call, target) in [
        (0x8003_1F70, 0x8003_1F94, 0x8001_0FE8u32),
        (0x8003_2CB0, 0x8003_2CD4, 0x8001_0FE8),
        (0x8003_AE04, 0x8003_AE24, 0x8001_1200),
        (0x8003_B6EC, 0x8003_B70C, 0x8001_1218),
    ] {
        write_instruction(
            map,
            &mut text,
            high,
            K::Lui {
                rt: Reg::A2,
                imm: 0x8001,
            },
        );
        write_instruction(
            map,
            &mut text,
            high + 4,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::A2,
                imm: target as u16 as i16,
            },
        );
        write_instruction(
            map,
            &mut text,
            call,
            K::Jal {
                target: 0x8002_CF98,
            },
        );
    }
    for (at, kind) in [
        (
            0x8002_CFA8,
            K::Addu {
                rd: Reg::S4,
                rs: Reg::A2,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_D0A8,
            K::Jal {
                target: QUEST_STRING_LENGTH_RUNTIME,
            },
        ),
        (
            0x8002_D0AC,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S4,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_D0C0,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::S4,
                rt: Reg::S0,
            },
        ),
        (
            0x8002_D0C4,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
        ),
    ] {
        write_instruction(map, &mut text, at, kind);
    }
    for (runtime, instruction) in [
        (
            QUEST_WORLD_MAP_LABEL_STATE_RUNTIME,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xA4,
            },
        ),
        (
            0x8006_62EC,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 13,
            },
        ),
        (
            0x8006_62F4,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        (
            0x8006_62F8,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_WORLD_MAP_LABEL_DISPATCH_TABLE_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8006_6304,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (0x8006_630C, K::Jr { rs: Reg::V0 }),
        (
            QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
            K::Jal {
                target: QUEST_WHOLE_STRING_RENDERER_RUNTIME,
            },
        ),
        (
            QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME + 4,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
    ] {
        write_instruction(map, &mut text, runtime, instruction);
    }
    for reference in MAP_REFERENCE_SPECS {
        let target = literal_runtime(reference.role);
        write_instruction(
            map,
            &mut text,
            reference.high_runtime - 4,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: 9,
            },
        );
        write_instruction(
            map,
            &mut text,
            reference.high_runtime,
            K::Lui {
                rt: Reg::A1,
                imm: (target >> 16) as u16,
            },
        );
        if let Some(runtime) = reference.jump_runtime {
            write_instruction(
                map,
                &mut text,
                runtime,
                K::J {
                    target: QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
                },
            );
        }
        write_instruction(
            map,
            &mut text,
            reference.low_runtime,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: target as u16 as i16,
            },
        );
    }
    for spec in CHOICE_REFERENCE_SPECS {
        for (high, register, target, call) in [
            (
                spec.yes_high_runtime,
                spec.yes_register,
                QUEST_UI_CHOICE_YES_RUNTIME,
                spec.yes_glyph_call_runtime,
            ),
            (
                spec.no_high_runtime,
                spec.no_register,
                QUEST_UI_CHOICE_NO_RUNTIME,
                spec.no_glyph_call_runtime,
            ),
        ] {
            write_instruction(
                map,
                &mut text,
                high,
                K::Lui {
                    rt: Reg::V0,
                    imm: 0x800C,
                },
            );
            write_instruction(
                map,
                &mut text,
                high + 4,
                K::Addiu {
                    rt: register,
                    rs: Reg::V0,
                    imm: target as u16 as i16,
                },
            );
            write_instruction(
                map,
                &mut text,
                high + 12,
                K::Jal {
                    target: QUEST_STRING_LENGTH_RUNTIME,
                },
            );
            write_instruction(
                map,
                &mut text,
                high + 16,
                K::Addu {
                    rd: Reg::A0,
                    rs: register,
                    rt: Reg::ZERO,
                },
            );
            write_instruction(
                map,
                &mut text,
                call,
                K::Jal {
                    target: QUEST_GLYPH_RENDERER_RUNTIME,
                },
            );
            write_instruction(
                map,
                &mut text,
                call + 4,
                K::Addiu {
                    rt: Reg::S0,
                    rs: Reg::S0,
                    imm: 2,
                },
            );
        }
        write_instruction(
            map,
            &mut text,
            spec.next_line_runtime,
            K::Jal {
                target: QUEST_TEXT_WINDOW_NEXT_LINE_RUNTIME,
            },
        );
        write_instruction(
            map,
            &mut text,
            spec.next_line_runtime + 4,
            K::Addu {
                rd: Reg::S0,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        );
    }
    text
}

#[test]
fn 지명_12개와_공용_선택지_4쌍을_한_source_gate로_묶는다() {
    let map = fixture_map();
    let summary = verify_box_quest_ui_literals(&map, &fixture_text(&map)).expect("UI literals");
    assert_eq!(summary.world_map_labels, 12);
    assert_eq!(summary.binary_choices, 2);
    assert_eq!(summary.binary_choice_reference_pairs, 4);
    assert_eq!(summary.binary_choice_direct_glyph_call_runtimes.len(), 8);
    assert_eq!(
        summary.world_map_state_values,
        [0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12]
    );
}

#[test]
fn 한_지명의_padding까지_바뀌면_원본_gate가_거부한다() {
    let map = fixture_map();
    let mut text = fixture_text(&map);
    let offset = map.text_offset(0x8001_237B).expect("padding");
    text[offset] = 1;
    let error = verify_box_quest_ui_literals(&map, &text).expect_err("slot drift");
    assert!(error.to_string().contains("원본 slot"));
}

#[test]
fn 선택지_참조가_하나_늘면_전수_분모가_거부한다() {
    let map = fixture_map();
    let mut text = fixture_text(&map);
    write_instruction(
        &map,
        &mut text,
        0x8005_0000,
        K::Lui {
            rt: Reg::V0,
            imm: 0x800C,
        },
    );
    write_instruction(
        &map,
        &mut text,
        0x8005_0004,
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::V0,
            imm: QUEST_UI_CHOICE_YES_RUNTIME as u16 as i16,
        },
    );
    let error = verify_box_quest_ui_literals(&map, &text).expect_err("extra reference");
    assert!(error.to_string().contains("정적 참조 분모"));
}

#[test]
fn literal_raw_pointer가_하나_늘면_전수_분모가_거부한다() {
    let map = fixture_map();
    let mut text = fixture_text(&map);
    let offset = map.text_offset(0x8005_0001).expect("raw pointer");
    text[offset..offset + 4].copy_from_slice(&QUEST_UI_CHOICE_YES_RUNTIME.to_le_bytes());
    let error = verify_box_quest_ui_literals(&map, &text).expect_err("raw pointer");
    assert!(error.to_string().contains("yes_raw"));
}

#[test]
fn 메뉴_glyph_call과_popup_caller_확장은_검산없이_허용하지_않는다() {
    let map = fixture_map();
    let mut text = fixture_text(&map);
    let call = QUEST_MENU_GLYPH_CALLS[0];
    write_instruction(
        &map,
        &mut text,
        call.runtime,
        K::Jal {
            target: 0x8001_0000,
        },
    );
    assert!(verify_box_quest_ui_literals(&map, &text).is_err());

    let mut text = fixture_text(&map);
    write_instruction(
        &map,
        &mut text,
        0x800A_0000,
        K::Jal {
            target: 0x8002_CF98,
        },
    );
    let error = verify_box_quest_ui_literals(&map, &text).unwrap_err();
    assert!(error.to_string().contains("popup caller"));
}
