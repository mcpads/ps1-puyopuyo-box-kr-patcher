use super::*;
use crate::pipeline::QuestTranslationSelectionRoute;
use crate::script::{QuestTranslationIndependentReviewStatus, QuestTranslationSelectionEntry};

fn demand(route_index: usize, stream_id: &str, glyphs: &str) -> StreamGlyphDemand {
    let used_glyphs = glyphs.chars().collect::<BTreeSet<_>>();
    StreamGlyphDemand {
        route_index,
        stream_id: stream_id.to_owned(),
        dynamic_glyphs: used_glyphs.clone(),
        used_glyphs,
    }
}

#[test]
fn 단일stream최대수요안에서_겹치는working_set을_같은page에묶는다() {
    let demands = [
        demand(0, "stream-0", "가나다"),
        demand(1, "stream-1", "가나"),
        demand(2, "stream-2", "라마"),
        demand(3, "stream-3", "라바"),
    ];
    let pages = pack_stream_demands(&demands).unwrap();

    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].route_indices, vec![0, 1]);
    assert_eq!(pages[1].route_indices, vec![2, 3]);
    assert!(pages.iter().all(|page| page.dynamic_glyphs.len() <= 3));
}

#[test]
fn 새page가필요할때도_각route를정확히한번배정한다() {
    let demands = [
        demand(0, "stream-0", "가"),
        demand(1, "stream-1", "나"),
        demand(2, "stream-2", "다"),
    ];
    let pages = pack_stream_demands(&demands).unwrap();
    let assigned = pages
        .iter()
        .flat_map(|page| page.route_indices.iter().copied())
        .collect::<BTreeSet<_>>();

    assert_eq!(pages.len(), 3);
    assert_eq!(assigned, BTreeSet::from([0, 1, 2]));
}

// 차 전달 직전 두 OP21의 glyph는 다음 OP21이 창을 닫을 때까지 함께 남는다.
// 각각 다른 bank에 배정하면 다음 page를 올리기 전에 두 bank가 모두 pinned다.
#[test]
fn 함께남는_차전달대사는_하나의page를공유한다() {
    let demands = [
        demand(0, "다른-대사-가", "가라"),
        demand(1, "다른-대사-나", "나다"),
        demand(2, "QUEST-SEQ-TEXT-STREAM:HASI.SEQ:0x0AA4", "가"),
        demand(3, "QUEST-SEQ-TEXT-STREAM:HASI.SEQ:0x0ADC", "나"),
    ];
    let pages = pack_stream_demands(&demands).unwrap();
    let page_for = |route| {
        pages
            .iter()
            .position(|page| page.route_indices.contains(&route))
    };
    assert_eq!(page_for(2), page_for(3));
    let assigned = pages
        .iter()
        .flat_map(|page| &page.route_indices)
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(assigned, BTreeSet::from([0, 1, 2, 3]));
}

// 상점 glyph와 제안문이 두 bank를 잡은 상태에서도 선택 후 첫 FF05까지
// 진행해야 한다. 응답 page를 올리려고 먼저 기다리면 창 해제에 도달하지 못한다.
#[test]
fn 모모모_선택_후속은_두_bank가_점유되어도_현재_page에서_진행한다() {
    use crate::pipeline::quest_glyph_atlas_bank_policy::{
        select_quest_glyph_atlas_bank, QuestGlyphAtlasBankSelection, QuestGlyphAtlasBankState,
    };

    for offsets in [["39C8", "3A90", "3B10"], ["3CE4", "3D6C", "3DB0"]] {
        let mut demands = vec![
            demand(0, "다른-가", "가라마"),
            demand(1, "다른-나", "나바사"),
            demand(2, "다른-다", "다아자"),
        ];
        for (index, (offset, glyph)) in offsets.into_iter().zip(["가", "나", "다"]).enumerate() {
            demands.push(demand(
                index + 3,
                &format!("QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x{offset}"),
                glyph,
            ));
        }
        let pages = pack_stream_demands(&demands).unwrap();
        let page_for = |route| {
            u8::try_from(
                pages
                    .iter()
                    .position(|page| page.route_indices.contains(&route))
                    .unwrap()
                    + 1,
            )
            .unwrap()
        };
        let state = QuestGlyphAtlasBankState {
            logical_pages: [127, page_for(3)],
            pinned: [true, true],
        };
        for response in [4, 5] {
            assert!(matches!(
                select_quest_glyph_atlas_bank(page_for(response), state).unwrap(),
                QuestGlyphAtlasBankSelection::Resident { .. }
            ));
        }
    }
}

