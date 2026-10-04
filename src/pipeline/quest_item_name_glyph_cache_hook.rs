//! ITEM_DAT 이름 callback과 direct glyph call을 공용 glyph cache body에 붙이는 typed 경로.
//!
//! OP21 본체는 page 공급과 두-bank 생명주기를 소유한다. item-name mode는
//! 장면별 소유권을 닫지 못한 secondary bank를 선택하지 않고 primary pin에서
//! 재시도한다. 이 모듈은 이름 전용 설치 site·진입·renderer caller filter만
//! 소유한다. 공용 frame과 mode별 복귀는 `quest_item_glyph_cache_dispatch`가
//! 맡아 ITEM_DAT 이름·설명 경계가 서로 섞이지 않게 한다.

use super::quest_glyph_atlas_bank_policy::QUEST_GLYPH_ATLAS_LOGICAL_PAGE_MASK;
use super::quest_item_glyph_cache_dispatch::{
    push_address, push_item_entry_frame, QuestItemTextModeEntry, QUEST_ITEM_NAME_MODE_CALLBACK,
    QUEST_ITEM_NAME_MODE_DIRECT_GLYPH, QUEST_ITEM_NAME_MODE_WHOLE_STRING,
    QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME, QUEST_ITEM_TEXT_MODE_RESIDENCY,
};
use crate::disasm::{
    QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES, QUEST_ITEM_NAME_GLYPH_WRITER_RETURN_RUNTIME,
    QUEST_ITEM_NAME_RESIDENCY_SITE_RUNTIME, QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES,
    QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_RETURN_RUNTIME, QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
    QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME, QUEST_SEQ_TEXT_CONSUMER_RUNTIME,
    QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES,
};
use crate::isa::{
    verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode, VerifyError,
};
use crate::script::QUEST_SEQ_TEXT_MODE_OPCODE;

pub(super) const QUEST_ITEM_NAME_RESIDENCY_SITE_SOURCE_ID: &str =
    "pipeline::quest_item_name_glyph_cache_hook::residency_site";
pub(super) const QUEST_ITEM_NAME_DIRECT_GLYPH_SITE_SOURCE_ID: &str =
    "pipeline::quest_item_name_glyph_cache_hook::direct_glyph_site";
pub(super) const QUEST_ITEM_NAME_WHOLE_STRING_SITE_SOURCE_ID: &str =
    "pipeline::quest_item_name_glyph_cache_hook::whole_string_site";
pub(super) const QUEST_ITEM_TEXT_MODE_RESIDENCY_SITE_SOURCE_ID: &str =
    "pipeline::quest_item_name_glyph_cache_hook::text_mode_residency_site";

pub(super) struct QuestItemNameRendererFilter {
    pub(super) selected_branches: Vec<usize>,
    pub(super) unselected_branches: Vec<usize>,
    pub(super) selected: usize,
}

pub(super) struct QuestItemNameEntry {
    pub(super) entry: usize,
    pub(super) page_request_jump: usize,
}

pub(super) struct QuestItemNameEntries {
    pub(super) callback: Option<QuestItemNameEntry>,
    pub(super) direct_glyph: Option<QuestItemNameEntry>,
    pub(super) whole_string: Option<QuestItemNameEntry>,
    pub(super) text_mode: Option<QuestItemTextModeEntry>,
}

pub(super) fn assemble_item_name_residency_site(
    item_name_entry_runtime: u32,
) -> Result<VerifiedMachineCode, VerifyError> {
    verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_ITEM_NAME_RESIDENCY_SITE_RUNTIME,
        QUEST_ITEM_NAME_RESIDENCY_SITE_SOURCE_ID,
        &[
            K::Jal {
                target: item_name_entry_runtime,
            },
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ],
    )
}

pub(super) fn assemble_item_name_direct_glyph_sites(
    item_name_direct_entry_runtime: u32,
) -> Result<Vec<VerifiedMachineCode>, VerifyError> {
    QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES
        .into_iter()
        .map(|runtime| {
            verify_typed_source(
                IsaProfile::DECLARED,
                runtime,
                QUEST_ITEM_NAME_DIRECT_GLYPH_SITE_SOURCE_ID,
                &[
                    K::Jal {
                        target: item_name_direct_entry_runtime,
                    },
                    K::Addiu {
                        rt: Reg::S0,
                        rs: Reg::S0,
                        imm: 2,
                    },
                ],
            )
        })
        .collect()
}

