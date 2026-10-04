use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    let instructions = [
        (
            0x8007_1050,
            K::Lui {
                rt: Reg::A1,
                imm: 0x801D,
            },
        ),
        (
            0x8007_1054,
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x1270,
            },
        ),
        (
            0x8007_1060,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 2,
            },
        ),
        (
            0x8007_1064,
            K::Lw {
                rt: Reg::A0,
                base: Reg::A0,
                offset: -0x4CD4,
            },
        ),
        (
            0x8007_1068,
            K::J {
                target: 0x8007_1418,
            },
        ),
        (
            0x8007_106C,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 0x97,
            },
        ),
        (
            0x8007_1418,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V1,
                sa: 3,
            },
        ),
        (
            0x8007_141C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8007_1420,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8007_1424,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_1428,
            K::Jal {
                target: QUEST_SEQ_LOAD_RUNTIME,
            },
        ),
        (
            0x8007_142C,
            K::Sw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x28,
            },
        ),
        (
            0x8007_3C68,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8019,
            },
        ),
        (
            0x8007_3C74,
            K::Ori {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x9A70,
            },
        ),
        (
            0x8007_3C88,
            K::Sw {
                rt: Reg::A0,
                base: Reg(17),
                offset: 0x1C,
            },
        ),
        (
            0x8007_3CDC,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8019,
            },
        ),
        (
            0x8007_3CF4,
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x9A70,
            },
        ),
        (
            0x8007_3D04,
            K::Sw {
                rt: Reg::A1,
                base: Reg(17),
                offset: 0x1C,
            },
        ),
        (
            0x8007_3D8C,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8019,
            },
        ),
        (
            0x8007_3D90,
            K::Ori {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x9A70,
            },
        ),
        (
            0x8007_3D94,
            K::Lui {
                rt: Reg::A1,
                imm: 0x801D,
            },
        ),
        (
            0x8007_3D98,
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x1270,
            },
        ),
        (
            0x8007_3D9C,
            K::Jal {
                target: QUEST_SEQ_WORD_COPY_RUNTIME,
            },
        ),
        (
            0x8007_3DA0,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 0x2000,
            },
        ),
        (
            0x8007_E618,
            K::Beq {
                rs: Reg::A2,
                rt: Reg::ZERO,
                offset: 7,
            },
        ),
        (
            0x8007_E620,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            0x8007_E62C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            0x8007_E630,
            K::Bne {
                rs: Reg::A2,
                rt: Reg::ZERO,
                offset: -5,
            },
        ),
    ];
    for (address, kind) in instructions {
        let offset = map.text_offset(address).expect("명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    (map, text)
}

#[test]
fn descriptor선택부터_active_stream까지_고정한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_loader(map, &text),
        Ok(QuestSeqLoaderSummary {
            profile: IsaProfile::DECLARED,
            descriptor_slot_runtime: 0x800D_B32C,
            descriptor_stride: 36,
            resource_id_state_offset: 2,
            staging_runtime: 0x801D_1270,
            active_runtime: 0x8019_9A70,
            loader_runtime: 0x8001_B1D4,
            copy_runtime: 0x8007_E618,
            active_copy_words: 0x2000,
            stream_entry_sites: [0x8007_3C88, 0x8007_3D04],
        })
    );
}

#[test]
fn staging에서_active로_향하지_않는_copy를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_3D94).expect("staging source 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Lui {
            rt: Reg::A0,
            imm: 0x801D,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_loader(map, &text),
        Err(QuestSeqLoaderError::InstructionMismatch {
            address: 0x8007_3D94,
            ..
        })
    ));
}
