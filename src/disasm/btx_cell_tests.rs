use crate::coords::LogicalFile;
use crate::isa::{encode_all, InstructionKind as K};

use super::*;

fn map(name: &'static str, words: usize) -> &'static ExeMap {
    Box::leak(Box::new(ExeMap {
        name,
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

fn consumer_text(cell_offset: i16, resource_offset: i16) -> Vec<u8> {
    let mut kinds = vec![nop(); 48];
    kinds[0] = K::Addiu {
        rt: Reg::SP,
        rs: Reg::SP,
        imm: -0x30,
    };
    kinds[1] = K::Lhu {
        rt: Reg::V1,
        base: Reg::S4,
        offset: resource_offset,
    };
    kinds[2] = K::Lui {
        rt: Reg::A0,
        imm: 0x800B,
    };
    kinds[3] = K::Sll {
        rd: Reg::V0,
        rt: Reg::V1,
        sa: 3,
    };
    kinds[4] = K::Addu {
        rd: Reg::V0,
        rs: Reg::V0,
        rt: Reg::V1,
    };
    kinds[5] = K::Sll {
        rd: Reg::V0,
        rt: Reg::V0,
        sa: 2,
    };
    kinds[6] = K::Lw {
        rt: Reg::V1,
        base: Reg::A0,
        offset: 0x1234,
    };
    kinds[7] = K::Lhu {
        rt: Reg::A0,
        base: Reg::S4,
        offset: cell_offset,
    };
    kinds[8] = K::Addu {
        rd: Reg::S3,
        rs: Reg::V1,
        rt: Reg::V0,
    };
    kinds[9] = K::Sll {
        rd: Reg::V0,
        rt: Reg::A0,
        sa: 1,
    };
    kinds[10] = K::Addu {
        rd: Reg::V0,
        rs: Reg::V0,
        rt: Reg::A0,
    };
    kinds[11] = K::Lw {
        rt: Reg::V1,
        base: Reg::S3,
        offset: 0x001C,
    };
    kinds[12] = K::Sll {
        rd: Reg::V0,
        rt: Reg::V0,
        sa: 2,
    };
    kinds[13] = K::Addu {
        rd: Reg::S5,
        rs: Reg::V1,
        rt: Reg::V0,
    };
    kinds[14] = K::Jr { rs: Reg::RA };
    encode_all(&kinds)
}

#[test]
fn resource_36과_cell_12_dataflow를_검출한다() {
    let map = map("TEST.EXE", 48);
    let text = consumer_text(0x006A, 0x006C);
    let analysis = audit_btx_cell_consumers([(map, text.as_slice())]).unwrap();

    assert_eq!(analysis.consumers.len(), 1);
    let consumer = &analysis.consumers[0];
    assert_eq!(consumer.function_candidate, Some(0x8001_0000));
    assert_eq!(consumer.resource_load, 0x8001_0004);
    assert_eq!(consumer.cell_load, 0x8001_001C);
    assert_eq!(consumer.resource_field_offset, 0x006C);
    assert_eq!(consumer.cell_field_offset, 0x006A);
    assert!(consumer.core_entry_reached);
}

#[test]
fn quest_shift_layout도_같은_dataflow로_검출한다() {
    let map = map("QUEST.EXE", 48);
    let text = consumer_text(0x0066, 0x0068);
    let analysis = audit_btx_cell_consumers([(map, text.as_slice())]).unwrap();

    assert_eq!(analysis.consumers.len(), 1);
    assert_eq!(analysis.consumers[0].resource_field_offset, 0x0068);
    assert_eq!(analysis.consumers[0].cell_field_offset, 0x0066);
}

#[test]
fn field간격과_multiplier가_다르면_기각한다() {
    let map = map("TEST.EXE", 48);
    let wrong_offset = consumer_text(0x0068, 0x006C);
    assert!(audit_btx_cell_consumers([(map, wrong_offset.as_slice())])
        .unwrap()
        .consumers
        .is_empty());

    let mut wrong_multiplier = consumer_text(0x006A, 0x006C);
    wrong_multiplier[3 * 4..4 * 4].copy_from_slice(
        &crate::isa::encode(K::Sll {
            rd: Reg::V0,
            rt: Reg::V1,
            sa: 2,
        })
        .to_le_bytes(),
    );
    assert!(
        audit_btx_cell_consumers([(map, wrong_multiplier.as_slice())])
            .unwrap()
            .consumers
            .is_empty()
    );
}

#[test]
fn 중간에_source_register를_덮어쓰면_기각한다() {
    let map = map("TEST.EXE", 48);
    let mut text = consumer_text(0x006A, 0x006C);
    text[2 * 4..3 * 4].copy_from_slice(
        &crate::isa::encode(K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 7,
        })
        .to_le_bytes(),
    );

    assert!(audit_btx_cell_consumers([(map, text.as_slice())])
        .unwrap()
        .consumers
        .is_empty());
}

#[test]
fn 중복_exe와_길이불일치를_거부한다() {
    let map = map("TEST.EXE", 48);
    let text = consumer_text(0x006A, 0x006C);
    assert_eq!(
        audit_btx_cell_consumers([(map, text.as_slice()), (map, text.as_slice())]),
        Err(BtxCellError::DuplicateExecutable("TEST.EXE"))
    );
    assert!(matches!(
        audit_btx_cell_consumers([(map, &text[..text.len() - 4])]),
        Err(BtxCellError::TextLengthMismatch { .. })
    ));
}

#[test]
fn tsv는_주소와_entry_trace를_보존한다() {
    let map = map("TEST.EXE", 48);
    let text = consumer_text(0x006A, 0x006C);
    let analysis = audit_btx_cell_consumers([(map, text.as_slice())]).unwrap();
    let report = render_btx_cell_consumers_tsv(&analysis);

    assert!(report.starts_with("exe\trenderer_family\tfunction_candidate"));
    assert!(report.contains("TEST.EXE\t\t0x80010000\t$s4\t0x006C\t0x006A"));
    assert!(report.ends_with("\ttrue\n"));
}
