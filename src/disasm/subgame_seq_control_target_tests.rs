use super::*;

#[test]
fn 동적_control의_생성_상한을_분리한다() {
    assert_eq!(BOX_SUBGAME_SEQ_FF12_GENERATED_BYTES, 22);
    assert_eq!(BOX_SUBGAME_SEQ_FF15_GUARD_EXCLUSIVE_HALFWORDS, 385);
    assert_eq!(BOX_SUBGAME_SEQ_FF15_MAX_GENERATED_BYTES, 768);
    assert_eq!(BOX_SUBGAME_SEQ_CONTROL_TARGET_SPECS.len(), 12);
}

#[test]
fn 주소_조립은_다른_목적_레지스터도_허용한다() {
    let instructions = [
        Instruction::new(
            0x8001_0000,
            0,
            K::Lui {
                rt: Reg::T0,
                imm: 0x800C,
            },
        ),
        Instruction::new(
            0x8001_0004,
            0,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::T0,
                imm: 0x1234,
            },
        ),
    ];
    assert!(loaded_addresses(&instructions).contains(&0x800C_1234));
}

#[test]
fn 보고서는_handler와_후보의_판정을_분리한다() {
    let analysis = SubgameControlTargetAnalysis {
        rows: vec![SubgameControlTargetRow {
            exe: "TEST.EXE",
            archive: "/TEST.VDD;1",
            code: 0x12,
            handler_runtime: 0x8001_0000,
            header_bytes: 10,
            target_kind: SubgameControlTargetKind::GeneratedScratch,
            scratch_runtime: Some(0x800C_0000),
            generated_bytes: Some(22),
            handler_sha256: Some([0; 32]),
        }],
        occurrences: vec![SubgameControlTargetOccurrence {
            container: "sequence",
            archive: "/TEST.VDD;1".to_string(),
            asset: "TEST.SEQ".to_string(),
            layout: "overlay",
            offset: 2,
            code: 0x08,
            header: vec![1, 2, 3, 4],
            direct_pointer: Some(0x0403_0201),
            target_offset: None,
        }],
        sequence_files: 1,
        overlays: 1,
        raw_opaque: 0,
        sequence_candidates: 1,
        exe_candidates: 0,
        internal_pointers: 0,
    };
    let report = render_subgame_seq_control_targets_tsv(&analysis);
    assert!(report.contains("generated-10-halfwords-plus-ff09-not-file-target"));
    assert!(report.contains("aligned-word-candidate-not-proven-control"));
}