#[test]
fn 동시표시수요를_한page에담을수없으면_분할하지않고실패한다() {
    let demands = [
        demand(0, "QUEST-SEQ-TEXT-STREAM:HASI.SEQ:0x0AA4", "가나"),
        demand(1, "QUEST-SEQ-TEXT-STREAM:HASI.SEQ:0x0ADC", "다라"),
    ];
    assert!(pack_stream_demands(&demands).is_err());
}

fn manual_only_selection() -> QuestTranslationSelectionAnalysis {
    QuestTranslationSelectionAnalysis {
        entries: vec![
            QuestTranslationSelectionEntry {
                stream_id: "manual".to_owned(),
                batch_id: "batch-manual".to_owned(),
                review_status: QuestTranslationIndependentReviewStatus::ReviewAgrees,
                disposition: QuestTranslationSelectionDisposition::SelectManualComposition,
                protected_source: "あい".to_owned(),
                draft_korean: "가나".to_owned(),
                reviewed_korean: "가나".to_owned(),
                selected_korean: Some("가나".to_owned()),
                selected_hangul: "가나".to_owned(),
                decision_dependencies: Vec::new(),
                translation_approved: false,
                product_eligible: false,
            },
            QuestTranslationSelectionEntry {
                stream_id: "retained".to_owned(),
                batch_id: "batch-retained".to_owned(),
                review_status: QuestTranslationIndependentReviewStatus::NeedsHumanDecision,
                disposition: QuestTranslationSelectionDisposition::RetainSource,
                protected_source: "うえ".to_owned(),
                draft_korean: "다라".to_owned(),
                reviewed_korean: "다라".to_owned(),
                selected_korean: None,
                selected_hangul: String::new(),
                decision_dependencies: vec!["사람 결정".to_owned()],
                translation_approved: false,
                product_eligible: false,
            },
        ],
        selected_entries: 1,
        retained_source_entries: 1,
        selected_entries_with_review_agreement: 1,
        selected_entries_with_review_proposal: 0,
        selected_manual_compositions: 1,
        selected_batches: 1,
        selected_hangul_repertoire: "가나".to_owned(),
        draft_hangul_repertoire: "가나다라".to_owned(),
        new_cache_glyphs: String::new(),
        draft_only_glyphs: "다라".to_owned(),
        translation_approved_entries: 0,
        product_eligible_entries: 0,
    }
}

fn manual_only_routing() -> QuestTranslationSelectionRoutingAnalysis {
    let route = |stream_id: &str,
                 batch_id: &str,
                 review_status: QuestTranslationIndependentReviewStatus,
                 placement: QuestTranslationSelectionPlacement,
                 protected_record: [u8; 4],
                 routed_record: [u8; 4],
                 selected_glyphs: Option<usize>| QuestTranslationSelectionRoute {
        sequence: "TEST.SEQ".to_owned(),
        stream_id: stream_id.to_owned(),
        batch_id: batch_id.to_owned(),
        review_status,
        placement,
        page_id: routed_record[1],
        record_offset: 0,
        end_offset: 16,
        protected_record,
        routed_record,
        source_stream_bytes: 16,
        selected_glyphs,
        projected_stream_bytes: selected_glyphs.map(|_| 20),
        relocated_stream_offset: selected_glyphs.map(|_| 16),
        return_runtime: selected_glyphs.map(|_| 0x8019_0010),
        translation_approved: false,
        layout_approved: false,
        product_eligible: false,
    };
    QuestTranslationSelectionRoutingAnalysis {
        routes: vec![
            route(
                "manual",
                "batch-manual",
                QuestTranslationIndependentReviewStatus::ReviewAgrees,
                QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate,
                [0x21, 1, 0, 0],
                [0x21, 1, 4, 0],
                Some(2),
            ),
            route(
                "retained",
                "batch-retained",
                QuestTranslationIndependentReviewStatus::NeedsHumanDecision,
                QuestTranslationSelectionPlacement::RetainSource,
                [0x21, 2, 0, 0],
                [0x21, 2, 0, 0],
                None,
            ),
        ],
        sequences: Vec::new(),
        selected: 1,
        retained_source: 1,
        selected_pages: 1,
        inline: 0,
        relocated: 1,
        known_relocated_bytes: 20,
        minimum_active_window_remaining: 1,
        runtime_base_observed_sequences: 0,
        static_loader_runtime_base_sequences: 0,
        runtime_base_bindings_proven: false,
        translation_approved_routes: 0,
        layout_approved_routes: 0,
        product_eligible_routes: 0,
    }
}

