//! QUEST resident font page hook의 typed MIPS source를 조립·검산한다.

use crate::disasm::{
    QUEST_FONT_HOOK_CONTINUATION_RUNTIME, QUEST_FONT_STATE_BASE_RUNTIME,
    QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, QUEST_FONT_UPLOAD_TARGET_RUNTIME,
    QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::isa::{verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode};

use super::{
    error, QuestPageFirstDialogueError, QUEST_FONT_STATE_ACTIVE_OFFSET, QUEST_HOOK_BODY_RUNTIME,
    QUEST_PAGE_FIRST_DIALOGUE_BODY_WORDS, QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID,
    QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME, QUEST_PAGE_FIRST_DIALOGUE_SOURCE_ID,
    QUEST_PAGE_HOOK_STATE_RUNTIME, QUEST_PAGE_RESTORE_PAGE_ID, QUEST_PAGE_RESTORE_SOURCE_RUNTIME,
    QUEST_PAGE_SECOND_DIALOGUE_RECORD_RUNTIME, QUEST_PAGE_TRANSITION_BODY_WORDS,
    QUEST_PAGE_TRANSITION_SOURCE_ID, QUEST_PAGE_TWO_DIALOGUES_BODY_WORDS,
    QUEST_PAGE_TWO_DIALOGUES_SOURCE_ID,
};

pub(super) fn load_address(register: Reg, address: u32) -> [K; 2] {
    [
        K::Lui {
            rt: register,
            imm: (address.wrapping_add(0x8000) >> 16) as u16,
        },
        K::Addiu {
            rt: register,
            rs: register,
            imm: address as u16 as i16,
        },
    ]
}

pub(super) fn branch_offset(
    source_index: usize,
    target_index: usize,
) -> Result<i16, QuestPageFirstDialogueError> {
    let source_next = source_index
        .checked_add(1)
        .ok_or_else(|| error("QUEST page hook branch source overflow"))?;
    let delta = isize::try_from(target_index)
        .ok()
        .and_then(|target| {
            isize::try_from(source_next)
                .ok()
                .map(|source| target - source)
        })
        .and_then(|value| i16::try_from(value).ok())
        .ok_or_else(|| error("QUEST page hook branch offset 변환 실패"))?;
    Ok(delta)
}

pub(super) fn assemble_body() -> Result<VerifiedMachineCode, QuestPageFirstDialogueError> {
    let [record_hi, record_lo] = load_address(Reg::T0, QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME);
    let [state_hi, _state_lo] = load_address(Reg::T0, QUEST_PAGE_HOOK_STATE_RUNTIME);
    let [font_state_hi, font_state_lo] = load_address(Reg::T0, QUEST_FONT_STATE_BASE_RUNTIME);
    let [source_hi, source_lo] = load_address(Reg::A1, QUEST_PAGE_CARRIER_START_RUNTIME);

    let mut kinds = vec![
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -32,
        },
        K::Sw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 28,
        },
        K::Sw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 24,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
        record_hi,
        record_lo,
        K::Bne {
            rs: Reg::V0,
            rt: Reg::T0,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        state_hi,
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: QUEST_PAGE_HOOK_STATE_RUNTIME as u16 as i16,
        },
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID),
        },
        K::Beq {
            rs: Reg::T1,
            rt: Reg::T2,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        font_state_hi,
        font_state_lo,
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_FONT_STATE_RECORD_COUNT as i16,
        },
        K::Lbu {
            rt: Reg::T2,
            base: Reg::T0,
            offset: QUEST_FONT_STATE_ACTIVE_OFFSET,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Bne {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: QUEST_FONT_STATE_RECORD_STRIDE as i16,
        },
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: -1,
        },
        K::Bne {
            rs: Reg::T1,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::ZERO,
            imm: 0x0240,
        },
        K::Sw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 0,
        },
        K::Lui {
            rt: Reg::T0,
            imm: 0x0100,
        },
        K::Ori {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0x0040,
        },
        K::Sw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 4,
        },
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::SP,
            imm: 0,
        },
        source_hi,
        source_lo,
        K::Jal {
            target: QUEST_FONT_UPLOAD_TARGET_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Jal {
            target: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
        },
        K::Addu {
            rd: Reg::A0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        state_hi,
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID),
        },
        K::Sb {
            rt: Reg::T1,
            base: Reg::T0,
            offset: QUEST_PAGE_HOOK_STATE_RUNTIME as u16 as i16,
        },
        K::Lw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 24,
        },
        K::Lw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 28,
        },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: 32,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::J {
            target: QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ];
    if kinds.len() != QUEST_PAGE_FIRST_DIALOGUE_BODY_WORDS {
        return Err(error(format!(
            "QUEST 첫 대사 page hook word 수 drift: expected={}, actual={}",
            QUEST_PAGE_FIRST_DIALOGUE_BODY_WORDS,
            kinds.len()
        )));
    }
    let tail = 38;
    for index in [6, 11, 18] {
        let offset = branch_offset(index, tail)?;
        match &mut kinds[index] {
            K::Beq { offset: actual, .. } | K::Bne { offset: actual, .. } => *actual = offset,
            _ => return Err(error("QUEST page hook branch index가 branch가 아님")),
        }
    }
    let loop_offset = branch_offset(21, 16)?;
    match &mut kinds[21] {
        K::Bne { offset, .. } => *offset = loop_offset,
        _ => return Err(error("QUEST page hook loop index가 bne가 아님")),
    }
    verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_HOOK_BODY_RUNTIME,
        QUEST_PAGE_FIRST_DIALOGUE_SOURCE_ID,
        &kinds,
    )
    .map_err(Into::into)
}

