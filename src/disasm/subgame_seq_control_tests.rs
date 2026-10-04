use super::*;

#[test]
fn code별_인자_폭_분모를_고정한다() {
    assert_eq!(BOX_SUBGAME_CONTROL_BEHAVIORS.len(), 48);
    assert_eq!(
        BOX_SUBGAME_CONTROL_BEHAVIORS[0x12],
        SubgameControlBehavior::DynamicSelect { header_bytes: 10 }
    );
    assert_eq!(
        BOX_SUBGAME_CONTROL_BEHAVIORS[0x15],
        SubgameControlBehavior::DynamicSelect { header_bytes: 8 }
    );
    assert_eq!(
        BOX_SUBGAME_CONTROL_BEHAVIORS[0x08],
        SubgameControlBehavior::CallAbsolute { pointer_bytes: 4 }
    );
}

#[test]
fn absolute_pointer_조각을_4바이트로_합친다() {
    assert_eq!(
        combine_absolute(
            Expr::Argument {
                offset: 0,
                bytes: 2,
            },
            Expr::ArgumentHigh {
                offset: 2,
                bytes: 2,
            },
        ),
        Some(Expr::Absolute { argument_bytes: 4 })
    );
}

#[test]
fn register_state_merge는_불일치를_unknown으로_만든다() {
    let mut left = RegisterState::initial();
    left.set(Reg::V0, Expr::Stream(2));
    let mut right = RegisterState::initial();
    right.set(Reg::V0, Expr::Stream(4));
    assert_eq!(left.merge(&right).get(Reg::V0), Expr::Unknown);
    assert_eq!(left.merge(&right).get(Reg::ZERO), Expr::Zero);
}

#[test]
fn tsv는_보호_contract와_인자_폭을_낸다() {
    let analysis = SubgameControlAnalysis {
        sites: vec![SubgameControlSite {
            exe: "TEST.EXE",
            code: 0x12,
            handler_runtime: 0x8001_0000,
            behavior: SubgameControlBehavior::DynamicSelect { header_bytes: 10 },
            reached_instructions: 7,
            trap_paths: 0,
        }],
    };
    let report = render_subgame_seq_controls_tsv(&analysis);
    assert!(report.contains("TEST.EXE\tFF12\t0x80010000\tdynamic-select\t10"));
    assert!(report.contains("preserve-control-and-arguments"));
}
