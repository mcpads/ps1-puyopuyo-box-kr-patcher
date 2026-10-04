use crate::coords::LogicalFile;
use crate::disasm::{BtxNameAnalysis, BtxNameLink};
use crate::isa::{encode_all, InstructionKind as K, Reg};

use super::*;

const LOAD: u32 = 0x8001_0000;
const RESOURCE: u16 = 40;
const CELL_FIELD: i16 = 0x0066;

fn map() -> &'static ExeMap {
    Box::leak(Box::new(ExeMap {
        name: "QUEST.EXE",
        load_addr: LOAD,
        text_len: SOURCE_BYTES as u32,
        entry_pc: LOAD,
        file: LogicalFile::new(0, (SOURCE_BYTES + 0x800) as u32),
    }))
}

fn nop() -> K {
    K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    }
}

fn fixed_draw_text(resource: u16) -> Vec<u8> {
    let mut words = vec![nop(); SOURCE_WORDS];
    words[0] = K::Addu {
        rd: Reg::T0,
        rs: Reg::A0,
        rt: Reg::ZERO,
    };
    words[1] = K::Lw {
        rt: Reg::A1,
        base: Reg::T0,
        offset: CELL_FIELD - 0x1A,
    };
    words[2] = K::Lw {
        rt: Reg::V0,
        base: Reg::T0,
        offset: CELL_FIELD - 0x22,
    };
    words[3] = K::Lw {
        rt: Reg::A3,
        base: Reg::T0,
        offset: CELL_FIELD - 0x16,
    };
    words[4] = K::Lhu {
        rt: Reg::A0,
        base: Reg::T0,
        offset: CELL_FIELD,
    };
    words[5] = K::Addu {
        rd: Reg::A1,
        rs: Reg::A1,
        rt: Reg::V0,
    };
    words[6] = K::Lui {
        rt: Reg::V0,
        imm: 0x800E,
    };
    words[7] = K::Sll {
        rd: Reg::V1,
        rt: Reg::A0,
        sa: 1,
    };
    words[8] = K::Lw {
        rt: Reg::V0,
        base: Reg::V0,
        offset: -19672,
    };
    words[9] = K::Addu {
        rd: Reg::V1,
        rs: Reg::V1,
        rt: Reg::A0,
    };
    words[10] = K::Lw {
        rt: Reg::V0,
        base: Reg::V0,
        offset: (u32::from(resource) * DESCRIPTOR_STRIDE + DESCRIPTOR_CELL_TABLE_OFFSET) as i16,
    };
    words[11] = K::Sll {
        rd: Reg::V1,
        rt: Reg::V1,
        sa: 2,
    };
    words[12] = K::Addu {
        rd: Reg::V0,
        rs: Reg::V0,
        rt: Reg::V1,
    };
    words[14] = K::Addiu {
        rt: Reg::A2,
        rs: Reg::V0,
        imm: 12,
    };
    words[47] = K::Lui {
        rt: Reg::A0,
        imm: 0x800F,
    };
    words[48] = K::Lw {
        rt: Reg::V0,
        base: Reg::GP,
        offset: 0x021C,
    };
    words[49] = K::Addiu {
        rt: Reg::A0,
        rs: Reg::A0,
        imm: -1344,
    };
    words[50] = K::Sll {
        rd: Reg::V0,
        rt: Reg::V0,
        sa: 3,
    };
    words[51] = K::Addu {
        rd: Reg::V0,
        rs: Reg::V0,
        rt: Reg::A0,
    };
    words[52] = K::Sh {
        rt: Reg::A1,
        base: Reg::V0,
        offset: 0,
    };
    words[53] = K::Sh {
        rt: Reg::A3,
        base: Reg::V0,
        offset: 2,
    };
    words[54] = K::Lhu {
        rt: Reg::A1,
        base: Reg::T0,
        offset: CELL_FIELD,
    };
    words[55] = K::Sb {
        rt: Reg::ZERO,
        base: Reg::V0,
        offset: 6,
    };
    words[56] = K::Lw {
        rt: Reg::V1,
        base: Reg::GP,
        offset: 0x021C,
    };
    words[58] = K::Sll {
        rd: Reg::V1,
        rt: Reg::V1,
        sa: 3,
    };
    words[59] = K::Sh {
        rt: Reg::A1,
        base: Reg::V0,
        offset: 4,
    };
    words[60] = K::Lbu {
        rt: Reg::V0,
        base: Reg::T0,
        offset: CELL_FIELD + 8,
    };
    words[61] = K::Addu {
        rd: Reg::V1,
        rs: Reg::V1,
        rt: Reg::A0,
    };
    words[62] = K::Andi {
        rt: Reg::V0,
        rs: Reg::V0,
        imm: 7,
    };
    words[63] = K::Sb {
        rt: Reg::V0,
        base: Reg::V1,
        offset: 7,
    };
    words[64] = K::Lw {
        rt: Reg::V0,
        base: Reg::GP,
        offset: 0x021C,
    };
    words[66] = K::Addiu {
        rt: Reg::V0,
        rs: Reg::V0,
        imm: 1,
    };
    words[67] = K::Sw {
        rt: Reg::V0,
        base: Reg::GP,
        offset: 0x021C,
    };
    words[68] = K::Jr { rs: Reg::RA };
    encode_all(&words)
}

