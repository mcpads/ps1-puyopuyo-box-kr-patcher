use super::*;

#[test]
fn scene_bound_body는_448바이트_후보와_state_앞에_든다() {
    let body = assemble_body().unwrap();
    assert_eq!(body.len_words(), QUEST_PAGE_FIRST_DIALOGUE_BODY_WORDS);
    assert_eq!(body.bytes().len(), QUEST_PAGE_FIRST_DIALOGUE_BODY_BYTES);
    assert!(body.bytes().len() < QUEST_HOOK_RESERVED_BYTES);
    assert!(body.base_addr() + body.bytes().len() as u32 <= QUEST_PAGE_HOOK_STATE_RUNTIME);
}

#[test]
fn target_record와_active실패는_모두_원본_tail로_간다() {
    let body = assemble_body().unwrap();
    let instructions = body.instructions();
    for index in [6usize, 11, 18] {
        let offset = match instructions[index].kind {
            K::Beq { offset, .. } | K::Bne { offset, .. } => offset,
            ref other => panic!("branch가 아님: {other:?}"),
        };
        let target = isize::try_from(index + 1).unwrap() + isize::from(offset);
        assert_eq!(target, 38);
    }
}

#[test]
fn raw_page를_resident_source에서_직접_upload한다() {
    let body = assemble_body().unwrap();
    let instructions = body.instructions();
    assert!(matches!(
        instructions[29].kind,
        K::Lui {
            rt: Reg::A1,
            imm: 0x8011
        }
    ));
    assert!(matches!(
        instructions[30].kind,
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: -0x7800
        }
    ));
    assert!(matches!(
        instructions[31].kind,
        K::Jal {
            target: QUEST_FONT_UPLOAD_TARGET_RUNTIME
        }
    ));
}

#[test]
fn active_load뒤에는_r3000a_load_delay_slot을_비운다() {
    let body = assemble_body().unwrap();
    let instructions = body.instructions();
    let load = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Lbu {
                    rt: Reg::T2,
                    base: Reg::T0,
                    offset: QUEST_FONT_STATE_ACTIVE_OFFSET,
                }
        })
        .unwrap();
    assert!(matches!(
        instructions[load + 1].kind,
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0
        }
    ));
    assert!(matches!(
        instructions[load + 2].kind,
        K::Bne {
            rs: Reg::T2,
            rt: Reg::ZERO,
            ..
        }
    ));
}

#[test]
fn drawsync뒤에만_page_state를_commit한다() {
    let body = assemble_body().unwrap();
    let instructions = body.instructions();
    let sync = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Jal {
                    target: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
                }
        })
        .unwrap();
    let commit = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction.kind,
                K::Sb {
                    rt: Reg::T1,
                    base: Reg::T0,
                    offset
                } if offset == QUEST_PAGE_HOOK_STATE_RUNTIME as u16 as i16
            )
        })
        .unwrap();
    assert!(sync < commit);
}

#[test]
fn page_source는_정적_carrier_안이고_target은_승인_record다() {
    assert_eq!(QUEST_PAGE_FIRST_DIALOGUE_RECORD_OFFSET, 0x21CC);
    assert_eq!(QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME, 0x8019_BC3C);
    assert_eq!(QUEST_PAGE_CARRIER_START_RUNTIME, 0x8010_8800);
    assert_eq!(
        QUEST_PAGE_CARRIER_END_RUNTIME - QUEST_PAGE_CARRIER_START_RUNTIME,
        QUEST_PAGE_CARRIER_CAPACITY_BYTES as u32
    );
}

#[test]
fn gpu_dump에서_font_rect를_row_stride로_추출한다() {
    let expected = (0..QUEST_FONT_VRAM_ROW_BYTES * QUEST_FONT_VRAM_ROWS)
        .map(|index| ((index * 37 + 11) % 256) as u8)
        .collect::<Vec<_>>();
    let mut gpu = vec![0xCD; PSX_GPU_RAM_BYTES];
    for row in 0..QUEST_FONT_VRAM_ROWS {
        let gpu_start = row * PSX_GPU_ROW_BYTES + QUEST_FONT_VRAM_X_BYTES;
        let page_start = row * QUEST_FONT_VRAM_ROW_BYTES;
        gpu[gpu_start..gpu_start + QUEST_FONT_VRAM_ROW_BYTES]
            .copy_from_slice(&expected[page_start..page_start + QUEST_FONT_VRAM_ROW_BYTES]);
    }
    assert_eq!(uploaded_font_page(&gpu).unwrap(), expected);
}

