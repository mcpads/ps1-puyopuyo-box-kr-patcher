//! QUEST font page 활성화 hook의 최소 body 예산.
//!
//! 이 모듈은 최종 patch를 만들지 않는다. resident raw page 주소가 아직
//! scene별로 결합되지 않았으므로 source pointer를 0으로 둔 unbound
//! template만 typed ISA로 조립한다. 공유 decoder와 loader arena를 쓰지
//! 않는 최소 upload body의 예산을 고정한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::disasm::{
    verify_box_quest_font_consumer, verify_box_quest_font_hook_site, QuestHookCandidateAnalysis,
    QUEST_FONT_HOOK_CONTINUATION_RUNTIME, QUEST_FONT_SOURCE_BYTES, QUEST_FONT_STATE_BASE_RUNTIME,
    QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, QUEST_FONT_UPLOAD_TARGET_RUNTIME,
    QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET,
};
use crate::isa::{
    verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode, VerifyError,
};

use super::quest_hook_poc::{
    verify_quest_hook_noop_sources, QUEST_HOOK_BODY_RESERVED_END_RUNTIME, QUEST_HOOK_BODY_RUNTIME,
    QUEST_HOOK_RESERVED_BYTES,
};

pub const QUEST_PAGE_HOOK_SCHEMA_VERSION: &str = "quest-page-hook-minimum-v2";
pub const QUEST_PAGE_HOOK_SOURCE_ID: &str = "pipeline::quest_page_hook_plan::unbound_minimum_body";
pub const QUEST_PAGE_HOOK_UNBOUND_SOURCE_RUNTIME: u32 = 0;
pub const QUEST_PAGE_HOOK_STATE_RUNTIME: u32 = QUEST_HOOK_BODY_RESERVED_END_RUNTIME - 4;
pub const QUEST_PAGE_HOOK_PAGE_ID: u8 = 1;
pub const QUEST_PAGE_HOOK_PAGE_BYTES: usize = QUEST_FONT_SOURCE_BYTES;
pub const QUEST_PAGE_HOOK_BODY_WORDS: usize = 40;
pub const QUEST_PAGE_HOOK_BODY_BYTES: usize = QUEST_PAGE_HOOK_BODY_WORDS * 4;

const QUEST_FONT_STATE_ACTIVE_OFFSET: i16 = QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET + 1;

/// source가 묶이지 않은 최소 page 활성화 body의 크기 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageHookMinimumPlan {
    pub schema_version: &'static str,
    pub body: VerifiedMachineCode,
    pub reserved_bytes: usize,
    pub body_bytes: usize,
    pub body_slack_bytes: usize,
    pub state_runtime: u32,
    pub source_runtime: u32,
    pub page_bytes: usize,
    pub checks_all_font_states: bool,
    pub waits_for_draw_sync: bool,
    pub commits_state_after_sync: bool,
    pub source_residency_proven: bool,
    pub build_eligible: bool,
}

#[derive(Debug)]
pub enum QuestPageHookPlanError {
    Message(String),
    MachineCode(VerifyError),
}

impl fmt::Display for QuestPageHookPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::MachineCode(source) => {
                write!(f, "QUEST page hook 최소 body ISA 검산 실패: {source}")
            }
        }
    }
}

impl Error for QuestPageHookPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MachineCode(source) => Some(source),
            Self::Message(_) => None,
        }
    }
}

impl From<VerifyError> for QuestPageHookPlanError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

fn error(message: impl Into<String>) -> QuestPageHookPlanError {
    QuestPageHookPlanError::Message(message.into())
}

fn load_address(register: Reg, address: u32) -> [K; 2] {
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

fn assemble_minimum_body() -> Result<VerifiedMachineCode, QuestPageHookPlanError> {
    let [state_hi, _state_lo] = load_address(Reg::T0, QUEST_PAGE_HOOK_STATE_RUNTIME);
    let [font_state_hi, font_state_lo] = load_address(Reg::T0, QUEST_FONT_STATE_BASE_RUNTIME);
    let [source_hi, source_lo] = load_address(Reg::A1, QUEST_PAGE_HOOK_UNBOUND_SOURCE_RUNTIME);

    let kinds = [
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
        state_hi,
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: QUEST_PAGE_HOOK_STATE_RUNTIME as u16 as i16,
        },
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_PAGE_HOOK_PAGE_ID),
        },
        K::Beq {
            rs: Reg::T1,
            rt: Reg::T2,
            offset: 26,
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
            offset: 19,
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
            offset: -6,
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
            imm: i16::from(QUEST_PAGE_HOOK_PAGE_ID),
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
    if kinds.len() != QUEST_PAGE_HOOK_BODY_WORDS {
        return Err(error(format!(
            "QUEST page hook 최소 body word 수 drift: expected={}, actual={}",
            QUEST_PAGE_HOOK_BODY_WORDS,
            kinds.len()
        )));
    }
    verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_HOOK_BODY_RUNTIME,
        QUEST_PAGE_HOOK_SOURCE_ID,
        &kinds,
    )
    .map_err(Into::into)
}

/// 지원 원본 ABI와 후보 공간을 다시 확인하고 unbound 최소 body 예산을 만든다.
pub fn verify_quest_page_hook_minimum_plan(
    map: &ExeMap,
    text: &[u8],
    candidates: &QuestHookCandidateAnalysis,
) -> Result<QuestPageHookMinimumPlan, QuestPageHookPlanError> {
    verify_box_quest_font_hook_site(map, text).map_err(|source| error(source.to_string()))?;
    verify_box_quest_font_consumer(map, text).map_err(|source| error(source.to_string()))?;
    let noop = verify_quest_hook_noop_sources(map, text, candidates)
        .map_err(|source| error(source.to_string()))?;
    if noop.reserved_bytes != QUEST_HOOK_RESERVED_BYTES {
        return Err(error("QUEST page hook reserved 크기가 no-op gate와 불일치"));
    }
    let body = assemble_minimum_body()?;
    if body.bytes().len() != QUEST_PAGE_HOOK_BODY_BYTES {
        return Err(error("QUEST page hook 최소 body byte 수 불일치"));
    }
    let body_end = QUEST_HOOK_BODY_RUNTIME
        .checked_add(QUEST_PAGE_HOOK_BODY_BYTES as u32)
        .ok_or_else(|| error("QUEST page hook body 끝 주소 overflow"))?;
    if body_end > QUEST_PAGE_HOOK_STATE_RUNTIME {
        return Err(error("QUEST page hook body가 hook-owned state와 겹침"));
    }

    Ok(QuestPageHookMinimumPlan {
        schema_version: QUEST_PAGE_HOOK_SCHEMA_VERSION,
        body,
        reserved_bytes: QUEST_HOOK_RESERVED_BYTES,
        body_bytes: QUEST_PAGE_HOOK_BODY_BYTES,
        body_slack_bytes: QUEST_PAGE_HOOK_STATE_RUNTIME as usize - body_end as usize,
        state_runtime: QUEST_PAGE_HOOK_STATE_RUNTIME,
        source_runtime: QUEST_PAGE_HOOK_UNBOUND_SOURCE_RUNTIME,
        page_bytes: QUEST_PAGE_HOOK_PAGE_BYTES,
        checks_all_font_states: true,
        waits_for_draw_sync: true,
        commits_state_after_sync: true,
        source_residency_proven: false,
        build_eligible: false,
    })
}

#[cfg(test)]
#[path = "quest_page_hook_plan_tests.rs"]
mod tests;