pub(super) fn assemble_item_name_whole_string_sites(
    item_name_whole_string_entry_runtime: u32,
) -> Result<Vec<VerifiedMachineCode>, VerifyError> {
    QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES
        .into_iter()
        .map(|runtime| {
            verify_typed_source(
                IsaProfile::DECLARED,
                runtime,
                QUEST_ITEM_NAME_WHOLE_STRING_SITE_SOURCE_ID,
                &[
                    K::Jal {
                        target: item_name_whole_string_entry_runtime,
                    },
                    K::Addu {
                        rd: Reg::A0,
                        rs: Reg::ZERO,
                        rt: Reg::ZERO,
                    },
                ],
            )
        })
        .collect()
}

pub(super) fn assemble_item_text_mode_residency_site(
    item_text_mode_entry_runtime: u32,
) -> Result<VerifiedMachineCode, VerifyError> {
    verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME,
        QUEST_ITEM_TEXT_MODE_RESIDENCY_SITE_SOURCE_ID,
        &[
            K::Jal {
                target: item_text_mode_entry_runtime,
            },
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ],
    )
}

/// OP21 return 주소를 이미 `T1`에 적재한 뒤 caller filter를 덧붙인다.
pub(super) fn append_item_name_renderer_filter(
    kinds: &mut Vec<K>,
    enabled: bool,
) -> QuestItemNameRendererFilter {
    let mut selected_branches = Vec::new();
    let mut unselected_branches = Vec::new();
    if enabled {
        let branch = kinds.len();
        kinds.extend([
            K::Beq {
                rs: Reg::RA,
                rt: Reg::T1,
                offset: 0,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]);
        selected_branches.push(branch);
        push_address(kinds, Reg::T1, QUEST_ITEM_NAME_GLYPH_WRITER_RETURN_RUNTIME);
        let callback_branch = kinds.len();
        kinds.extend([
            K::Beq {
                rs: Reg::RA,
                rt: Reg::T1,
                offset: 0,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]);
        selected_branches.push(callback_branch);
        let returns = QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES
            .into_iter()
            .chain(QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES)
            .chain(
                crate::disasm::QUEST_MENU_GLYPH_CALLS
                    .iter()
                    .filter(|c| !c.whole_string)
                    .map(|c| c.runtime),
            )
            .map(|runtime| runtime + 8)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if let Some(&first) = returns.first() {
            push_address(kinds, Reg::T1, first);
        }
        for (index, &return_runtime) in returns.iter().enumerate() {
            let next = returns.get(index + 1).copied().unwrap_or(return_runtime);
            let delta = i64::from(next) - i64::from(return_runtime);
            let branch = kinds.len();
            kinds.push(K::Beq {
                rs: Reg::RA,
                rt: Reg::T1,
                offset: 0,
            });
            if let Ok(imm) = i16::try_from(delta) {
                // 비교 뒤 delay slot에서 다음 정확한 caller 주소를 준비한다.
                kinds.push(K::Addiu {
                    rt: Reg::T1,
                    rs: Reg::T1,
                    imm,
                });
            } else {
                kinds.push(K::Sll {
                    rd: Reg::ZERO,
                    rt: Reg::ZERO,
                    sa: 0,
                });
                push_address(kinds, Reg::T1, next);
            }
            selected_branches.push(branch);
        }
        push_address(
            kinds,
            Reg::T1,
            QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_RETURN_RUNTIME,
        );
        let caller_mismatch = kinds.len();
        kinds.extend([
            K::Bne {
                rs: Reg::RA,
                rt: Reg::T1,
                offset: 0,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]);
        unselected_branches.push(caller_mismatch);
        push_address(kinds, Reg::T2, QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME);
        kinds.extend([
            K::Lbu {
                rt: Reg::T2,
                base: Reg::T2,
                offset: 0,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]);
        let inactive = kinds.len();
        kinds.extend([
            K::Beq {
                rs: Reg::T2,
                rt: Reg::ZERO,
                offset: 0,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]);
        unselected_branches.push(inactive);
    } else {
        let unselected = kinds.len();
        kinds.extend([
            K::Bne {
                rs: Reg::RA,
                rt: Reg::T1,
                offset: 0,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]);
        unselected_branches.push(unselected);
    }
    QuestItemNameRendererFilter {
        selected_branches,
        unselected_branches,
        selected: kinds.len(),
    }
}

// A1은 원본 renderer에 전달할 16-bit code다. package의 실제 assignment로
// 증명한 공통 prefix만 유한 탐색한다. 찾으면 기본 page 강제를 건너뛴다.
fn append_shared_direct_search(kinds: &mut Vec<K>, shared_slots: u16) -> usize {
    let nop = K::Sll {
        rd: Reg::ZERO,
        rt: Reg::ZERO,
        sa: 0,
    };
    kinds.extend([
        K::Lw {
            rt: Reg::T2,
            base: Reg::SP,
            offset: 20,
        },
        nop,
    ]);
    push_address(
        kinds,
        Reg::T1,
        crate::font::QUEST_CODE_TABLE_RUNTIME
            + (super::quest_glyph_cache_plan::QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX * 2)
                as u32,
    );
    kinds.push(K::Addiu {
        rt: Reg::T3,
        rs: Reg::ZERO,
        imm: i16::try_from(shared_slots).expect("공통 prefix는 원본 code table 범위 안"),
    });
    let scan = kinds.len();
    kinds.extend([
        K::Lhu {
            rt: Reg::T0,
            base: Reg::T1,
            offset: 0,
        },
        nop,
    ]);
    let matched = kinds.len();
    kinds.extend([
        K::Beq {
            rs: Reg::T0,
            rt: Reg::T2,
            offset: 0,
        },
        nop,
        K::Addiu {
            rt: Reg::T3,
            rs: Reg::T3,
            imm: -1,
        },
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: 2,
        },
        K::Bne {
            rs: Reg::T3,
            rt: Reg::ZERO,
            offset: -7,
        },
        nop,
    ]);
    debug_assert_eq!(kinds.len() - scan, 8);
    matched
}

fn append_item_name_entry_for_mode(
    kinds: &mut Vec<K>,
    default_page_id: u8,
    first_item_text_page_id: u8,
    last_item_text_page_id: u8,
    mode: i16,
    shared_slots: u16,
) -> QuestItemNameEntry {
    let entry = kinds.len();
    push_item_entry_frame(kinds, mode);
    kinds.push(K::Addiu {
        rt: Reg::S1,
        rs: Reg::ZERO,
        imm: i16::from(default_page_id),
    });
    push_address(
        kinds,
        Reg::T0,
        super::quest_glyph_cache_hook_plan::QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME,
    );
    kinds.extend([
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Andi {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: u16::from(QUEST_GLYPH_ATLAS_LOGICAL_PAGE_MASK),
        },
        K::Sltiu {
            rt: Reg::T0,
            rs: Reg::T1,
            imm: i16::from(first_item_text_page_id),
        },
        K::Sltiu {
            rt: Reg::T2,
            rs: Reg::T1,
            imm: i16::from(last_item_text_page_id) + 1,
        },
        K::Xor {
            rd: Reg::T0,
            rs: Reg::T0,
            rt: Reg::T2,
        },
        K::Subu {
            rd: Reg::T0,
            rs: Reg::ZERO,
            rt: Reg::T0,
        },
        K::Xor {
            rd: Reg::T2,
            rs: Reg::S1,
            rt: Reg::T1,
        },
        K::And {
            rd: Reg::T2,
            rs: Reg::T2,
            rt: Reg::T0,
        },
        K::Xor {
            rd: Reg::S1,
            rs: Reg::S1,
            rt: Reg::T2,
        },
    ]);
    // 이름과 모든 item page에서 같은 catalog cell인 direct 글자는 현재 page를
    // 재사용한다. 카드·비행처럼 첫 page에만 있는 글자는 기본 page로 보낸다.
    if mode != QUEST_ITEM_NAME_MODE_CALLBACK {
        let shared_match = if mode == QUEST_ITEM_NAME_MODE_DIRECT_GLYPH && shared_slots > 0 {
            Some(append_shared_direct_search(kinds, shared_slots))
        } else {
            None
        };
        let callers: &[u32] = if mode == QUEST_ITEM_NAME_MODE_DIRECT_GLYPH {
            &QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES
        } else {
            &QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES
        };
        kinds.push(K::Lw {
            rt: Reg::T0,
            base: Reg::SP,
            offset: 60,
        });
        for (index, caller) in callers.iter().enumerate() {
            push_address(kinds, Reg::T1, caller + 8);
            kinds.extend([
                K::Beq {
                    rs: Reg::T0,
                    rt: Reg::T1,
                    offset: i16::try_from((callers.len() - index - 1) * 4 + 2)
                        .expect("고정 이름 caller 수는 branch 범위 안"),
                },
                K::Sll {
                    rd: Reg::ZERO,
                    rt: Reg::ZERO,
                    sa: 0,
                },
            ]);
        }
        kinds.push(K::Addiu {
            rt: Reg::S1,
            rs: Reg::ZERO,
            imm: i16::from(default_page_id),
        });
        if let Some(index) = shared_match {
            let distance = i16::try_from(kinds.len() - index - 1)
                .expect("공통 item code branch는 고정 caller 범위 안");
            if let K::Beq { offset, .. } = &mut kinds[index] {
                *offset = distance;
            }
        }
    }
    kinds.extend([
        K::J { target: 0 },
        K::Addu {
            rd: Reg::S2,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ]);
    QuestItemNameEntry {
        entry,
        page_request_jump: kinds.len() - 2,
    }
}

fn append_item_text_mode_entry(
    kinds: &mut Vec<K>,
    first_item_text_page_id: u8,
    last_item_text_page_id: u8,
) -> QuestItemTextModeEntry {
    let entry = kinds.len();
    kinds.extend([
        K::Lw {
            rt: Reg::T0,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: -4,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_SEQ_TEXT_MODE_OPCODE),
        },
        K::Bne {
            rs: Reg::T1,
            rt: Reg::T2,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: -2,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
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
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: -1,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
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
        K::Lbu {
            rt: Reg::T1,
            base: Reg::T0,
            offset: -3,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Sltiu {
            rt: Reg::T2,
            rs: Reg::T1,
            imm: i16::from(first_item_text_page_id),
        },
        K::Bne {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Sltiu {
            rt: Reg::T2,
            rs: Reg::T1,
            imm: i16::from(last_item_text_page_id) + 1,
        },
        K::Beq {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let bypass_branches = vec![entry + 5, entry + 9, entry + 13, entry + 18, entry + 21];
    push_item_entry_frame(kinds, QUEST_ITEM_TEXT_MODE_RESIDENCY);
    kinds.extend([
        K::Addu {
            rd: Reg::S1,
            rs: Reg::T1,
            rt: Reg::ZERO,
        },
        K::J { target: 0 },
        K::Addu {
            rd: Reg::S2,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ]);
    let page_request_jump = kinds.len() - 2;
    let bypass = kinds.len();
    kinds.extend([
        K::J {
            target: QUEST_SEQ_TEXT_CONSUMER_RUNTIME,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    QuestItemTextModeEntry {
        entry,
        page_request_jump,
        bypass_branches,
        bypass,
    }
}

pub(super) fn append_item_name_entries(
    kinds: &mut Vec<K>,
    default_page_id: Option<u8>,
    item_text_page_range: Option<(u8, u8)>,
    shared_slots: u16,
) -> QuestItemNameEntries {
    let page_mode = default_page_id.zip(item_text_page_range);
    let callback = page_mode.map(|(page_id, (first, last))| {
        append_item_name_entry_for_mode(
            kinds,
            page_id,
            first,
            last,
            QUEST_ITEM_NAME_MODE_CALLBACK,
            shared_slots,
        )
    });
    let direct_glyph = page_mode.map(|(page_id, (first, last))| {
        append_item_name_entry_for_mode(
            kinds,
            page_id,
            first,
            last,
            QUEST_ITEM_NAME_MODE_DIRECT_GLYPH,
            shared_slots,
        )
    });
    let whole_string = page_mode.map(|(page_id, (first, last))| {
        append_item_name_entry_for_mode(
            kinds,
            page_id,
            first,
            last,
            QUEST_ITEM_NAME_MODE_WHOLE_STRING,
            shared_slots,
        )
    });
    let text_mode =
        item_text_page_range.map(|(first, last)| append_item_text_mode_entry(kinds, first, last));
    QuestItemNameEntries {
        callback,
        direct_glyph,
        whole_string,
        text_mode,
    }
}

#[cfg(test)]
#[path = "quest_item_name_glyph_cache_hook_tests.rs"]
mod tests;
