use super::*;
use crate::analysis::{
    QuestResidentSeqFrontierRequirement, QuestResidentSeqPath, QuestResidentSeqPathStatus,
};
use crate::coords::LogicalFile;
use crate::font::QUEST_CODE_TABLE_COUNT;

const FIXTURE_LOAD_ADDR: u32 = 0x8008_0000;
const FIXTURE_TEXT_LEN: usize = 0xD000;

fn map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: FIXTURE_LOAD_ADDR,
        text_len: u32::try_from(FIXTURE_TEXT_LEN).expect("fixture text len"),
        entry_pc: FIXTURE_LOAD_ADDR,
        file: LogicalFile::new(0, u32::try_from(FIXTURE_TEXT_LEN + 0x800).unwrap()),
    }
}

fn code_table_with(first: [u8; 2]) -> QuestCodeTable {
    let mut codes = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| u16::try_from(index).expect("fixture code").to_le_bytes())
        .collect::<Vec<_>>();
    codes[0] = first;
    QuestCodeTable::parse(&codes.into_iter().flatten().collect::<Vec<_>>())
        .expect("fixture code table")
}

fn text_with_glyph(code: [u8; 2]) -> Vec<u8> {
    let mut text = vec![0; FIXTURE_TEXT_LEN];
    for record_runtime in [0x8008_9650, 0x8008_97F8, 0x8008_C4A0] {
        let record = usize::try_from(record_runtime - FIXTURE_LOAD_ADDR).unwrap();
        text[record..record + 4].copy_from_slice(&[0x21, 0, 0, 0]);
        text[record + 4..record + 6].copy_from_slice(&code);
        // 세 주소 모두 FF03 시작이 2 mod 4라 reserved 4바이트까지 6바이트다.
        text[record + 6..record + 12].copy_from_slice(&[0x03, 0xFF, 0, 0, 0, 0]);
    }
    text
}

fn text_node(record_runtime: u32) -> QuestResidentSeqPathNode {
    QuestResidentSeqPathNode {
        runtime: record_runtime,
        opcode: 0x21,
        transition: QuestResidentSeqPathTransition::TextDeactivateAndAlign,
        next_runtime: Some(record_runtime + 12),
        side_stream_runtime: None,
        child_stream_runtime: None,
        text_terminal: Some(QuestTextTerminal::DeactivateAndAlign),
        text_glyphs: 1,
        text_controls: 1,
    }
}

fn path(root_runtime: u32, nodes: Vec<QuestResidentSeqPathNode>) -> QuestResidentSeqPath {
    QuestResidentSeqPath {
        root_runtime,
        install_sites: 0,
        nodes,
        status: QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier,
        frontier_requirement: Some(QuestResidentSeqFrontierRequirement::HandlerState),
    }
}

fn graph() -> (
    QuestResidentSeqPathSummary,
    QuestResidentSeqSpawnPathSummary,
) {
    let paths = QuestResidentSeqPathSummary {
        paths: vec![
            path(0x8008_97F8, vec![text_node(0x8008_97F8)]),
            path(0x8008_C4A0, vec![text_node(0x8008_C4A0)]),
        ],
        install_sites: 0,
        nodes: 2,
        op21_records: 2,
        text_glyphs: 2,
        text_controls: 2,
        released_paths: 0,
        runtime_frontiers: 2,
    };
    let spawn_path = path(0x8008_9650, vec![text_node(0x8008_9650)]);
    let deferred_path = QuestResidentSeqPath {
        root_runtime: 0x8008_98D0,
        install_sites: 0,
        nodes: Vec::new(),
        status: QuestResidentSeqPathStatus::ReleasedInterpreter,
        frontier_requirement: None,
    };
    let spawn = QuestResidentSeqSpawnPathSummary {
        parent_record_runtime: 0x8008_9898,
        spawned_stream_runtime: 0x8008_9648,
        path: spawn_path,
        deferred_stream_runtime: 0x8008_98D0,
        deferred_path,
        menu_branch_runtime: 0x8008_96CC,
        menu_selection_field_offset: 0x34,
        menu_selection_count: 3,
        new_game_selection_index: 1,
        new_game_stream_runtime: 0x8008_96DC,
        text_glyphs: 1,
        text_controls: 1,
    };
    (paths, spawn)
}

