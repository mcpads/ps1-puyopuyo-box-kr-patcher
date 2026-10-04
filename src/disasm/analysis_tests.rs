use super::*;
use crate::coords::{ExeMap, LogicalFile};
use crate::isa::{decode_slice, encode_all, InstructionKind as K, Reg};

const BASE: u32 = 0x8001_0000;

fn nop() -> K {
    K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    }
}

fn build(kinds: &[K]) -> RefGraph {
    let text = encode_all(kinds);
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: BASE,
        text_len: text.len() as u32,
        entry_pc: BASE,
        file: LogicalFile::new(100, 0x800 + text.len() as u32),
    };
    let instructions = decode_slice(BASE, &text).expect("정렬된 텍스트");
    RefGraph::build(map, &instructions)
}

/// entry(0x00) → f1(0x10) → f2(0x20), f1은 데이터 0x800E6948을 참조.
fn sample() -> RefGraph {
    build(&[
        // 0x00 entry
        K::Jal {
            target: BASE + 0x10,
        },
        nop(),
        K::Jr { rs: Reg::RA },
        nop(),
        // 0x10 f1
        K::Lui {
            rt: Reg::T0,
            imm: 0x800E,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x6948,
        },
        K::Jal {
            target: BASE + 0x20,
        },
        nop(),
        // 0x20 f2
        K::Jr { rs: Reg::RA },
        nop(),
        nop(),
        nop(),
    ])
}

#[test]
fn callers_are_reported_as_fan_in() {
    let g = sample();
    assert_eq!(g.callers(BASE + 0x10), vec![BASE]);
    assert_eq!(g.callers(BASE + 0x20), vec![BASE + 0x18]);
}

#[test]
fn callees_are_reported_as_fan_out() {
    let g = sample();
    assert_eq!(g.callees(BASE), vec![BASE + 0x10]);
    assert_eq!(g.callees(BASE + 0x10), vec![BASE + 0x20]);
    assert!(g.callees(BASE + 0x20).is_empty());
}

#[test]
fn call_targets_are_recognized_as_functions() {
    let g = sample();
    let funcs: Vec<u32> = g.functions().collect();
    assert!(funcs.contains(&BASE));
    assert!(funcs.contains(&(BASE + 0x10)));
    assert!(funcs.contains(&(BASE + 0x20)));
}

#[test]
fn data_references_are_attributed_to_the_containing_function() {
    let g = sample();
    assert_eq!(g.data_refs(BASE + 0x10), vec![0x800E_6948]);
    // entry는 데이터를 참조하지 않는다.
    assert!(g.data_refs(BASE).is_empty());
}

#[test]
fn hot_data_ranks_shared_structures_first() {
    let g = build(&[
        K::Lui {
            rt: Reg::T0,
            imm: 0x800E,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x6948,
        },
        K::Lui {
            rt: Reg::T1,
            imm: 0x800E,
        },
        K::Lw {
            rt: Reg::V1,
            base: Reg::T1,
            offset: 0x6948,
        },
        K::Lui {
            rt: Reg::T2,
            imm: 0x8009,
        },
        K::Ori {
            rt: Reg::T2,
            rs: Reg::T2,
            imm: 0xBD50,
        },
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let hot = g.hot_data(4);
    assert_eq!(hot[0].0, 0x800E_6948);
    assert_eq!(hot[0].1, 2);
}

#[test]
fn call_path_connects_entry_to_a_leaf() {
    let g = sample();
    let path = g.call_path(BASE, BASE + 0x20).expect("경로가 있어야 함");
    assert_eq!(path, vec![BASE, BASE + 0x10, BASE + 0x20]);
}

#[test]
fn call_path_is_none_when_no_static_route_exists() {
    let g = sample();
    assert!(g.call_path(BASE + 0x20, BASE).is_none());
}

#[test]
fn seeded_function_without_static_callers_is_an_indirect_candidate() {
    // 0x08은 시드로 넣었지만 아무도 정적으로 호출하지 않는다.
    // 0x10은 jal 타깃이므로 호출자가 있다.
    let text = encode_all(&[
        K::Jal {
            target: BASE + 0x10,
        },
        nop(),
        K::Jr { rs: Reg::RA },
        nop(),
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let map = ExeMap {
        name: "TEST.EXE",
        load_addr: BASE,
        text_len: text.len() as u32,
        entry_pc: BASE,
        file: LogicalFile::new(100, 0x800 + text.len() as u32),
    };
    let instructions = decode_slice(BASE, &text).unwrap();
    let g = RefGraph::build_with_seeds(map, &instructions, &[BASE + 0x08, BASE + 0x10]);

    let candidates = g.seeded_without_callers();
    assert!(candidates.contains(&(BASE + 0x08)));
    assert!(
        !candidates.contains(&(BASE + 0x10)),
        "jal 타깃이 후보로 잡혔다"
    );
    assert!(!candidates.contains(&BASE), "엔트리가 후보로 잡혔다");
}

#[test]
fn without_seeds_there_are_no_indirect_candidates() {
    // 정적 jal 타깃은 정의상 호출자가 있다.
    assert!(sample().seeded_without_callers().is_empty());
}

#[test]
fn branch_edges_are_separate_from_call_edges() {
    let g = build(&[
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 2, // (0x00+4) + 2*4 = BASE+0x0C
        },
        nop(),
        nop(),
        K::Jr { rs: Reg::RA },
        nop(),
    ]);
    let fan_in = g.fan_in(BASE + 0x0C);
    assert_eq!(fan_in.len(), 1);
    assert_eq!(fan_in[0].kind, RefKind::Branch);
    assert!(g.callers(BASE + 0x0C).is_empty());
}

#[test]
fn summary_reports_both_directions() {
    let g = sample();
    let s = g.summary(BASE + 0x10);
    assert!(s.contains("0x80010010"));
    assert!(s.contains("fan-in"));
    assert!(s.contains("fan-out"));
}

#[test]
fn kseg1_query_finds_the_kseg0_node() {
    let g = sample();
    assert_eq!(g.callers(0xA001_0010), vec![BASE]);
}