fn names(resource: u16) -> BtxNameAnalysis {
    let descriptor_start = 0x8008_C8F8;
    BtxNameAnalysis {
        summaries: Vec::new(),
        links: vec![BtxNameLink {
            exe: "QUEST.EXE",
            archive: "/QUEST/RPG_GRP.VDD;1",
            asset_name: "COM_FONT.BTX".to_string(),
            exe_text_offset: 0,
            runtime_addr: LOAD,
            static_ref_sites: Vec::new(),
            entry_reached_ref_sites: Vec::new(),
            table_runtime_start: LOAD,
            table_runtime_end: LOAD,
            table_static_ref_sites: Vec::new(),
            table_entry_reached_ref_sites: Vec::new(),
            descriptor_pointer_offset: 0,
            descriptor_pointer_runtime: descriptor_start
                + u32::from(resource) * DESCRIPTOR_STRIDE
                + DESCRIPTOR_NAME_POINTER_OFFSET,
            descriptor_runtime_start: descriptor_start,
            descriptor_runtime_end: descriptor_start + 4096,
            descriptor_static_ref_sites: Vec::new(),
            descriptor_entry_reached_ref_sites: Vec::new(),
        }],
        analysis_sha256: [0; 32],
    }
}

#[test]
fn 고정_com_font_descriptor의_cell_command_commit을_찾는다() {
    let map = map();
    let text = fixed_draw_text(RESOURCE);
    let analysis = audit_common_font_fixed_draws([(map, text.as_slice())], &names(RESOURCE))
        .expect("같은 COM_FONT resource의 direct draw는 검출해야 한다");

    assert_eq!(analysis.scanned_executables, 1);
    assert_eq!(analysis.common_font_executables, 1);
    let [site] = analysis.sites.as_slice() else {
        panic!("고정 COM_FONT draw가 하나여야 한다");
    };
    assert_eq!(site.function_runtime, LOAD);
    assert_eq!(site.descriptor_root_pointer_runtime, 0x800D_B328);
    assert_eq!(site.common_font_resource_id, RESOURCE);
    assert_eq!(site.command_buffer_runtime, 0x800E_FAC0);
    assert_eq!(site.command_count_gp_offset, 0x021C);
    assert_eq!(site.cell_field_offset, CELL_FIELD);
    assert_eq!(site.commit_runtime, LOAD + 67 * 4);
}

#[test]
fn 다른_resource의_고정_draw를_com_font로_승격하지_않는다() {
    let map = map();
    let text = fixed_draw_text(RESOURCE + 1);
    let analysis = audit_common_font_fixed_draws([(map, text.as_slice())], &names(RESOURCE))
        .expect("다른 fixed descriptor도 분석 자체는 성공해야 한다");

    assert!(analysis.sites.is_empty());
}

#[test]
fn command_count_commit_offset이_다르면_direct_draw로_보지_않는다() {
    let map = map();
    let mut text = fixed_draw_text(RESOURCE);
    text[67 * 4..68 * 4].copy_from_slice(
        &crate::isa::encode(K::Sw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 0x021E,
        })
        .to_le_bytes(),
    );
    let analysis = audit_common_font_fixed_draws([(map, text.as_slice())], &names(RESOURCE))
        .expect("불일치 함수는 후보에서 제외해야 한다");

    assert!(analysis.sites.is_empty());
}
