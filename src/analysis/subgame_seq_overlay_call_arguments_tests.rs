use super::{render_subgame_seq_overlay_call_arguments_tsv, SubgameSeqOverlayCallArgumentAnalysis};

#[test]
fn 빈_flow도_전수_분모와_음성_경계를_보고한다() {
    let report =
        render_subgame_seq_overlay_call_arguments_tsv(&SubgameSeqOverlayCallArgumentAnalysis {
            sequence_files: 324,
            overlay_files: 207,
            rows: Vec::new(),
        });
    assert!(report.contains("summary\tsubgame-seq-overlay-call-argument-v1\t324\t207\t0\t0"));
    assert!(report.contains("stack-slot-flow-census-only"));
    assert!(!report.lines().any(|line| line.starts_with("flow\t")));
}
