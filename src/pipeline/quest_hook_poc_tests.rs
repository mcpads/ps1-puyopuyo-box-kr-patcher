use crate::isa::{InstructionKind as K, Reg};

use super::*;

#[test]
fn 무동작_hook_source는_원본_abi를_보존한다() {
    let (site, body) = assemble_sources().expect("typed source 검산");
    assert_eq!(site.typed_source_id(), Some(SITE_SOURCE_ID));
    assert_eq!(body.typed_source_id(), Some(BODY_SOURCE_ID));
    assert_eq!(site.len_words(), 2);
    assert_eq!(body.len_words(), 3);
    assert_eq!(
        site.instructions()
            .iter()
            .map(|instruction| instruction.kind)
            .collect::<Vec<_>>(),
        vec![
            K::J {
                target: QUEST_HOOK_BODY_RUNTIME
            },
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ]
    );
    assert_eq!(
        body.instructions()
            .iter()
            .map(|instruction| instruction.kind)
            .collect::<Vec<_>>(),
        vec![
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
            K::J {
                target: QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]
    );
}

#[test]
fn 무동작_hook은_같은_kseg0_jump_segment와_후보_크기를_지킨다() {
    assert!(same_jump_segment(
        QUEST_FONT_HOOK_SITE_RUNTIME,
        QUEST_HOOK_BODY_RUNTIME
    ));
    assert!(same_jump_segment(
        QUEST_HOOK_BODY_RUNTIME + 4,
        QUEST_FONT_HOOK_CONTINUATION_RUNTIME
    ));
    const {
        assert!(QUEST_HOOK_BODY_BYTES <= QUEST_HOOK_RESERVED_BYTES);
    }
    assert_eq!(
        QUEST_HOOK_BODY_RESERVED_END_RUNTIME - QUEST_HOOK_BODY_RUNTIME,
        QUEST_HOOK_RESERVED_BYTES as u32
    );
}
