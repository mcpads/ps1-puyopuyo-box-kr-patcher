use crate::analysis::{QuestItemAcquisitionTextUnit, QuestItemAcquisitionTextUnitAnalysis};
use crate::disasm::QuestItemAcquisitionMessageRole;

use super::{bind_rows, parse_rows, COMPOSITIONS};

fn units_for_embedded_rows() -> QuestItemAcquisitionTextUnitAnalysis {
    let roles = [
        QuestItemAcquisitionMessageRole::CurrencyAcquiredSuffix,
        QuestItemAcquisitionMessageRole::ItemAcquiredSuffix,
        QuestItemAcquisitionMessageRole::InventoryFullNotice,
        QuestItemAcquisitionMessageRole::DiscardPrompt,
        QuestItemAcquisitionMessageRole::AbandonSubject,
        QuestItemAcquisitionMessageRole::AbandonParticle,
        QuestItemAcquisitionMessageRole::AbandonPredicate,
    ];
    let rows = parse_rows(COMPOSITIONS).expect("embedded rows");
    let units = rows
        .into_iter()
        .zip(roles)
        .enumerate()
        .map(|(index, (row, role))| QuestItemAcquisitionTextUnit {
            id: row.unit_id,
            role,
            runtime: 0x8001_0000 + index as u32 * 4,
            source_text: "원문".to_owned(),
            source: vec![1, 2, 0],
            source_sha256: row.source_sha256,
        })
        .collect::<Vec<_>>();
    QuestItemAcquisitionTextUnitAnalysis {
        source_bytes: units.iter().map(|unit| unit.source.len()).sum(),
        units,
    }
}

#[test]
fn 빈_particle은_명시적_수동_생략으로_받는다() {
    let units = units_for_embedded_rows();
    let analysis = bind_rows(parse_rows(COMPOSITIONS).expect("rows"), &units).expect("binding");
    let particle = analysis
        .compositions
        .iter()
        .find(|composition| composition.unit_id.contains("abandon-particle"))
        .expect("particle");
    assert!(particle.selected_text.is_empty());
    assert!(!analysis.used_glyphs.is_empty());
}

#[test]
fn source_unit이_하나라도_늘면_수동_composition_누락으로_실패한다() {
    let mut units = units_for_embedded_rows();
    units.units.push(QuestItemAcquisitionTextUnit {
        id: "extra".to_owned(),
        role: QuestItemAcquisitionMessageRole::AbandonParticle,
        runtime: 0x8001_F000,
        source_text: "追加".to_owned(),
        source: vec![1, 2, 0],
        source_sha256: [0; 32],
    });
    let error = bind_rows(parse_rows(COMPOSITIONS).expect("rows"), &units)
        .expect_err("missing composition");
    assert!(error.to_string().contains("composition 누락"));
}
