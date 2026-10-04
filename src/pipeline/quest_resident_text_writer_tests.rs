use super::*;
use crate::analysis::{QuestResidentTextRole, QuestResidentTextRun, QuestResidentTextToken};
use crate::font::QUEST_CODE_TABLE_COUNT;
use crate::script::QuestControlMeaning;

const PAGE_ID_FROM_FIXTURE_PLAN: u8 = 7;
const FIXTURE_CODE_SUPPLY_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

fn code_table() -> QuestCodeTable {
    let mut codes = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| u16::try_from(index).expect("fixture code").to_le_bytes())
        .collect::<Vec<_>>();
    codes[0] = [0x81, 0x40];
    codes[1] = [0x82, 0xA0];
    codes[2] = [0x82, 0xA2];
    codes[3] = [0x82, 0xA4];
    codes[4] = [0x82, 0xA6];
    let raw = codes.into_iter().flatten().collect::<Vec<_>>();
    QuestCodeTable::parse(&raw).expect("fixture QUEST code table")
}

fn page_codes() -> BTreeMap<u8, BTreeMap<char, [u8; 2]>> {
    BTreeMap::from([(
        PAGE_ID_FROM_FIXTURE_PLAN,
        BTreeMap::from([('가', 30u16.to_le_bytes()), ('나', 31u16.to_le_bytes())]),
    )])
}

fn source_unit(id: &str) -> QuestResidentTextUnit {
    let protected_raw = vec![
        0x21, 0x00, 0x00, 0x00, // record
        0x82, 0xA0, 0x82, 0xA2, // run 0
        0x04, 0xFF, // line control
        0x82, 0xA4, 0x82, 0xA6, // run 1
        0x03, 0xFF, 0x00, 0x00, 0x00, 0x00, // FF03
    ];
    let record_text_offset = 0x100;
    let record_runtime = 0x8001_0000;
    QuestResidentTextUnit {
        id: id.to_string(),
        role: QuestResidentTextRole::StartMenu,
        record_runtime,
        record_text_offset,
        body_runtime: record_runtime + 4,
        body_text_offset: record_text_offset + 4,
        end_runtime: record_runtime + u32::try_from(protected_raw.len()).unwrap(),
        end_text_offset: record_text_offset + protected_raw.len(),
        record_raw: [0x21, 0, 0, 0],
        terminal: QuestTextTerminal::DeactivateAndAlign,
        expected_raw_sha256: sha256(&protected_raw),
        protected_raw,
        protected_source: "あいうえ".to_string(),
        tokens: vec![
            QuestResidentTextToken {
                index: 0,
                text_offset: 0x104,
                runtime: 0x8001_0004,
                raw: vec![0x82, 0xA0],
                kind: QuestResidentTextTokenKind::Glyph,
                source: Some("あ".to_string()),
                control_code: None,
                control_meaning: None,
            },
            QuestResidentTextToken {
                index: 1,
                text_offset: 0x106,
                runtime: 0x8001_0006,
                raw: vec![0x82, 0xA2],
                kind: QuestResidentTextTokenKind::Glyph,
                source: Some("い".to_string()),
                control_code: None,
                control_meaning: None,
            },
            QuestResidentTextToken {
                index: 2,
                text_offset: 0x108,
                runtime: 0x8001_0008,
                raw: vec![0x04, 0xFF],
                kind: QuestResidentTextTokenKind::Control,
                source: None,
                control_code: Some(0x04),
                control_meaning: Some(QuestControlMeaning::AdvanceTextLine),
            },
            QuestResidentTextToken {
                index: 3,
                text_offset: 0x10A,
                runtime: 0x8001_000A,
                raw: vec![0x82, 0xA4],
                kind: QuestResidentTextTokenKind::Glyph,
                source: Some("う".to_string()),
                control_code: None,
                control_meaning: None,
            },
            QuestResidentTextToken {
                index: 4,
                text_offset: 0x10C,
                runtime: 0x8001_000C,
                raw: vec![0x82, 0xA6],
                kind: QuestResidentTextTokenKind::Glyph,
                source: Some("え".to_string()),
                control_code: None,
                control_meaning: None,
            },
            QuestResidentTextToken {
                index: 5,
                text_offset: 0x10E,
                runtime: 0x8001_000E,
                raw: vec![0x03, 0xFF, 0, 0, 0, 0],
                kind: QuestResidentTextTokenKind::Control,
                source: None,
                control_code: Some(0x03),
                control_meaning: Some(QuestControlMeaning::DeactivateInterpreterAndAlign),
            },
        ],
        runs: vec![
            QuestResidentTextRun {
                index: 0,
                text_offset: 0x104,
                runtime: 0x8001_0004,
                raw: vec![0x82, 0xA0, 0x82, 0xA2],
                source: "あい".to_string(),
                glyph_occurrences: 2,
            },
            QuestResidentTextRun {
                index: 1,
                text_offset: 0x10A,
                runtime: 0x8001_000A,
                raw: vec![0x82, 0xA4, 0x82, 0xA6],
                source: "うえ".to_string(),
                glyph_occurrences: 2,
            },
        ],
        glyph_occurrences: 4,
        japanese_glyph_occurrences: 4,
        control_occurrences: 2,
        runtime_evidence: None,
    }
}

