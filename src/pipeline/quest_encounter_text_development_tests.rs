use super::super::quest_battle_name_compositions::{
    QuestBattleNameComposition, QuestBattleNameDecisionStatus,
};
use super::super::quest_resident_encounter_text_composition::{
    QuestResidentEncounterRunDecision, QuestResidentEncounterRunDecisionStatus,
};
use super::super::quest_resident_text_writer::{
    QuestResidentTextComposition, QuestResidentTextCompositionStatus,
};
use super::*;

fn code_table() -> QuestCodeTable {
    let mut codes = Vec::with_capacity(345);
    for index in 0..345usize {
        codes.push([
            0x90 + u8::try_from(index / 200).unwrap(),
            u8::try_from(index % 200).unwrap(),
        ]);
    }
    QuestCodeTable::parse(&codes.into_iter().flatten().collect::<Vec<_>>()).unwrap()
}

fn battle_names() -> QuestBattleNameCompositionAnalysis {
    QuestBattleNameCompositionAnalysis {
        compositions: vec![QuestBattleNameComposition {
            unit_id: "name".to_owned(),
            index: 0,
            protected_source: "あ".to_owned(),
            protected_slot_sha256: [1; 32],
            slot_capacity_glyphs: 3,
            development_text: "가나다".to_owned(),
            pronunciation_tail: '다',
            subject_particle: '가',
            candidates: vec!["가나다".to_owned()],
            decision_status: QuestBattleNameDecisionStatus::DraftNeedsIndependentReview,
        }],
        used_glyphs: "가나다".to_owned(),
        selected_drafts: 1,
        needs_human_choice: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    }
}

fn encounter() -> QuestResidentEncounterCompositionAnalysis {
    QuestResidentEncounterCompositionAnalysis {
        composition: QuestResidentTextComposition {
            unit_id: "encounter".to_owned(),
            source_sha256: [2; 32],
            status: QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry,
            runs: Vec::new(),
        },
        decisions: vec![QuestResidentEncounterRunDecision {
            run_index: 0,
            protected_source: "い".to_owned(),
            development_text: "나다라".to_owned(),
            leading_pad_cells: 0,
            trailing_pad_cells: 0,
            candidates: vec!["나다라".to_owned()],
            decision_status: QuestResidentEncounterRunDecisionStatus::DraftNeedsHumanChoice,
        }],
        used_glyphs: "나다라".to_owned(),
        human_choice_runs: 1,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    }
}

#[test]
fn 전투이름과조우문구의_합집합을_다음page로_파생한다() {
    let demand =
        derive_quest_encounter_text_page_demand(115, &battle_names(), &encounter(), &code_table())
            .unwrap();

    assert_eq!(demand.page_id, 116);
    assert_eq!(demand.shared_glyphs, "나다");
    assert_eq!(demand.used_glyphs, "가나다라");
    assert_eq!(demand.storage_glyphs, "가나다라");
    assert_eq!(demand.dynamic_slot_capacity, 315);
}

#[test]
fn page_id_overflow와_승인상태_drift를_거부한다() {
    assert!(derive_quest_encounter_text_page_demand(
        u8::MAX,
        &battle_names(),
        &encounter(),
        &code_table(),
    )
    .is_err());

    let mut approved = battle_names();
    approved.translation_approved = true;
    assert!(
        derive_quest_encounter_text_page_demand(1, &approved, &encounter(), &code_table(),)
            .is_err()
    );
}
