use crate::coords::LogicalFile;
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

fn shared_fixture() -> (BtxDescriptorSelectionSpec, &'static ExeMap, Vec<u8>) {
    let map = map(64);
    let mut kinds = vec![nop(); 64];
    kinds[0..5].copy_from_slice(&expected_selection_sequence(
        BtxDescriptorSelectionFamily::Shared,
    ));
    kinds[8] = K::Beq {
        rs: Reg::V0,
        rt: Reg::S1,
        offset: 29,
    };
    kinds[9] = K::Lui {
        rt: Reg::V0,
        imm: 0x8002,
    };
    kinds[10] = K::Addiu {
        rt: Reg::V0,
        rs: Reg::V0,
        imm: 0x3000,
    };
    kinds[11..13].copy_from_slice(&expected_copy_setup(BtxDescriptorSelectionFamily::Shared));
    kinds[13..27].copy_from_slice(&expected_copy_sequence(
        BtxDescriptorSelectionFamily::Shared,
    ));
    kinds[27] = K::Jr { rs: Reg::RA };
    let spec = selection(
        "TEST.EXE",
        "/TEST/TEST_GRP.VDD;1",
        0x8001_0000,
        0x8001_0020,
        0x8001_0034,
        0x8002_3000,
        BtxDescriptorSelectionFamily::Shared,
    );
    (spec, map, encode_all(&kinds))
}

#[test]
fn resource_36과_descriptor_36_copy를_검산한다() {
    let (spec, map, text) = shared_fixture();
    let analysis = audit_btx_descriptor_selections(&[spec], [(map, text.as_slice())]).unwrap();
    let [selection] = analysis.selections.as_slice() else {
        panic!("선택 경계가 하나여야 한다");
    };
    assert_eq!(selection.descriptor_select_runtime, 0x8001_0010);
    assert_eq!(selection.descriptor_name_field_read_runtime, 0x8001_0038);
    assert_eq!(selection.descriptor_copy_end_runtime, 0x8001_0068);
    assert_eq!(selection.descriptor_copy_source_register, Reg::V1);
    assert!(selection.entry_reached);
}

#[test]
fn descriptor_4_read가_달라지면_거부한다() {
    let (spec, map, mut text) = shared_fixture();
    let address = spec.descriptor_copy_runtime + 4;
    let offset = (address - map.load_addr) as usize;
    text[offset..offset + 4].copy_from_slice(
        &crate::isa::encode(K::Lw {
            rt: Reg::A3,
            base: Reg::V1,
            offset: 8,
        })
        .to_le_bytes(),
    );
    let error = audit_btx_descriptor_selections(&[spec], [(map, text.as_slice())]).unwrap_err();
    assert!(error.to_string().contains("선택 명령 불일치"));
}

#[test]
fn 입력_누락과_중복을_거부한다() {
    let (spec, map, text) = shared_fixture();
    assert!(audit_btx_descriptor_selections(&[spec], []).is_err());
    assert!(audit_btx_descriptor_selections(
        &[spec],
        [(map, text.as_slice()), (map, text.as_slice())]
    )
    .is_err());
}
