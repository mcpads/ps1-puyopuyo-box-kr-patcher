use crate::analysis::{QuestUiLiteralUnit, QuestUiLiteralUnitAnalysis};
use crate::disasm::{QuestUiLiteralRole, QuestUiLiteralSurface};

use super::{bind_rows, parse_rows, COMPOSITIONS};

fn embedded_rows() -> Vec<super::CompositionRow> {
    let mut rows = parse_rows(COMPOSITIONS).expect("map/choice rows");
    rows.extend(
        parse_rows(include_str!(
            "../../assets/translations/quest_menu_literal_compositions.tsv"
        ))
        .expect("menu rows"),
    );
    rows.extend(
        parse_rows(include_str!(
            "../../assets/translations/quest_card_flight_literal_compositions.tsv"
        ))
        .expect("card/flight rows"),
    );
    rows
}

fn units_for_embedded_rows() -> QuestUiLiteralUnitAnalysis {
    let roles = [
        QuestUiLiteralRole::PlumberVillage,
        QuestUiLiteralRole::SkeletonTCave,
        QuestUiLiteralRole::NeighborTownBridge,
        QuestUiLiteralRole::CherrybellTown,
        QuestUiLiteralRole::HappinessTower,
        QuestUiLiteralRole::HarpyForest,
        QuestUiLiteralRole::MountainPassCave,
        QuestUiLiteralRole::SevenleaseVillage,
        QuestUiLiteralRole::ZohDaimaohRuins,
        QuestUiLiteralRole::PakistaShop,
        QuestUiLiteralRole::PararaShop,
        QuestUiLiteralRole::SasorimanShop,
        QuestUiLiteralRole::ChoiceYes,
        QuestUiLiteralRole::ChoiceNo,
    ];
    let roles = roles.into_iter().chain(
        crate::disasm::QUEST_MENU_LITERAL_SPECS
            .iter()
            .map(|s| QuestUiLiteralRole::MenuText(s.0)),
    );
    let rows = embedded_rows();
    let units = rows
        .into_iter()
        .zip(roles)
        .enumerate()
        .map(|(index, (row, role))| QuestUiLiteralUnit {
            id: row.unit_id,
            role,
            surface: role.surface(),
            runtime: 0x8001_0000 + index as u32 * 8,
            end_runtime: 0x8001_0008 + index as u32 * 8,
            source_text: "원문".to_owned(),
            source_slot: vec![1, 2, 0, 0, 0, 0, 0, 0],
            source_sha256: row.source_sha256,
        })
        .collect::<Vec<_>>();
    QuestUiLiteralUnitAnalysis {
        source_bytes: units.iter().map(|unit| unit.source_slot.len()).sum(),
        world_map_labels: units
            .iter()
            .filter(|unit| unit.surface == QuestUiLiteralSurface::WorldMapLabel)
            .count(),
        binary_choices: units
            .iter()
            .filter(|unit| unit.surface == QuestUiLiteralSurface::BinaryChoice)
            .count(),
        units,
    }
}

#[test]
fn ui_literal에_수동_문구를_전수_결속한다() {
    let units = units_for_embedded_rows();
    let analysis = bind_rows(embedded_rows(), &units).expect("binding");
    assert_eq!(
        analysis.compositions.len(),
        crate::analysis::BOX_QUEST_UI_LITERAL_UNITS
    );
    assert!(analysis.used_glyphs.contains('착'));
    assert!(analysis.used_glyphs.contains('예'));
    assert!(analysis.used_glyphs.contains('동'));
    assert!(!analysis.translation_approved);
    assert!(!analysis.layout_approved);
    assert!(!analysis.runtime_verified);
    assert!(!analysis.product_eligible);
}

#[test]
fn source_unit_추가와_자동_개행은_거부한다() {
    let mut units = units_for_embedded_rows();
    let mut extra = units.units[0].clone();
    extra.id = "extra".to_owned();
    extra.runtime = 0x8001_F000;
    extra.end_runtime = 0x8001_F008;
    units.units.push(extra);
    assert!(bind_rows(embedded_rows(), &units).is_err());

    let mut input = COMPOSITIONS.to_owned();
    input = input.replacen("플럼버 마을", "플럼버\n마을", 1);
    assert!(parse_rows(&input).is_err());
}
