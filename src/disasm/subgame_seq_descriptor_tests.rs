use crate::coords::{ExeMap, LogicalFile};
use crate::isa::{encode_all, InstructionKind as K};

use super::*;

fn map(words: usize) -> &'static ExeMap {
    Box::leak(Box::new(ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: (words * 4) as u32,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, (words * 4 + 0x800) as u32),
    }))
}

fn nop() -> K {
    K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    }
}

fn shared_fixture() -> (SubgameSeqDescriptorSpec, &'static ExeMap, Vec<u8>) {
    let map = map(128);
    let mut kinds = vec![nop(); 128];
    let spec = SubgameSeqDescriptorSpec {
        exe: "TEST.EXE",
        archive: "/TEST/TEST_SEQ.VDD;1",
        asset: "TEST.SEQ",
        archive_entries: 1,
        vdd_index: 0,
        selector_runtime: map.load_addr,
        table_root_runtime: 0x8002_4000,
        descriptor_object_runtime: map.load_addr + 0x100,
        descriptor_runtime: map.load_addr + 0x180,
        asset_name_runtime: map.load_addr + 0x1D0,
        stored_bytes: 1,
        decompressed_bytes: 1,
        family: SelectorFamily::Shared,
    };
    let start = 0usize;
    let set = |kinds: &mut [K], byte_offset: usize, kind: K| {
        kinds[start + byte_offset / 4] = kind;
    };
    set(
        &mut kinds,
        0x1C,
        K::Lw {
            rt: Reg::A0,
            base: Reg::S3,
            offset: 0x1C,
        },
    );
    set(
        &mut kinds,
        0x24,
        K::Lhu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 4,
        },
    );
    set(
        &mut kinds,
        0x2C,
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 5,
        },
    );
    set(
        &mut kinds,
        0x30,
        K::Lui {
            rt: Reg::V0,
            imm: 0x8002,
        },
    );
    set(
        &mut kinds,
        0x34,
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 3,
        },
    );
    set(
        &mut kinds,
        0x38,
        K::Lhu {
            rt: Reg::A0,
            base: Reg::A0,
            offset: 4,
        },
    );
    set(
        &mut kinds,
        0x3C,
        K::J {
            target: map.load_addr + 0x50,
        },
    );
    for byte_offset in [0x40, 0x44] {
        set(
            &mut kinds,
            byte_offset,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x4000,
            },
        );
    }
    set(
        &mut kinds,
        0x48,
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 3,
        },
    );
    set(
        &mut kinds,
        0x4C,
        K::Lhu {
            rt: Reg::A0,
            base: Reg::S3,
            offset: 0x6C,
        },
    );
    set(
        &mut kinds,
        0x50,
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 2,
        },
    );
    set(
        &mut kinds,
        0x54,
        K::Addu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::V0,
        },
    );
    set(
        &mut kinds,
        0x58,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::A0,
            sa: 3,
        },
    );
    set(
        &mut kinds,
        0x5C,
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A0,
        },
    );
    set(
        &mut kinds,
        0x60,
        K::Lw {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 0,
        },
    );
    set(
        &mut kinds,
        0x64,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    );
    set(
        &mut kinds,
        0x68,
        K::Addu {
            rd: Reg::S2,
            rs: Reg::V1,
            rt: Reg::V0,
        },
    );
    set(
        &mut kinds,
        0x6C,
        K::Jal {
            target: spec.descriptor_object_runtime,
        },
    );
    set(
        &mut kinds,
        0x70,
        K::Addu {
            rd: Reg::A0,
            rs: Reg::S2,
            rt: Reg::ZERO,
        },
    );
    let object = 0x100 / 4;
    kinds[object] = K::Lw {
        rt: Reg::V0,
        base: Reg::A0,
        offset: 0x14,
    };
    kinds[object + 2] = K::Beq {
        rs: Reg::V0,
        rt: Reg::ZERO,
        offset: 14,
    };
    kinds[object + 9] = K::Sw {
        rt: Reg::V1,
        base: Reg::A0,
        offset: 0x14,
    };
    (spec, map, encode_all(&kinds))
}

#[test]
fn record_type과_resource_36_selector를_검산한다() {
    let (spec, map, text) = shared_fixture();
    let instructions = linear::sweep(map, &text).unwrap();
    let (type_read, resource_read, resource_x36) =
        verify_shared_selector(&instructions, spec).unwrap();
    verify_object_cache(&instructions, spec).unwrap();
    assert_eq!(type_read, map.load_addr + 0x34);
    assert_eq!(resource_read, map.load_addr + 0x38);
    assert_eq!(resource_x36, map.load_addr + 0x58);
}

#[test]
fn resource_36의_마지막_곱셈이_달라지면_거부한다() {
    let (spec, map, mut text) = shared_fixture();
    let offset = 0x64;
    text[offset..offset + 4].copy_from_slice(
        &crate::isa::encode(K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 1,
        })
        .to_le_bytes(),
    );
    let instructions = linear::sweep(map, &text).unwrap();
    assert!(verify_shared_selector(&instructions, spec).is_err());
}

#[test]
fn 보고서는_선택과_source_descriptor의_증거_경계를_표시한다() {
    let analysis = SubgameSeqDescriptorAnalysis {
        rows: vec![SubgameSeqDescriptorRow {
            exe: "TEST.EXE",
            archive: "/TEST/TEST_SEQ.VDD;1",
            asset: "TEST.SEQ",
            vdd_index: 0,
            selector_runtime: 1,
            record_type_read_runtime: 2,
            record_resource_read_runtime: 3,
            table_root_runtime: 4,
            resource_x36_runtime: 5,
            descriptor_object_runtime: 6,
            descriptor_runtime: 7,
            asset_name_runtime: 8,
            stored_bytes: 9,
            decompressed_bytes: 10,
        }],
    };
    let report = render_subgame_seq_descriptors_tsv(&analysis);
    assert!(report.contains("generic-selector-and-source-descriptor-only"));
    assert!(report.contains("0x00000005"));
}
