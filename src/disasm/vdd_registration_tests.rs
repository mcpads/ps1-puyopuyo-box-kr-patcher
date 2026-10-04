use crate::coords::{ExeMap, LogicalFile};

use super::*;

fn map() -> &'static ExeMap {
    Box::leak(Box::new(ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x400,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, 0xC00),
    }))
}

fn spec(resource_type: u8) -> VddRegistrationSourceSpec {
    let map = map();
    VddRegistrationSourceSpec {
        exe: map.name,
        resource_type,
        archive: "TEST_GRP.VDD",
        registration_record_runtime: map.load_addr,
        registration_handler_runtime: map.load_addr + 0x100,
        loader_runtime: map.load_addr + 0x200,
        table_root_runtime: 0x8005_0000,
        archive_descriptor_runtime: map.load_addr + 0x300,
        archive_name_runtime: map.load_addr + 0x350,
        family: VddRegistrationFamily::Shared,
    }
}

#[test]
fn type0과_type1_등록_record를_구분한다() {
    let map = map();
    for resource_type in [0, 1] {
        let spec = spec(resource_type);
        let mut text = vec![0u8; map.text_len as usize];
        text[..4].copy_from_slice(&[VDD_REGISTRATION_OPCODE, resource_type, 0, 0]);
        text[4..8].copy_from_slice(&spec.archive_descriptor_runtime.to_le_bytes());
        verify_registration_record(spec, map, &text).unwrap();
    }
}

#[test]
fn 등록_record의_reserved_byte_drift를_거부한다() {
    let map = map();
    let spec = spec(0);
    let mut text = vec![0u8; map.text_len as usize];
    text[..4].copy_from_slice(&[VDD_REGISTRATION_OPCODE, 0, 0, 0]);
    text[4..8].copy_from_slice(&spec.archive_descriptor_runtime.to_le_bytes());
    verify_registration_record(spec, map, &text).unwrap();
    text[2] = 1;
    assert!(verify_registration_record(spec, map, &text).is_err());
}
