use crate::coords::{ExeMap, LogicalFile};
use crate::isa::{encode_all, InstructionKind as K};

use super::*;
use crate::disasm::{BtxNameLink, CommonFontDrawSite};

const LOAD: u32 = 0x8001_0000;
const WORDS: usize = 64;

fn map() -> &'static ExeMap {
    Box::leak(Box::new(ExeMap {
        name: "GOCHA1.EXE",
        load_addr: LOAD,
        text_len: (WORDS * 4) as u32,
        entry_pc: LOAD,
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

fn text() -> Vec<u8> {
    let mut words = vec![nop(); WORDS];
    words[4] = K::Addiu {
        rt: Reg::T0,
        rs: Reg::ZERO,
        imm: 82,
    };
    words[5] = K::Sh {
        rt: Reg::T0,
        base: Reg::S0,
        offset: 0x006C,
    };
    words[6] = K::Addiu {
        rt: Reg::T1,
        rs: Reg::ZERO,
        imm: 26,
    };
    words[7] = K::Sh {
        rt: Reg::T1,
        base: Reg::S0,
        offset: 0x006A,
    };
    encode_all(&words)
}

fn names(pointer_runtime: u32) -> BtxNameAnalysis {
    BtxNameAnalysis {
        summaries: Vec::new(),
        links: vec![BtxNameLink {
            exe: "GOCHA1.EXE",
            archive: "/GOCHA1/TUU_GRP.VDD;1",
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
            descriptor_pointer_runtime: pointer_runtime,
            descriptor_runtime_start: LOAD,
            descriptor_runtime_end: LOAD + 4096,
            descriptor_static_ref_sites: Vec::new(),
            descriptor_entry_reached_ref_sites: Vec::new(),
        }],
        analysis_sha256: [0; 32],
    }
}

fn draw_sites() -> CommonFontDrawSiteAnalysis {
    CommonFontDrawSiteAnalysis {
        sites: vec![CommonFontDrawSite {
            exe: "GOCHA1.EXE",
            archive: "/GOCHA1/TUU_GRP.VDD;1",
            function_runtime: LOAD,
            renderer_source_bytes: 32,
            renderer_source_sha256: [0; 32],
            lookup_runtime: LOAD,
            primitive_runtime: LOAD + 4,
            primitive_target_runtime: LOAD + 8,
            object_register: Reg::S0,
            descriptor_register: Reg::S1,
            cell_index_register: Reg::A0,
            cell_record_register: Reg::S2,
            cell_fields_register: Reg::S3,
            resource_field_offset: 0x006C,
            cell_field_offset: 0x006A,
        }],
        analysis_sha256: [0; 32],
    }
}

fn descriptor_pointer_for_resource(resource: u32) -> u32 {
    LOAD + resource * DESCRIPTOR_STRIDE + DESCRIPTOR_NAME_POINTER_OFFSET
}

#[test]
fn 같은_object의_com_font_resource와_후보cell_literal을_결속한다() {
    let map = map();
    let text = text();
    let analysis = analyze_common_font_cell_sources(
        [(map, text.as_slice())],
        &names(descriptor_pointer_for_resource(82)),
        &draw_sites(),
    )
    .expect("같은 object의 정확한 resource와 cell literal은 결속해야 한다");

    assert_eq!(analysis.resolved_surfaces, 1);
    assert_eq!(analysis.unresolved_surfaces, 0);
    assert_eq!(analysis.literal_writer_sites, 1);
    assert_eq!(analysis.entry_reached_writer_sites, 1);
    let row = &analysis.rows[0];
    assert_eq!(row.surface_id, "gocha1-chain-label");
    assert_eq!(row.common_font_resource_id, 82);
    assert_eq!(row.matched_cell, Some(26));
    assert_eq!(row.cell_store_runtime, Some(LOAD + 7 * 4));
    assert_eq!(row.resource_store_runtimes, vec![LOAD + 5 * 4]);
    assert!(row.entry_reached);
    assert!(!row.runtime_draw_verified);

    let report = render_common_font_cell_sources_tsv(&analysis);
    assert!(report.contains("gocha1-chain-label\tGOCHA1.EXE"));
    assert!(report.contains("\t26\t82\t26\t0x8001001C\t0x80010014\ttrue\t"));
}

#[test]
fn resource와_cell사이에서_object_base가_덮이면_결속하지_않는다() {
    let map = map();
    let mut text = text();
    text[6 * 4..7 * 4].copy_from_slice(
        &crate::isa::encode(K::Addu {
            rd: Reg::S0,
            rs: Reg::A0,
            rt: Reg::ZERO,
        })
        .to_le_bytes(),
    );
    text[8 * 4..9 * 4].copy_from_slice(
        &crate::isa::encode(K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: 26,
        })
        .to_le_bytes(),
    );
    text[9 * 4..10 * 4].copy_from_slice(
        &crate::isa::encode(K::Sh {
            rt: Reg::T1,
            base: Reg::S0,
            offset: 0x006A,
        })
        .to_le_bytes(),
    );

    let analysis = analyze_common_font_cell_sources(
        [(map, text.as_slice())],
        &names(descriptor_pointer_for_resource(82)),
        &draw_sites(),
    )
    .expect("미해결 surface도 보존해야 한다");

    assert_eq!(analysis.resolved_surfaces, 0);
    assert_eq!(analysis.unresolved_surfaces, 1);
    assert_eq!(analysis.rows[0].cell_store_runtime, None);
    assert_eq!(
        analysis.rows[0].static_verdict,
        "no-bounded-paired-literal-writer-computed-or-indirect-open"
    );
}

#[test]
fn 계산된_cell_source를_literal부재나_미사용으로_승격하지_않는다() {
    let map = map();
    let mut text = text();
    text[6 * 4..7 * 4].copy_from_slice(
        &crate::isa::encode(K::Lhu {
            rt: Reg::T1,
            base: Reg::A0,
            offset: 0,
        })
        .to_le_bytes(),
    );

    let analysis = analyze_common_font_cell_sources(
        [(map, text.as_slice())],
        &names(descriptor_pointer_for_resource(82)),
        &draw_sites(),
    )
    .expect("계산 source는 미해결 surface로 남겨야 한다");

    assert_eq!(analysis.literal_writer_sites, 0);
    assert_eq!(analysis.unresolved_surfaces, 1);
    assert_eq!(analysis.rows[0].matched_cell, None);
}

#[test]
fn descriptor_name_pointer가_36바이트격자_4가_아니면_거부한다() {
    let map = map();
    let text = text();
    let error = analyze_common_font_cell_sources(
        [(map, text.as_slice())],
        &names(LOAD + 82 * DESCRIPTOR_STRIDE + 8),
        &draw_sites(),
    )
    .expect_err("descriptor resource ID를 추측하면 안 된다");

    assert_eq!(
        error,
        CommonFontCellSourceError::DescriptorPointerLayout("GOCHA1.EXE")
    );
}
