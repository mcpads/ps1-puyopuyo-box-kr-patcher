use super::*;
use crate::coords::LogicalFile;

fn test_map(len: usize) -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: len as u32,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, u32::try_from(len + 0x800).unwrap()),
    }
}

#[test]
fn zero_run은_4바이트_안쪽경계와_64바이트_최소폭을_요구한다() {
    let map = test_map(256);
    let mut text = vec![0xFF; 256];
    text[3..70].fill(0);
    text[100..163].fill(0);
    text[180..248].fill(0);
    assert_eq!(
        aligned_zero_runs(&map, &text, 0x8001_0100).unwrap(),
        vec![(0x8001_0004, 0x8001_0044), (0x8001_00B4, 0x8001_00F8)]
    );
}

#[test]
fn candidate_digest는_주소_참조수와_판정을_모두_포함한다() {
    let candidate = QuestHookSpaceCandidate {
        start_runtime: 0x8002_0000,
        end_runtime: 0x8002_0100,
        bytes: 256,
        entry_reached_instructions: 0,
        direct_control_targets: 0,
        address_loads: 0,
        aligned_pointer_words: 0,
        static_verdict: QuestHookCandidateStaticVerdict::UnreferencedNeedsRuntime,
    };
    let first = digest_candidates(&[candidate]);
    let second = digest_candidates(&[QuestHookSpaceCandidate {
        aligned_pointer_words: 1,
        static_verdict: QuestHookCandidateStaticVerdict::ReferencedNeedsConsumer,
        ..candidate
    }]);
    assert_ne!(first, second);
}

#[test]
fn tsv는_정적하한과_runtime미판정을_분리한다() {
    let candidate = QuestHookSpaceCandidate {
        start_runtime: 0x8002_0000,
        end_runtime: 0x8002_0080,
        bytes: 128,
        entry_reached_instructions: 0,
        direct_control_targets: 0,
        address_loads: 0,
        aligned_pointer_words: 0,
        static_verdict: QuestHookCandidateStaticVerdict::UnreferencedNeedsRuntime,
    };
    let analysis = QuestHookCandidateAnalysis {
        candidates: vec![candidate],
        total_zero_bytes: 128,
        entry_reached_candidates: 0,
        referenced_candidates: 0,
        unreferenced_candidates: 1,
        indirect_sites: 3,
        analysis_sha256: digest_candidates(&[candidate]),
    };
    let report = render_quest_hook_space_candidates_tsv(&analysis);
    assert!(report.contains("unreferenced-needs-runtime\tneeds-write-exec-observation"));
}