fn composition(unit: &QuestResidentTextUnit) -> QuestResidentTextComposition {
    QuestResidentTextComposition {
        unit_id: unit.id.clone(),
        source_sha256: unit.expected_raw_sha256,
        status: QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry,
        runs: vec![
            QuestResidentTextRunComposition {
                run_index: 0,
                text: "가".to_string(),
                leading_pad_cells: 0,
                trailing_pad_cells: 1,
            },
            QuestResidentTextRunComposition {
                run_index: 1,
                text: "나".to_string(),
                leading_pad_cells: 1,
                trailing_pad_cells: 0,
            },
        ],
    }
}

fn assignment(unit: &QuestResidentTextUnit) -> QuestResidentTextPageAssignment {
    QuestResidentTextPageAssignment {
        unit_id: unit.id.clone(),
        page_id: PAGE_ID_FROM_FIXTURE_PLAN,
    }
}

#[test]
fn 수동_run과_pad만_쓰고_record_control_ff03_span을_보존한다() {
    let unit = source_unit("resident-fixture");
    let result = write_with_page_codes(
        std::slice::from_ref(&unit),
        &[composition(&unit)],
        &[assignment(&unit)],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .unwrap();
    let write = &result.writes[0];

    assert_eq!(&write.output[..4], &[0x21, PAGE_ID_FROM_FIXTURE_PLAN, 0, 0]);
    assert_eq!(&write.output[4..8], &[0x1E, 0x00, 0x81, 0x40]);
    assert_eq!(&write.output[8..10], &[0x04, 0xFF]);
    assert_eq!(&write.output[10..14], &[0x81, 0x40, 0x1F, 0x00]);
    assert_eq!(&write.output[14..], &[0x03, 0xFF, 0, 0, 0, 0]);
    assert_eq!(write.output.len(), write.source.len());
    assert_eq!(write.record_text_offset, 0x100);
    assert_eq!(write.end_text_offset, 0x114);
    assert_eq!(write.page_id, PAGE_ID_FROM_FIXTURE_PLAN);
    assert!(write.source_exact && write.controls_exact && write.terminal_exact);
    assert_eq!(write.japanese_glyphs_remaining, 0);
    assert_eq!(write.status.label(), QUEST_RESIDENT_TEXT_COMPOSITION_STATUS);
    assert!(!write.translation_approved);
    assert!(!write.layout_approved);
    assert!(!write.runtime_verified);
    assert!(!write.product_eligible);
}

#[test]
fn source_sha와_run_cell수가_다르면_실패한다() {
    let unit = source_unit("resident-fixture");
    let mut stale = composition(&unit);
    stale.source_sha256[0] ^= 0xFF;
    assert!(write_with_page_codes(
        std::slice::from_ref(&unit),
        &[stale],
        &[assignment(&unit)],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .is_err());

    let mut wrong_cells = composition(&unit);
    wrong_cells.runs[0].trailing_pad_cells = 0;
    assert!(write_with_page_codes(
        std::slice::from_ref(&unit),
        &[wrong_cells],
        &[assignment(&unit)],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .is_err());
}

#[test]
fn 일본어잔존과_미매핑glyph를_거부한다() {
    let unit = source_unit("resident-fixture");
    let mut japanese = composition(&unit);
    japanese.runs[0].text = "あ".to_string();
    assert!(write_with_page_codes(
        std::slice::from_ref(&unit),
        &[japanese],
        &[assignment(&unit)],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .is_err());

    let mut unmapped = composition(&unit);
    unmapped.runs[0].text = "라".to_string();
    assert!(write_with_page_codes(
        std::slice::from_ref(&unit),
        &[unmapped],
        &[assignment(&unit)],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .is_err());
}

#[test]
fn composition_page_run의_중복과_누락을_거부한다() {
    let unit = source_unit("resident-fixture");
    let selected = composition(&unit);
    assert!(write_with_page_codes(
        std::slice::from_ref(&unit),
        &[selected.clone(), selected.clone()],
        &[assignment(&unit)],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .is_err());

    let mut missing_run = selected;
    missing_run.runs.pop();
    assert!(write_with_page_codes(
        std::slice::from_ref(&unit),
        &[missing_run],
        &[assignment(&unit)],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .is_err());

    let page = assignment(&unit);
    assert!(write_with_page_codes(
        std::slice::from_ref(&unit),
        &[composition(&unit)],
        &[page.clone(), page],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .is_err());
}

#[test]
fn 파생page_zero는_원문route와_구분해_거부한다() {
    let unit = source_unit("resident-fixture");
    let mut page = assignment(&unit);
    page.page_id = 0;
    assert!(write_with_page_codes(
        std::slice::from_ref(&unit),
        &[composition(&unit)],
        &[page],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .is_err());
}

#[test]
fn glyph_pair를_바꾸고_sha를_다시써도_writer_provenance가_거부한다() {
    let unit = source_unit("resident-fixture");
    let mut result = write_with_page_codes(
        std::slice::from_ref(&unit),
        &[composition(&unit)],
        &[assignment(&unit)],
        &page_codes(),
        &code_table(),
        FIXTURE_CODE_SUPPLY_SHA256,
    )
    .unwrap();
    result.writes[0].output[4..6].copy_from_slice(&32u16.to_le_bytes());
    result.writes[0].output_sha256 = sha256_hex(&result.writes[0].output);

    let error = verify_quest_resident_text_writer_provenance(&result).unwrap_err();
    assert!(error.to_string().contains("provenance 불일치"));
}