#[test]
fn gpu_dump_길이_drift를_거부한다() {
    assert!(uploaded_font_page(&vec![0; PSX_GPU_RAM_BYTES - 1])
        .unwrap_err()
        .to_string()
        .contains("GPU RAM dump 길이 불일치"));
}

#[test]
fn two_page_body는_448바이트_후보와_state_앞에_든다() {
    let body = assemble_transition_body().unwrap();
    assert_eq!(body.len_words(), QUEST_PAGE_TRANSITION_BODY_WORDS);
    assert_eq!(body.bytes().len(), QUEST_PAGE_TRANSITION_BODY_BYTES);
    assert!(body.bytes().len() < QUEST_HOOK_RESERVED_BYTES);
    assert!(body.base_addr() + body.bytes().len() as u32 <= QUEST_PAGE_HOOK_STATE_RUNTIME);
}

#[test]
fn two_page_body는_한글record만_한글source이고_나머지는_원본source다() {
    let body = assemble_transition_body().unwrap();
    let instructions = body.instructions();
    assert!(matches!(
        instructions[6].kind,
        K::Beq {
            rs: Reg::V0,
            rt: Reg::T0,
            ..
        }
    ));
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(
                instruction.kind,
                K::Beq {
                    rs: Reg::V0,
                    rt: Reg::T0,
                    ..
                } | K::Bne {
                    rs: Reg::V0,
                    rt: Reg::T0,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(matches!(
        instructions[10].kind,
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: 2
        }
    ));
    assert!(matches!(
        instructions[15].kind,
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: 1
        }
    ));
    assert_eq!(QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME, 0x8019_BC3C);
    assert_eq!(QUEST_PAGE_CARRIER_START_RUNTIME, 0x8010_8800);
    assert_eq!(QUEST_PAGE_RESTORE_SOURCE_RUNTIME, 0x8011_0800);
    assert_eq!(
        QUEST_PAGE_RESTORE_SOURCE_RUNTIME + QUEST_FONT_SOURCE_BYTES as u32,
        0x8011_8800
    );
    assert!(
        QUEST_PAGE_RESTORE_SOURCE_RUNTIME + QUEST_FONT_SOURCE_BYTES as u32
            <= QUEST_PAGE_CARRIER_END_RUNTIME
    );
}

#[test]
fn two_page_body는_sync뒤_stack의_page_id를_commit한다() {
    let body = assemble_transition_body().unwrap();
    let instructions = body.instructions();
    let sync = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Jal {
                    target: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
                }
        })
        .unwrap();
    let page_id_load = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Lw {
                    rt: Reg::T1,
                    base: Reg::SP,
                    offset: 8,
                }
        })
        .unwrap();
    let commit = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction.kind,
                K::Sb {
                    rt: Reg::T1,
                    base: Reg::T0,
                    offset
                } if offset == QUEST_PAGE_HOOK_STATE_RUNTIME as u16 as i16
            )
        })
        .unwrap();
    assert!(sync < page_id_load);
    assert_eq!(page_id_load + 2, commit);
}

#[test]
fn two_dialogue_body는_두_record만_공용_korean_page를_고른다() {
    let body = assemble_two_dialogue_transition_body().unwrap();
    assert_eq!(body.len_words(), QUEST_PAGE_TWO_DIALOGUES_BODY_WORDS);
    assert_eq!(body.bytes().len(), QUEST_PAGE_TWO_DIALOGUES_BODY_BYTES);
    assert!(body.bytes().len() < QUEST_HOOK_RESERVED_BYTES);
    assert!(body.base_addr() + body.bytes().len() as u32 <= QUEST_PAGE_HOOK_STATE_RUNTIME);
    assert_eq!(QUEST_PAGE_SECOND_DIALOGUE_RECORD_OFFSET, 0x22DC);
    assert_eq!(QUEST_PAGE_SECOND_DIALOGUE_RECORD_RUNTIME, 0x8019_BD4C);
    assert_eq!(
        body.instructions()
            .iter()
            .filter(|instruction| matches!(
                instruction.kind,
                K::Beq {
                    rs: Reg::V0,
                    rt: Reg::T0,
                    ..
                }
            ))
            .count(),
        2
    );
}
