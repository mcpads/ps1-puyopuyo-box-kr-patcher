use super::*;
use crate::font::{QuestCodeTable, QUEST_CODE_TABLE_BYTES};
use crate::script::{audit_quest_item_text_units, QuestItemTextTerminal};

fn code_table(codes: &[[u8; 2]]) -> QuestCodeTable {
    let mut raw = Vec::with_capacity(QUEST_CODE_TABLE_BYTES);
    raw.extend_from_slice(codes.as_flattened());
    for index in codes.len()..QUEST_CODE_TABLE_BYTES / 2 {
        raw.extend_from_slice(&[0x80 | ((index >> 8) as u8), index as u8]);
    }
    QuestCodeTable::parse(&raw).expect("고유 code table")
}

fn source(padding: usize) -> (Vec<u8>, QuestItemTextUnitAnalysis) {
    let base = 0x8010_0000u32;
    let mut raw = vec![0u8; 0x80 + padding];
    let record_offset = 8usize;
    raw[..4].copy_from_slice(&(base + record_offset as u32).to_le_bytes());
    raw[4..8].copy_from_slice(&(base + record_offset as u32).to_le_bytes());
    let name_offset = 0x30usize;
    let description_offset = 0x40usize;
    raw[record_offset + 12..record_offset + 16]
        .copy_from_slice(&(base + name_offset as u32).to_le_bytes());
    raw[record_offset + 16..record_offset + 20]
        .copy_from_slice(&(base + description_offset as u32).to_le_bytes());
    raw[name_offset..name_offset + 6].copy_from_slice(&[0x83, 0x4A, 0x83, 0x4D, 0, 0]);
    raw[description_offset..description_offset + 12]
        .copy_from_slice(&[0x0B, 0xFF, 0, 0, 0x82, 0xA0, 0x1A, 0xFF, 0, 0, 0, 0]);
    let table = code_table(&[[0x83, 0x4A], [0x83, 0x4D], [0x82, 0xA0]]);
    let analysis = audit_quest_item_text_units(&raw, base, 2, &table).unwrap();
    (raw, analysis)
}

#[test]
fn 이름은_nul원문을_옮기고_설명은_page0_op21을_앞에붙인다() {
    let (raw, analysis) = source(0);
    let control = plan_quest_item_text_relink_control(&raw, &analysis).unwrap();

    assert_eq!(control.pointer_writes, 2);
    assert_eq!(control.name_units, 1);
    assert_eq!(control.description_units, 1);
    assert_eq!(control.description_op21_records, 1);
    assert!(control.source_exact_outside_relinked_fields);
    let name = control
        .relocated_units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Name)
        .unwrap();
    assert_eq!(name.relocated_record_offset, None);
    assert_eq!(
        control
            .output
            .get(name.relocated_text_offset..name.relocated_text_offset + name.protected_bytes),
        Some([0x83, 0x4A, 0x83, 0x4D, 0, 0].as_slice())
    );
    let description = control
        .relocated_units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Description)
        .unwrap();
    let record = description.relocated_record_offset.unwrap();
    assert_eq!(
        control.output.get(record..record + 4),
        Some([QUEST_SEQ_TEXT_MODE_OPCODE, 0, 0, 0].as_slice())
    );
    assert_eq!(
        analysis
            .units
            .iter()
            .find(|unit| unit.role == QuestItemTextRole::Description)
            .unwrap()
            .terminal,
        QuestItemTextTerminal::Text(crate::script::QuestTextTerminal::NotifyParentAndRelease)
    );
}

#[test]
fn 보호원문_drift와_active_window초과를_실패폐쇄한다() {
    let (mut raw, analysis) = source(0);
    raw[0x30] ^= 1;
    assert!(plan_quest_item_text_relink_control(&raw, &analysis).is_err());

    let (large, large_analysis) = source(QUEST_ITEM_TEXT_WINDOW_BYTES - 0x80 - 4);
    assert!(plan_quest_item_text_relink_control(&large, &large_analysis).is_err());
}

#[test]
fn 뒤가_다른자원으로_가득차도_번역을_원문자리에_넣고_보호영역을_유지한다() {
    // append-only는 MCRD1을 침범한다. 짧은 번역과 OP21 metadata는
    // 확인된 원문 자리 안에 들어가므로 파일을 늘리지 않아야 한다.
    let (raw, analysis) = source(QUEST_ITEM_TEXT_WINDOW_BYTES - 0x80);
    let name = analysis
        .units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Name)
        .unwrap();
    let description = analysis
        .units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Description)
        .unwrap();
    let names = BTreeMap::from([(name.id.clone(), vec![0x90, 1, 0, 0])]);
    let descriptions = BTreeMap::from([(
        description.id.clone(),
        QuestItemDescriptionRelinkReplacement {
            page_id: 7,
            body: vec![0x1A, 0xFF, 0, 0, 0, 0],
        },
    )]);
    let result = plan_quest_item_text_relink_control_with_replacements(
        &raw,
        &analysis,
        &names,
        &descriptions,
    )
    .unwrap();
    assert_eq!(result.output.len(), raw.len());
    assert_eq!(result.active_window_remaining, 0);
    for (offset, byte) in raw.iter().enumerate() {
        let relink_field = analysis.units.iter().any(|unit| {
            (unit.pointer_field_offset..unit.pointer_field_offset + 4).contains(&offset)
                || (unit.text_offset..unit.end_offset).contains(&offset)
        });
        if !relink_field {
            assert_eq!(result.output[offset], *byte);
        }
    }
    for unit in &result.relocated_units {
        assert!(unit.relocated_text_offset + unit.protected_bytes <= QUEST_ITEM_TEXT_WINDOW_BYTES);
    }
}

