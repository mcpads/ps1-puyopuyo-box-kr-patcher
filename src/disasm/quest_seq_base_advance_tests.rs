use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    for site in QUEST_SEQ_BASE_DIRECT_ADVANCE_SITES {
        let mut instructions = vec![
            (
                site.load_runtime,
                K::Lw {
                    rt: site.load_reg,
                    base: site.object_reg,
                    offset: 0x1C,
                },
            ),
            (
                site.increment_runtime,
                K::Addiu {
                    rt: site.value_reg,
                    rs: site.increment_source_reg,
                    imm: site.record_bytes,
                },
            ),
            (
                site.store_runtime,
                K::Sw {
                    rt: site.value_reg,
                    base: site.object_reg,
                    offset: 0x1C,
                },
            ),
        ];
        if let Some(address) = site.move_runtime {
            instructions.push((
                address,
                K::Addu {
                    rd: site.value_reg,
                    rs: site.load_reg,
                    rt: Reg::ZERO,
                },
            ));
        }
        for (address, kind) in instructions {
            let offset = map.text_offset(address).expect("fixture 명령 범위");
            text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
        }
    }
    (map, text)
}

#[test]
fn base_직접_전진_71곳을_37개_active_opcode로_고정한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_base_direct_advances(map, &text),
        Ok(QuestSeqBaseAdvanceSummary {
            sites: 71,
            active_opcodes: 37,
            handlers: 38,
            fallback_entries: 82,
            widths: [4, 8, 12],
            without_direct_advance: [0x03, 0x04, 0x05, 0x09, 0x0A, 0x0D, 0x0E, 0x28, 0x39],
        })
    );
}

#[test]
fn 활성_46종을_직접_37종과_전이_9종으로_빈틈없이_나눈다() {
    let mut direct = [false; 0x80];
    for site in QUEST_SEQ_BASE_DIRECT_ADVANCE_SITES {
        if QUEST_SEQ_BASE_DISPATCH_HANDLERS[usize::from(site.opcode)]
            != QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME
        {
            direct[usize::from(site.opcode)] = true;
        }
    }
    let mut transitions = [false; 0x80];
    for opcode in QUEST_SEQ_BASE_WITHOUT_DIRECT_ADVANCE {
        transitions[usize::from(opcode)] = true;
    }
    let mut active = 0;
    for (opcode, &handler) in QUEST_SEQ_BASE_DISPATCH_HANDLERS.iter().enumerate() {
        if handler == QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME {
            assert!(!direct[opcode]);
            assert!(!transitions[opcode]);
        } else {
            active += 1;
            assert_ne!(direct[opcode], transitions[opcode], "OP{opcode:02X}");
        }
    }
    assert_eq!(active, 46);
}

#[test]
fn op08과33의_여러_폭을_단일폭으로_축약하지_않는다() {
    for (opcode, expected) in [(0x08, vec![4, 8]), (0x33, vec![8, 12])] {
        let mut widths = QUEST_SEQ_BASE_DIRECT_ADVANCE_SITES
            .iter()
            .filter(|site| site.opcode == opcode)
            .map(|site| site.record_bytes)
            .collect::<Vec<_>>();
        widths.sort_unstable();
        widths.dedup();
        assert_eq!(widths, expected);
    }
}

#[test]
fn alias_move_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8002_2F80).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Addu {
            rd: Reg::V0,
            rs: Reg::A0,
            rt: Reg::ZERO,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_base_direct_advances(map, &text),
        Err(QuestSeqBaseAdvanceError::InstructionMismatch {
            opcode: 0x24,
            address: 0x8002_2F80,
            ..
        })
    ));
}
