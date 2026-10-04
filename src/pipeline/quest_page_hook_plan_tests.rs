use super::*;

#[test]
fn 최소_body는_448바이트_후보에_들지만_source는_unbound다() {
    let body = assemble_minimum_body().unwrap();
    assert_eq!(body.len_words(), QUEST_PAGE_HOOK_BODY_WORDS);
    assert_eq!(body.bytes().len(), QUEST_PAGE_HOOK_BODY_BYTES);
    assert!(body.bytes().len() < QUEST_HOOK_RESERVED_BYTES);
    assert_eq!(QUEST_PAGE_HOOK_UNBOUND_SOURCE_RUNTIME, 0);
    assert!(body.base_addr() + body.bytes().len() as u32 <= QUEST_PAGE_HOOK_STATE_RUNTIME);
}

#[test]
fn 최소_body는_drawsync뒤에만_page_state를_commit한다() {
    let body = assemble_minimum_body().unwrap();
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
fn 모든_font_state의_active_byte를_검사한다() {
    assert_eq!(QUEST_FONT_STATE_RECORD_COUNT, 10);
    assert_eq!(QUEST_FONT_STATE_RECORD_STRIDE, 108);
    assert_eq!(QUEST_FONT_STATE_ACTIVE_OFFSET, 0x1F);
    let body = assemble_minimum_body().unwrap();
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
fn unbound_raw_page를_직접_upload한다() {
    assert_eq!(QUEST_PAGE_HOOK_PAGE_BYTES, 32 * 1024);
    let body = assemble_minimum_body().unwrap();
    let instructions = body.instructions();
    assert!(matches!(
        instructions[24].kind,
        K::Lui {
            rt: Reg::A1,
            imm: 0
        }
    ));
    assert!(matches!(
        instructions[25].kind,
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 0
        }
    ));
    assert!(matches!(
        instructions[26].kind,
        K::Jal {
            target: QUEST_FONT_UPLOAD_TARGET_RUNTIME
        }
    ));
}

#[test]
fn 원본_tail_진입_abi를_다시_만든다() {
    let body = assemble_minimum_body().unwrap();
    let kinds = body
        .instructions()
        .iter()
        .rev()
        .take(4)
        .map(|instruction| instruction.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
            K::J {
                target: QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
            },
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ]
    );
}
