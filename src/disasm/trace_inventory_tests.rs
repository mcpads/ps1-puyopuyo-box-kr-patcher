use crate::coords::LogicalFile;
use crate::isa::{encode_all, InstructionKind as K, Reg};

use super::*;

fn test_map(name: &'static str, words: usize) -> &'static ExeMap {
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

#[test]
fn 엔트리_도달_하한과_간접_분기를_분리한다() {
    let map = test_map("A.EXE", 5);
    let text = encode_all(&[
        K::Jal {
            target: 0x8001_0010,
        },
        nop(),
        K::Jr { rs: Reg::T0 },
        nop(),
        K::Jr { rs: Reg::RA },
    ]);
    let summaries = audit_executable_traces([(map, text.as_slice())]).unwrap();
    let summary = &summaries[0];

    assert_eq!(summary.reached_instructions, 5);
    assert_eq!(summary.call_targets, 1);
    assert_eq!(summary.internal_call_targets, 1);
    assert_eq!(summary.indirect_sites, 1);
    assert!(summary.has_open_indirect_flow());
    assert_eq!(summary.unknown_instructions, 0);
}

#[test]
fn trace_digest는_load_addr가_달라도_같은_offset과_word면_같다() {
    let first = test_map("A.EXE", 2);
    let second = Box::leak(Box::new(ExeMap {
        name: "B.EXE",
        load_addr: 0x8002_0000,
        text_len: 8,
        entry_pc: 0x8002_0000,
        file: LogicalFile::new(0, 0x808),
    }));
    let first_text = encode_all(&[K::Jr { rs: Reg::RA }, nop()]);
    let second_text = first_text.clone();
    let summaries = audit_executable_traces([
        (first, first_text.as_slice()),
        (second, second_text.as_slice()),
    ])
    .unwrap();

    assert_eq!(summaries[0].trace_sha256, summaries[1].trace_sha256);
}

#[test]
fn 길이_중복_빈_입력을_실패시킨다() {
    assert_eq!(
        audit_executable_traces(std::iter::empty()),
        Err(TraceInventoryError::EmptyInput)
    );

    let map = test_map("A.EXE", 1);
    assert!(matches!(
        audit_executable_traces([(map, &[][..])]),
        Err(TraceInventoryError::TextLengthMismatch { .. })
    ));

    let text = encode_all(&[K::Jr { rs: Reg::RA }]);
    assert_eq!(
        audit_executable_traces([(map, text.as_slice()), (map, text.as_slice())]),
        Err(TraceInventoryError::DuplicateExecutable("A.EXE"))
    );
}

#[test]
fn tsv는_판정_경계와_digest를_결정적으로_노출한다() {
    let summary = ExecutableTraceSummary {
        exe: "A.EXE",
        text_bytes: 4,
        reached_instructions: 1,
        decoded_instructions: 1,
        unknown_instructions: 0,
        call_targets: 0,
        internal_call_targets: 0,
        external_call_targets: 0,
        indirect_sites: 1,
        out_of_range_edges: 0,
        trace_sha256: [0xAB; 32],
    };
    let tsv = render_executable_trace_tsv(&[summary]);

    assert!(tsv.starts_with("exe\ttext_bytes\treached_instructions"));
    assert!(tsv.contains("\ttrue\tabababababababababababababababab"));
    assert!(tsv.ends_with('\n'));
}