fn assemble_transition_body_for_records(
    records: &[u32],
    source_id: &'static str,
    expected_words: usize,
) -> Result<VerifiedMachineCode, QuestPageFirstDialogueError> {
    if records.is_empty() {
        return Err(error("QUEST two-page hook의 Korean record가 없음"));
    }
    let [korean_source_hi, korean_source_lo] =
        load_address(Reg::A1, QUEST_PAGE_CARRIER_START_RUNTIME);
    let [original_source_hi, original_source_lo] =
        load_address(Reg::A1, QUEST_PAGE_RESTORE_SOURCE_RUNTIME);
    let [state_hi, _state_lo] = load_address(Reg::T0, QUEST_PAGE_HOOK_STATE_RUNTIME);
    let [font_state_hi, font_state_lo] = load_address(Reg::T0, QUEST_FONT_STATE_BASE_RUNTIME);

    let mut kinds = vec![
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -32,
        },
        K::Sw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 28,
        },
        K::Sw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 24,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ];
    let mut record_branch_indices = Vec::with_capacity(records.len());
    for record in records {
        let [record_hi, record_lo] = load_address(Reg::T0, *record);
        kinds.extend([record_hi, record_lo]);
        record_branch_indices.push(kinds.len());
        kinds.extend([
            K::Beq {
                rs: Reg::V0,
                rt: Reg::T0,
                offset: 0,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]);
    }
    let korean_label = kinds
        .len()
        .checked_add(5)
        .ok_or_else(|| error("QUEST Korean page label overflow"))?;
    let common_selector = korean_label
        .checked_add(3)
        .ok_or_else(|| error("QUEST page selector label overflow"))?;
    let selector_runtime = QUEST_HOOK_BODY_RUNTIME
        .checked_add(
            u32::try_from(common_selector)
                .map_err(|_| error("QUEST page selector index 변환 실패"))?
                .checked_mul(4)
                .ok_or_else(|| error("QUEST page selector byte offset overflow"))?,
        )
        .ok_or_else(|| error("QUEST page selector runtime overflow"))?;
    kinds.extend([
        original_source_hi,
        original_source_lo,
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_PAGE_RESTORE_PAGE_ID),
        },
        K::J {
            target: selector_runtime,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        korean_source_hi,
        korean_source_lo,
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID),
        },
    ]);
    if kinds.len() != common_selector {
        return Err(error("QUEST two-page 공통 selector 위치 drift"));
    }
    kinds.extend([
        K::Sw {
            rt: Reg::T2,
            base: Reg::SP,
            offset: 8,
        },
        state_hi,
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: QUEST_PAGE_HOOK_STATE_RUNTIME as u16 as i16,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::T1,
            rt: Reg::T2,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        font_state_hi,
        font_state_lo,
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::ZERO,
            imm: QUEST_FONT_STATE_RECORD_COUNT as i16,
        },
        K::Lbu {
            rt: Reg::T2,
            base: Reg::T0,
            offset: QUEST_FONT_STATE_ACTIVE_OFFSET,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Bne {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: QUEST_FONT_STATE_RECORD_STRIDE as i16,
        },
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: -1,
        },
        K::Bne {
            rs: Reg::T1,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::ZERO,
            imm: 0x0240,
        },
        K::Sw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 0,
        },
        K::Lui {
            rt: Reg::T0,
            imm: 0x0100,
        },
        K::Ori {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 0x0040,
        },
        K::Sw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 4,
        },
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::SP,
            imm: 0,
        },
        K::Jal {
            target: QUEST_FONT_UPLOAD_TARGET_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Jal {
            target: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
        },
        K::Addu {
            rd: Reg::A0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        state_hi,
        K::Lw {
            rt: Reg::T1,
            base: Reg::SP,
            offset: 8,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Sb {
            rt: Reg::T1,
            base: Reg::T0,
            offset: QUEST_PAGE_HOOK_STATE_RUNTIME as u16 as i16,
        },
        K::Lw {
            rt: Reg::A0,
            base: Reg::SP,
            offset: 24,
        },
        K::Lw {
            rt: Reg::RA,
            base: Reg::SP,
            offset: 28,
        },
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: 32,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::J {
            target: QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    if kinds.len() != expected_words {
        return Err(error(format!(
            "QUEST two-page hook word 수 drift: expected={}, actual={}",
            expected_words,
            kinds.len()
        )));
    }
    for index in record_branch_indices {
        let offset = branch_offset(index, korean_label)?;
        match &mut kinds[index] {
            K::Beq { offset: actual, .. } => *actual = offset,
            _ => return Err(error("QUEST Korean record branch index가 beq가 아님")),
        }
    }
    let tail = common_selector + 30;
    for (index, target) in [
        (common_selector + 4, tail),
        (common_selector + 11, tail),
        (common_selector + 14, common_selector + 9),
    ] {
        let offset = branch_offset(index, target)?;
        match &mut kinds[index] {
            K::Beq { offset: actual, .. } | K::Bne { offset: actual, .. } => *actual = offset,
            _ => return Err(error("QUEST two-page hook branch index가 branch가 아님")),
        }
    }
    verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_HOOK_BODY_RUNTIME,
        source_id,
        &kinds,
    )
    .map_err(Into::into)
}

pub(super) fn assemble_transition_body() -> Result<VerifiedMachineCode, QuestPageFirstDialogueError>
{
    assemble_transition_body_for_records(
        &[QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME],
        QUEST_PAGE_TRANSITION_SOURCE_ID,
        QUEST_PAGE_TRANSITION_BODY_WORDS,
    )
}

pub(super) fn assemble_two_dialogue_transition_body(
) -> Result<VerifiedMachineCode, QuestPageFirstDialogueError> {
    assemble_transition_body_for_records(
        &[
            QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME,
            QUEST_PAGE_SECOND_DIALOGUE_RECORD_RUNTIME,
        ],
        QUEST_PAGE_TWO_DIALOGUES_SOURCE_ID,
        QUEST_PAGE_TWO_DIALOGUES_BODY_WORDS,
    )
}
