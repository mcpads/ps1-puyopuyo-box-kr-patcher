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

fn route_text(caller_index: usize, stub_index: usize) -> Vec<u8> {
    let mut kinds = vec![nop(); 96];
    let stub = 0x8001_0000 + (stub_index * 4) as u32;
    kinds[caller_index] = K::Jal { target: stub };
    kinds[caller_index + 1] = nop();
    kinds[caller_index + 2] = K::Jr { rs: Reg::RA };
    kinds[stub_index] = K::Addiu {
        rt: Reg::T2,
        rs: Reg::ZERO,
        imm: 0x00B0,
    };
    kinds[stub_index + 1] = K::Jr { rs: Reg::T2 };
    kinds[stub_index + 2] = K::Addiu {
        rt: Reg::T1,
        rs: Reg::ZERO,
        imm: 0x0051,
    };
    encode_all(&kinds)
}

#[test]
fn stub_direct_caller와_entry_trace를_분리해_센다() {
    let map = map("A.EXE", 96);
    let text = route_text(32, 64);
    let analysis = audit_bios_font_routes([(map, text.as_slice())]).unwrap();
    let route = &analysis.routes[0];

    assert_eq!(route.stubs, [0x8001_0100]);
    assert_eq!(route.direct_callers, [0x8001_0080]);
    assert_eq!(route.wrapper_entry, None);
    assert!(route.wrapper_callers.is_empty());
    assert_eq!(route.entry_reached_callers, [0x8001_0080]);
    assert_eq!(route.entry_reached_stubs, [0x8001_0100]);
    assert!(route.entry_reached_wrapper_callers.is_empty());
    assert!(route.callsite_shape_sha256.is_some());
}

#[test]
fn wrapper_entry와_그_direct_caller를_별도_추적한다() {
    let map = map("A.EXE", 96);
    let mut kinds = vec![nop(); 96];
    let wrapper_entry = 0x8001_0040;
    let stub = 0x8001_0100;
    kinds[4] = K::Jal {
        target: wrapper_entry,
    };
    kinds[16] = K::Addiu {
        rt: Reg::SP,
        rs: Reg::SP,
        imm: -0x38,
    };
    kinds[32] = K::Jal { target: stub };
    kinds[64] = K::Addiu {
        rt: Reg::T2,
        rs: Reg::ZERO,
        imm: 0x00B0,
    };
    kinds[65] = K::Jr { rs: Reg::T2 };
    kinds[66] = K::Addiu {
        rt: Reg::T1,
        rs: Reg::ZERO,
        imm: 0x0051,
    };
    let text = encode_all(&kinds);

    let analysis = audit_bios_font_routes([(map, text.as_slice())]).unwrap();
    let route = &analysis.routes[0];

    assert_eq!(route.wrapper_entry, Some(wrapper_entry));
    assert_eq!(route.wrapper_callers, [0x8001_0010]);
    assert_eq!(route.entry_reached_wrapper_callers, [0x8001_0010]);
}

#[test]
fn 호출점_구조_비교는_immediate를_무시하고_register를_보존한다() {
    assert_eq!(
        normalize_instruction_shape(0x2484_1234),
        normalize_instruction_shape(0x2484_5678)
    );
    assert_ne!(
        normalize_instruction_shape(0x2484_1234),
        normalize_instruction_shape(0x2485_1234)
    );
    assert_eq!(
        normalize_instruction_shape(0x0C01_2345),
        normalize_instruction_shape(0x0C03_4567)
    );
}

#[test]
fn 두_route의_같은_국소_shape를_전부_일치시킨다() {
    let first_map = map("A.EXE", 96);
    let second_map = map("B.EXE", 96);
    let first = route_text(32, 64);
    let mut second = route_text(32, 64);
    second[0..4].copy_from_slice(&0x2404_7777u32.to_le_bytes());

    let analysis = audit_bios_font_routes([
        (first_map, first.as_slice()),
        (second_map, second.as_slice()),
    ])
    .unwrap();

    assert_eq!(analysis.callsite_similarities.len(), 1);
    assert_eq!(
        analysis.callsite_similarities[0].matched_words,
        CALLSITE_WINDOW_WORDS
    );
}

#[test]
fn 잘린_window와_중복_exe를_거부한다() {
    let short_map = map("SHORT.EXE", 32);
    let short = route_text(0, 16);
    assert!(matches!(
        audit_bios_font_routes([(short_map, &short[..32 * 4])]),
        Err(BiosFontError::CallsiteWindowOutsideText { .. })
    ));

    let map = map("A.EXE", 96);
    let text = route_text(32, 64);
    assert_eq!(
        audit_bios_font_routes([(map, text.as_slice()), (map, text.as_slice())]),
        Err(BiosFontError::DuplicateExecutable("A.EXE"))
    );
}

#[test]
fn long_form_tsv는_route와_similarity를_분리한다() {
    let map_a = map("A.EXE", 96);
    let map_b = map("B.EXE", 96);
    let first = route_text(32, 64);
    let second = first.clone();
    let analysis =
        audit_bios_font_routes([(map_a, first.as_slice()), (map_b, second.as_slice())]).unwrap();
    let report = render_bios_font_analysis_tsv(&analysis);

    assert!(report.starts_with("kind\tleft_exe\tright_exe"));
    assert!(report.contains("route\tA.EXE"));
    assert!(report.contains("similarity\tA.EXE\tB.EXE"));
}
