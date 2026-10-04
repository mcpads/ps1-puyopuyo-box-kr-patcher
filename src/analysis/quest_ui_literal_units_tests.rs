use sha2::{Digest, Sha256};

use crate::disasm::{
    QuestUiLiteral, QuestUiLiteralRole, QuestUiLiteralSummary, QuestUiLiteralSurface,
};
use crate::text::encode_sjis;

use super::*;

const SPECS: [(QuestUiLiteralRole, u32, u32, &str); 14] = [
    (
        QuestUiLiteralRole::PlumberVillage,
        0x8001_236C,
        0x8001_237C,
        "プランバーの村",
    ),
    (
        QuestUiLiteralRole::SkeletonTCave,
        0x8001_237C,
        0x8001_238C,
        "スケＴの洞くつ",
    ),
    (
        QuestUiLiteralRole::NeighborTownBridge,
        0x8001_238C,
        0x8001_239C,
        "となり町への橋",
    ),
    (
        QuestUiLiteralRole::CherrybellTown,
        0x8001_239C,
        0x8001_23AC,
        "チェリベルの町",
    ),
    (
        QuestUiLiteralRole::HappinessTower,
        0x8001_23AC,
        0x8001_23BC,
        "しあわせの塔",
    ),
    (
        QuestUiLiteralRole::HarpyForest,
        0x8001_23BC,
        0x8001_23CC,
        "ハーピーの森",
    ),
    (
        QuestUiLiteralRole::MountainPassCave,
        0x8001_23CC,
        0x8001_23D8,
        "峠の洞くつ",
    ),
    (
        QuestUiLiteralRole::SevenleaseVillage,
        0x8001_23D8,
        0x8001_23EC,
        "セブンリースの村",
    ),
    (
        QuestUiLiteralRole::ZohDaimaohRuins,
        0x8001_23EC,
        0x8001_2400,
        "ぞう大魔王の遺跡",
    ),
    (
        QuestUiLiteralRole::PakistaShop,
        0x8001_2400,
        0x8001_2410,
        "パキスタの店",
    ),
    (
        QuestUiLiteralRole::PararaShop,
        0x8001_2410,
        0x8001_241C,
        "パララの店",
    ),
    (
        QuestUiLiteralRole::SasorimanShop,
        0x8001_241C,
        0x8001_242C,
        "さそりまんの店",
    ),
    (
        QuestUiLiteralRole::ChoiceYes,
        0x800B_E0F8,
        0x800B_E100,
        "はい",
    ),
    (
        QuestUiLiteralRole::ChoiceNo,
        0x800B_E100,
        0x800B_E108,
        "いいえ",
    ),
];

fn fixture_summary() -> QuestUiLiteralSummary {
    let literals = SPECS
        .iter()
        .copied()
        .chain(
            crate::disasm::QUEST_MENU_LITERAL_SPECS
                .iter()
                .map(|&(role, a, b, t)| (QuestUiLiteralRole::MenuText(role), a, b, t)),
        )
        .collect::<Vec<_>>()
        .iter()
        .map(|(role, runtime, end_runtime, source_text)| {
            let mut source_slot = encode_sjis(source_text).expect("SJIS");
            source_slot.push(0);
            source_slot.resize((end_runtime - runtime) as usize, 0);
            QuestUiLiteral {
                role: *role,
                surface: role.surface(),
                runtime: *runtime,
                end_runtime: *end_runtime,
                source_text: (*source_text).to_owned(),
                source_slot,
            }
        })
        .collect::<Vec<_>>();
    QuestUiLiteralSummary {
        source_bytes: literals
            .iter()
            .map(|literal| literal.source_slot.len())
            .sum(),
        literals,
        world_map_labels: 12,
        binary_choices: 2,
        world_map_state_values: vec![0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12],
        world_map_dispatch_table_runtime: 0x8001_28F0,
        world_map_whole_string_call_runtime: 0x8006_63E0,
        binary_choice_reference_pairs: 4,
        binary_choice_direct_glyph_call_runtimes: vec![
            0x8003_5F54,
            0x8003_5FE8,
            0x8003_7730,
            0x8003_77E4,
            0x8003_7DA8,
            0x8003_7E3C,
            0x8003_9B04,
            0x8003_9B98,
        ],
    }
}

#[test]
fn ui_literal을_padding_포함_digest에_결속한다() {
    let analysis = verify_box_quest_ui_literal_units(&fixture_summary()).expect("units");
    assert_eq!(analysis.units.len(), BOX_QUEST_UI_LITERAL_UNITS);
    assert_eq!(analysis.world_map_labels, 12);
    assert_eq!(analysis.binary_choices, 2);
    assert_eq!(
        analysis.units[0].id,
        "QUEST-UI-LITERAL:world-map-label:plumber-village:0x8001236C"
    );
    let expected_digest: [u8; 32] = Sha256::digest(&analysis.units[0].source_slot).into();
    assert_eq!(analysis.units[0].source_sha256, expected_digest);
}

#[test]
fn surface나_분모_drift는_거부한다() {
    let mut summary = fixture_summary();
    summary.literals[0].surface = QuestUiLiteralSurface::BinaryChoice;
    assert!(audit_quest_ui_literal_units(&summary).is_err());

    let mut summary = fixture_summary();
    summary.literals.pop();
    summary.binary_choices = 1;
    summary.source_bytes = summary
        .literals
        .iter()
        .map(|literal| literal.source_slot.len())
        .sum();
    assert!(verify_box_quest_ui_literal_units(&summary).is_err());
}