#[test]
fn page_packing은_수동composition만배정하고_원문route는건드리지않는다() {
    let selection = manual_only_selection();
    let demands = collect_stream_demands(&selection, &manual_only_routing()).unwrap();

    assert_eq!(demands.len(), 1);
    assert_eq!(demands[0].stream_id, "manual");
    assert_eq!(demands[0].route_index, 0);
}

#[test]
fn page_packing은_수동조판없는검토문선택을거부한다() {
    let mut selection = manual_only_selection();
    selection.entries[0].disposition = QuestTranslationSelectionDisposition::SelectReviewedKorean;

    assert!(collect_stream_demands(&selection, &manual_only_routing()).is_err());
}

// 메일 목록은 모든 제목을 만든 뒤 선택을 받으므로 도중에 bank를 비울 수 없다.
#[test]
fn 메일목록의_누적제목은_선택창완성전_같은page를사용한다() {
    let demands = [
        demand(0, "다른-긴-대사-가", "가아자차카타파"),
        demand(5, "다른-긴-대사-나", "다나마바사라하"),
        demand(1, "QUEST-SEQ-TEXT-STREAM:TOWN20.SEQ:0x31B4", "가나"),
        demand(2, "QUEST-SEQ-TEXT-STREAM:TOWN20.SEQ:0x3208", "다라"),
        demand(3, "QUEST-SEQ-TEXT-STREAM:TOWN20.SEQ:0x3234", "마바"),
        demand(4, "QUEST-SEQ-TEXT-STREAM:TOWN20.SEQ:0x3264", "사"),
    ];
    let pages = pack_stream_demands(&demands).unwrap();
    let page_for = |route| pages.iter().position(|p| p.route_indices.contains(&route));
    for route in 2..=4 {
        assert_eq!(page_for(1), page_for(route));
    }
    let assigned = pages
        .iter()
        .flat_map(|p| &p.route_indices)
        .copied()
        .collect::<Vec<_>>();
    for route in 0..=5 {
        assert_eq!(assigned.iter().filter(|&&value| value == route).count(), 1);
    }
}

#[test]
fn 플럼버의_일곱메일제목은_하나의page에누적된다() {
    let demands = [
        demand(0, "다른-대사-가", "가아자차카타파"),
        demand(6, "다른-대사-나", "다나마바사라하"),
        demand(1, "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4C20", "가나"),
        demand(2, "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4C70", "다라"),
        demand(3, "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4CA8", "마"),
        demand(4, "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4CD4", "바"),
        demand(5, "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4D00", "사"),
    ];
    let pages = pack_stream_demands(&demands).unwrap();
    let title_page = pages
        .iter()
        .find(|page| page.route_indices.contains(&1))
        .unwrap();
    for route in 1..=5 {
        assert!(title_page.route_indices.contains(&route));
    }
    assert_eq!(
        pages.iter().map(|p| p.route_indices.len()).sum::<usize>(),
        demands.len()
    );
}
