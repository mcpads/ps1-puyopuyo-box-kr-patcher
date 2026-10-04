use crate::coords::{ExeMap, LogicalFile};
use crate::isa::{encode, encode_all, InstructionKind as K};

use super::*;

const WORDS: usize = 64;
const FUNCTION: u32 = 0x8001_0000;

fn map() -> &'static ExeMap {
    Box::leak(Box::new(ExeMap {
        name: "GOCHA1.EXE",
        load_addr: FUNCTION,
        text_len: (WORDS * 4) as u32,
        entry_pc: FUNCTION,
        file: LogicalFile::new(0, (WORDS * 4 + 0x800) as u32),
    }))
}

fn nop() -> K {
    K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    }
}

fn branch_offset(branch_index: usize, target_index: usize) -> i16 {
    i16::try_from(target_index - branch_index - 1).unwrap()
}

fn renderer_text() -> Vec<u8> {
    let mut kinds = vec![nop(); WORDS];
    kinds[0] = K::Addiu {
        rt: Reg::SP,
        rs: Reg::SP,
        imm: -48,
    };
    kinds[2] = K::Addu {
        rd: Reg::S5,
        rs: Reg::A0,
        rt: Reg::ZERO,
    };
    kinds[10] = K::Lhu {
        rt: Reg::V1,
        base: Reg::S5,
        offset: 0x006C,
    };
    kinds[11] = K::Lhu {
        rt: Reg::A0,
        base: Reg::S5,
        offset: 0x006A,
    };
    kinds[12] = K::Addu {
        rd: Reg::S4,
        rs: Reg::V1,
        rt: Reg::V0,
    };
    kinds[13] = K::Lw {
        rt: Reg::V1,
        base: Reg::S4,
        offset: 0x001C,
    };
    kinds[14] = K::Addu {
        rd: Reg::S6,
        rs: Reg::V1,
        rt: Reg::V0,
    };
    kinds[15] = K::Bgez {
        rs: Reg::A1,
        offset: 1,
    };
    kinds[16] = K::Addiu {
        rt: Reg::S3,
        rs: Reg::S6,
        imm: 12,
    };
    kinds[18] = K::Bgez {
        rs: Reg::A0,
        offset: 1,
    };
    for branch_index in [20usize, 22, 24, 26] {
        kinds[branch_index] = K::Blez {
            rs: Reg::V0,
            offset: branch_offset(branch_index, 40),
        };
    }
    kinds[28] = K::Jal {
        target: 0x8002_0000,
    };
    kinds[29] = nop();
    encode_all(&kinds)
}

fn cell_analysis() -> BtxCellAnalysis {
    BtxCellAnalysis {
        summaries: Vec::new(),
        consumers: vec![BtxCellConsumer {
            exe: "GOCHA1.EXE",
            function_candidate: Some(FUNCTION),
            object_register: Reg::S5,
            resource_field_offset: 0x006C,
            cell_field_offset: 0x006A,
            resource_load: FUNCTION + 10 * 4,
            cell_load: FUNCTION + 11 * 4,
            descriptor_table_load: FUNCTION + 9 * 4,
            descriptor_entry: FUNCTION + 12 * 4,
            cell_table_load: FUNCTION + 13 * 4,
            cell_entry: FUNCTION + 14 * 4,
            core_entry_reached: true,
            renderer_family: Some(BtxRendererFamily::ShortRenderer),
        }],
        analysis_sha256: [0; 32],
    }
}

#[test]
fn lookup과_clipping통과지점의_live_register를_산출한다() {
    let map = map();
    let text = renderer_text();
    let analysis = analyze_common_font_draw_sites([(map, text.as_slice())], &cell_analysis())
        .expect("source 역할이 맞으면 관측 지점을 산출해야 한다");

    assert_eq!(analysis.sites.len(), 1);
    let site = &analysis.sites[0];
    assert_eq!(site.lookup_runtime, FUNCTION + 13 * 4);
    assert_eq!(site.primitive_runtime, FUNCTION + 28 * 4);
    assert_eq!(site.primitive_target_runtime, 0x8002_0000);
    assert_eq!(site.renderer_source_bytes, 30 * 4);
    assert_eq!(site.object_register, Reg::S5);
    assert_eq!(site.descriptor_register, Reg::S4);
    assert_eq!(site.cell_index_register, Reg::A0);
    assert_eq!(site.cell_record_register, Reg::S6);
    assert_eq!(site.cell_fields_register, Reg::S3);

    let report = render_common_font_draw_sites_tsv(&analysis);
    assert!(report.contains("GOCHA1.EXE\t/GOCHA1/TUU_GRP.VDD;1"));
    assert!(report.contains("$s5\t$s4\t$a0\t$s6\t$s3"));
}

#[test]
fn 관측전_descriptor_register를_덮어쓰면_거부한다() {
    let map = map();
    let mut text = renderer_text();
    text[19 * 4..20 * 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::S4,
            rs: Reg::ZERO,
            imm: 0,
        })
        .to_le_bytes(),
    );

    assert!(matches!(
        analyze_common_font_draw_sites([(map, text.as_slice())], &cell_analysis()),
        Err(CommonFontDrawSiteError::RegisterClobbered {
            role: "descriptor",
            register: Reg::S4,
            ..
        })
    ));
}

#[test]
fn 마지막_네_clipping분기가_같은탈출점이_아니면_거부한다() {
    let map = map();
    let mut text = renderer_text();
    text[26 * 4..27 * 4].copy_from_slice(
        &encode(K::Blez {
            rs: Reg::V0,
            offset: branch_offset(26, 41),
        })
        .to_le_bytes(),
    );

    assert!(matches!(
        analyze_common_font_draw_sites([(map, text.as_slice())], &cell_analysis()),
        Err(CommonFontDrawSiteError::ClippingBranchMismatch { .. })
    ));
}
