use super::*;
use crate::coords::{exe_map, ExeMap};
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST map");
    let mut text = vec![0; map.text_len as usize];
    for &(runtime, _, instruction) in QUEST_ESCAPE_OUTCOME_EVIDENCE {
        let offset = map
            .text_offset(runtime)
            .expect("fixture instruction offset");
        text[offset..offset + 4].copy_from_slice(&encode(instruction).to_le_bytes());
    }
    (map, text)
}

#[test]
fn 강제상태와_합계이상_경계는_난수없이_결과를_정한다() {
    assert_eq!(
        classify_quest_escape_outcome(true, 7, 7),
        QuestEscapeOutcomeRule::ForcedFailure,
    );
    assert_eq!(
        classify_quest_escape_outcome(false, 7, 7),
        QuestEscapeOutcomeRule::GuaranteedSuccess,
    );
    assert_eq!(QuestEscapeOutcomeRule::ForcedFailure.outcome_value(0), 0,);
    assert_eq!(
        QuestEscapeOutcomeRule::GuaranteedSuccess.outcome_value(u16::MAX),
        1,
    );
}

#[test]
fn 세_비율구간은_서로_다른_난수규칙을_고른다() {
    let lowest = classify_quest_escape_outcome(false, 4, 9);
    let middle = classify_quest_escape_outcome(false, 6, 10);
    let highest = classify_quest_escape_outcome(false, 7, 10);

    assert_eq!(
        lowest,
        QuestEscapeOutcomeRule::RandomZeroSucceeds { modulus: 8 },
    );
    assert_eq!(
        middle,
        QuestEscapeOutcomeRule::RandomZeroSucceeds { modulus: 2 },
    );
    assert_eq!(
        highest,
        QuestEscapeOutcomeRule::RandomNonzeroSucceeds { modulus: 3 },
    );
    assert_eq!(lowest.outcome_value(0), 1);
    assert_eq!(lowest.outcome_value(1), 0);
    assert_eq!(middle.outcome_value(0), 1);
    assert_eq!(middle.outcome_value(1), 0);
    assert_eq!(highest.outcome_value(0), 0);
    assert_eq!(highest.outcome_value(1), 1);
}

#[test]
fn 지원원본의_opc8_증거가_drift하면_검증을_거부한다() {
    let (map, text) = fixture();
    let summary = verify_box_quest_escape_outcome(map, &text).expect("OPC8 fixture");
    assert_eq!(summary.opcode, QUEST_ESCAPE_OUTCOME_OPCODE);
    assert_eq!(
        summary.outcome_field_offset,
        QUEST_ESCAPE_OUTCOME_FIELD_OFFSET
    );
    assert_eq!(summary.random_moduli, [8, 2, 3]);

    let mut drift = text;
    let offset = map
        .text_offset(0x8007_6D04)
        .expect("forced failure load offset");
    drift[offset] ^= 1;
    assert!(verify_box_quest_escape_outcome(map, &drift).is_err());
}
