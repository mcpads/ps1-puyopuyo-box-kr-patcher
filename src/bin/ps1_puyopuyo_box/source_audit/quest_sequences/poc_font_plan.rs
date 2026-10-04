//! QUEST 승인 PoC 문장의 기존 code pair와 provisional glyph 공급을 검증한다.

use std::collections::BTreeSet;

use ps1_puyopuyo_box::font::quest::{verify_quest_poc_font_plan, QuestCodeTable};
use ps1_puyopuyo_box::script::quest_translation::{
    approved_quest_poc_translation, QUEST_DEFEAT_DIALOGUE_ID,
};

pub(super) fn audit_quest_poc_font_plan(
    town00: &[u8],
    sequences: &[(String, Vec<u8>)],
    code_table: &QuestCodeTable,
) -> Result<(), String> {
    let observed_pairs = sequences
        .iter()
        .flat_map(|(_, raw)| raw.windows(2).map(|pair| [pair[0], pair[1]]))
        .collect::<BTreeSet<_>>();
    let approved = approved_quest_poc_translation(town00, QUEST_DEFEAT_DIALOGUE_ID)
        .map_err(|error| error.to_string())?;
    let required_glyphs = approved
        .text
        .chars()
        .filter(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
        .collect::<BTreeSet<_>>();
    let font_plan = verify_quest_poc_font_plan(code_table, &observed_pairs, &required_glyphs)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST PoC font plan: source-unobserved {}, provisional wide {}, \
         assigned {}, provisional remaining {}",
        font_plan.source_unobserved_codes,
        font_plan.provisional_wide_slots,
        font_plan.assigned_glyphs,
        font_plan.provisional_remaining,
    );
    Ok(())
}