#[test]
fn graph가_가리킨_op21만_가역_원문단위로_만든다() {
    let map = map();
    let text = text_with_glyph([0x82, 0xA0]);
    let code_table = code_table_with([0x82, 0xA0]);
    let (paths, spawn) = graph();

    let analysis =
        audit_quest_resident_text_units(&map, &text, &code_table, &paths, &spawn).unwrap();

    assert_eq!(
        analysis
            .units
            .iter()
            .map(|unit| unit.role)
            .collect::<Vec<_>>(),
        [
            QuestResidentTextRole::StartMenu,
            QuestResidentTextRole::Farewell,
            QuestResidentTextRole::Defeat,
        ]
    );
    for unit in &analysis.units {
        assert_eq!(unit.record_raw, [0x21, 0, 0, 0]);
        assert_eq!(unit.protected_source, "あ");
        assert_eq!(unit.glyph_occurrences, 1);
        assert_eq!(unit.control_occurrences, 1);
        assert_eq!(unit.runs.len(), 1);
        assert_eq!(unit.runs[0].source, "あ");
        assert_eq!(unit.tokens.len(), 2);
        assert_eq!(unit.tokens[0].kind, QuestResidentTextTokenKind::Glyph);
        assert_eq!(unit.tokens[1].kind, QuestResidentTextTokenKind::Control);
        assert_eq!(unit.tokens[1].control_code, Some(0x03));
        assert_eq!(unit.end_runtime, unit.record_runtime + 12);
        assert!(unit.runtime_evidence.is_none());

        let mut serialized = unit.record_raw.to_vec();
        serialized.extend(
            unit.tokens
                .iter()
                .flat_map(|token| token.raw.iter().copied()),
        );
        assert_eq!(serialized, unit.protected_raw);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&serialized)),
            unit.expected_raw_sha256
        );
    }
}

#[test]
fn graph_op21_누락과_중복을_원문_scan으로_보충하지_않는다() {
    let map = map();
    let text = text_with_glyph([0x82, 0xA0]);
    let code_table = code_table_with([0x82, 0xA0]);
    let (mut paths, spawn) = graph();
    paths.paths.pop();
    paths.nodes = 1;
    paths.op21_records = 1;
    paths.text_glyphs = 1;
    paths.text_controls = 1;

    let missing =
        audit_quest_resident_text_units(&map, &text, &code_table, &paths, &spawn).unwrap_err();
    assert!(missing.to_string().contains("graph 분모 불일치"));

    let (mut paths, spawn) = graph();
    paths
        .paths
        .push(path(0x8008_9650, vec![text_node(0x8008_9650)]));
    paths.nodes += 1;
    paths.op21_records += 1;
    paths.text_glyphs += 1;
    paths.text_controls += 1;
    let duplicate =
        audit_quest_resident_text_units(&map, &text, &code_table, &paths, &spawn).unwrap_err();
    assert!(duplicate.to_string().contains("graph record 중복"));
}

#[test]
fn 일본어가_없는_graph_unit은_번역대상으로_채택하지_않는다() {
    let map = map();
    let text = text_with_glyph([0x82, 0x60]);
    let code_table = code_table_with([0x82, 0x60]);
    let (paths, spawn) = graph();

    let error =
        audit_quest_resident_text_units(&map, &text, &code_table, &paths, &spawn).unwrap_err();

    assert!(error.to_string().contains("일본어 glyph가 없는 unit"));
}

#[test]
fn 지원원본_verifier는_다른_source_span을_부분결과없이_거부한다() {
    let map = map();
    let text = text_with_glyph([0x82, 0xA0]);
    let code_table = code_table_with([0x82, 0xA0]);
    let (paths, spawn) = graph();

    let error =
        verify_box_quest_resident_text_units(&map, &text, &code_table, &paths, &spawn).unwrap_err();

    assert!(error.to_string().contains("분모 불일치"));
}

#[test]
fn source_graph_단위결합은_순서와합계를_보존하고_중복과겹침을_거부한다() {
    let map = map();
    let text = text_with_glyph([0x82, 0xA0]);
    let code_table = code_table_with([0x82, 0xA0]);
    let (paths, spawn) = graph();
    let analysis =
        audit_quest_resident_text_units(&map, &text, &code_table, &paths, &spawn).unwrap();

    let combined =
        combine_quest_resident_text_units([&analysis.units[..1], &analysis.units[1..]]).unwrap();
    assert_eq!(combined.units, analysis.units);
    assert_eq!(combined.glyph_occurrences, 3);
    assert_eq!(combined.control_occurrences, 3);

    let duplicate =
        combine_quest_resident_text_units([analysis.units.as_slice(), &analysis.units[..1]])
            .unwrap_err();
    assert!(duplicate.to_string().contains("ID 중복"));

    let mut overlapping = analysis.units[1].clone();
    overlapping.id = "overlapping".to_owned();
    overlapping.record_text_offset = analysis.units[0].record_text_offset + 1;
    overlapping.end_text_offset = analysis.units[0].end_text_offset + 1;
    let overlap = combine_quest_resident_text_units([
        &analysis.units[..1],
        std::slice::from_ref(&overlapping),
    ])
    .unwrap_err();
    assert!(overlap.to_string().contains("보호 span 겹침"));
}