#[test]
fn 재사용대상이_record_header나_다른_text와_겹치면_거부한다() {
    let (raw, analysis) = source(0);
    for offset in [8, 0x30] {
        let mut altered = analysis.clone();
        let description = altered
            .units
            .iter_mut()
            .find(|unit| unit.role == QuestItemTextRole::Description)
            .unwrap();
        let mut source = raw.clone();
        description.text_offset = offset;
        description.end_offset = offset + 4;
        description.text_runtime = 0x8010_0000 + offset as u32;
        source[description.pointer_field_offset..description.pointer_field_offset + 4]
            .copy_from_slice(&description.text_runtime.to_le_bytes());
        description.protected_raw = source[offset..offset + 4].to_vec();
        assert!(plan_quest_item_text_relink_control(&source, &altered).is_err());
    }
}

#[test]
fn record의_이름이나_설명이_빠지면_부분_relink를_만들지_않는다() {
    let (raw, mut analysis) = source(0);
    analysis
        .units
        .retain(|unit| unit.role == QuestItemTextRole::Name);

    assert!(plan_quest_item_text_relink_control(&raw, &analysis).is_err());
}

#[test]
fn 이름전수만_다른_nul열로_바꾸고_설명원문을_보존한다() {
    let (raw, analysis) = source(0);
    let name = analysis
        .units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Name)
        .unwrap();
    let replacements = BTreeMap::from([(name.id.clone(), vec![0x90, 1, 0, 0])]);
    let control =
        plan_quest_item_text_relink_control_with_names(&raw, &analysis, &replacements).unwrap();
    let relocated_name = control
        .relocated_units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Name)
        .unwrap();
    assert_eq!(relocated_name.source_bytes, 6);
    assert_eq!(relocated_name.protected_bytes, 4);
    assert_eq!(
        control.output.get(
            relocated_name.relocated_text_offset
                ..relocated_name.relocated_text_offset + relocated_name.protected_bytes
        ),
        Some([0x90, 1, 0, 0].as_slice())
    );
    let description = control
        .relocated_units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Description)
        .unwrap();
    let source_description = analysis
        .units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Description)
        .unwrap();
    assert_eq!(
        control.output.get(
            description.relocated_text_offset
                ..description.relocated_text_offset + description.protected_bytes
        ),
        Some(source_description.protected_raw.as_slice())
    );
}

#[test]
fn 이름replacement의_부분집합과_잘못된_nul을_거부한다() {
    let (raw, analysis) = source(0);
    let name = analysis
        .units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Name)
        .unwrap();
    assert!(plan_quest_item_text_relink_control_with_names(
        &raw,
        &analysis,
        &BTreeMap::from([(name.id.clone(), vec![0x90, 1])])
    )
    .is_err());
    assert!(plan_quest_item_text_relink_control_with_names(
        &raw,
        &analysis,
        &BTreeMap::from([(name.id.clone(), vec![0, 0, 0, 0])])
    )
    .is_err());
}

#[test]
fn 이름과설명replacement를_한결과로합성하고_page와종단을검증한다() {
    let (raw, analysis) = source(0);
    let name = analysis
        .units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Name)
        .unwrap();
    let description = analysis
        .units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Description)
        .unwrap();
    let names = BTreeMap::from([(name.id.clone(), vec![0x90, 1, 0, 0])]);
    let body = vec![0x0B, 0xFF, 0, 0, 0x90, 2, 0x1A, 0xFF, 0, 0, 0, 0];
    let descriptions = BTreeMap::from([(
        description.id.clone(),
        QuestItemDescriptionRelinkReplacement {
            page_id: 9,
            body: body.clone(),
        },
    )]);
    let control = plan_quest_item_text_relink_control_with_replacements(
        &raw,
        &analysis,
        &names,
        &descriptions,
    )
    .unwrap();
    let relocated = control
        .relocated_units
        .iter()
        .find(|unit| unit.role == QuestItemTextRole::Description)
        .unwrap();
    let record = relocated.relocated_record_offset.unwrap();
    assert_eq!(
        control.output.get(record..record + 4),
        Some([QUEST_SEQ_TEXT_MODE_OPCODE, 9, 0, 0].as_slice())
    );
    assert_eq!(relocated.relocated_text_offset, record + 4);
    assert_eq!(
        control
            .output
            .get(relocated.pointer_field_offset..relocated.pointer_field_offset + 4),
        Some(
            (0x8010_0000u32 + relocated.relocated_text_offset as u32)
                .to_le_bytes()
                .as_slice()
        )
    );
    assert_eq!(
        control.output.get(
            relocated.relocated_text_offset
                ..relocated.relocated_text_offset + relocated.protected_bytes
        ),
        Some(body.as_slice())
    );

    let wrong_id = BTreeMap::from([(
        "missing-description".to_owned(),
        QuestItemDescriptionRelinkReplacement {
            page_id: 9,
            body: body.clone(),
        },
    )]);
    assert!(plan_quest_item_text_relink_control_with_replacements(
        &raw, &analysis, &names, &wrong_id,
    )
    .is_err());
    for invalid in [
        QuestItemDescriptionRelinkReplacement {
            page_id: 0,
            body: body.clone(),
        },
        QuestItemDescriptionRelinkReplacement {
            page_id: 9,
            body: vec![0x90, 2],
        },
    ] {
        let replacements = BTreeMap::from([(description.id.clone(), invalid)]);
        assert!(plan_quest_item_text_relink_control_with_replacements(
            &raw,
            &analysis,
            &names,
            &replacements,
        )
        .is_err());
    }
}
