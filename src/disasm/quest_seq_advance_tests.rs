use super::*;
use crate::coords::exe_map;
use crate::isa::encode;
use crate::script::QUEST_SEQ_DISPATCH_HANDLERS;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    for site in QUEST_SEQ_DIRECT_ADVANCE_SITES {
        for (address, kind) in [
            (
                site.load_runtime,
                K::Lw {
                    rt: site.source_reg,
                    base: site.object_reg,
                    offset: QUEST_SEQ_STREAM_POINTER_FIELD_OFFSET,
                },
            ),
            (
                site.increment_runtime,
                K::Addiu {
                    rt: site.value_reg,
                    rs: site.source_reg,
                    imm: site.record_bytes,
                },
            ),
            (
                site.store_runtime,
                K::Sw {
                    rt: site.value_reg,
                    base: site.object_reg,
                    offset: QUEST_SEQ_STREAM_POINTER_FIELD_OFFSET,
                },
            ),
        ] {
            let offset = map.text_offset(address).expect("fixture 명령 범위");
            text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
        }
    }
    (map, text)
}

#[test]
fn 직접_전진_76곳을_74개_opcode와_다섯_폭으로_고정한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_direct_advances(map, &text),
        Ok(QuestSeqAdvanceSummary {
            sites: 76,
            opcodes: 74,
            widths: [4, 8, 12, 16, 20],
            without_direct_advance: [0x84, 0x8A, 0xAD, 0xC0],
        })
    );
}

#[test]
fn nonfallback_78개를_직접_전진_74개와_열린_4개로_빠짐없이_나눈다() {
    let mut direct = [false; 256];
    for site in QUEST_SEQ_DIRECT_ADVANCE_SITES {
        direct[usize::from(site.opcode)] = true;
    }
    let mut open = [false; 256];
    for opcode in QUEST_SEQ_WITHOUT_DIRECT_ADVANCE {
        open[usize::from(opcode)] = true;
    }

    let mut active = 0;
    for (index, &handler) in QUEST_SEQ_DISPATCH_HANDLERS.iter().enumerate() {
        let opcode = 0x80 + u8::try_from(index).expect("dispatch index");
        let fallback = handler == QUEST_SEQ_MAIN_FALLBACK_HANDLER_RUNTIME
            || handler == QUEST_SEQ_EXTENDED_FALLBACK_HANDLER_RUNTIME;
        if fallback {
            assert!(!direct[usize::from(opcode)]);
            assert!(!open[usize::from(opcode)]);
        } else {
            active += 1;
            assert_ne!(
                direct[usize::from(opcode)],
                open[usize::from(opcode)],
                "OP{opcode:02X}"
            );
        }
    }
    assert_eq!(active, 78);
}

#[test]
fn op89의_서로_다른_두_전진폭을_단일폭으로_축약하지_않는다() {
    let widths = QUEST_SEQ_DIRECT_ADVANCE_SITES
        .iter()
        .filter(|site| site.opcode == 0x89)
        .map(|site| site.record_bytes)
        .collect::<Vec<_>>();
    assert_eq!(widths, [20, 8]);
}

#[test]
fn 한_명령_drift를_거부한다() {
    let (map, mut text) = fixture();
    let site = QUEST_SEQ_DIRECT_ADVANCE_SITES
        .iter()
        .find(|site| site.opcode == 0x82)
        .expect("OP82 site");
    let offset = map
        .text_offset(site.increment_runtime)
        .expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: site.value_reg,
            rs: site.source_reg,
            imm: 8,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_direct_advances(map, &text),
        Err(QuestSeqAdvanceError::InstructionMismatch {
            opcode: 0x82,
            address,
            ..
        }) if address == site.increment_runtime
    ));
}
