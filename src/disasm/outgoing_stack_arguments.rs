//! 표준 stack frame의 outgoing argument slot과 간접 호출을 연결한다.
//!
//! 선형 명령열에서 `jalr $ra, reg`를 찾고 활성 frame 안의 지정된 `sp`
//! offset write를 연결한다. 현재 호출을 위해 설정한 값과 앞선 호출이 이미
//! 소비한 값을 구분할 뿐, 함수 경계·callee·인자 의미를 확정하지 않는다.

use crate::isa::{Instruction, InstructionKind as K, Reg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutgoingStackArgumentOrigin {
    CurrentCallSetup,
    CurrentCallDelaySlot,
    PreviouslyConsumed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingStackArgumentFlow {
    pub call_runtime: u32,
    pub target_reg: Reg,
    pub argument_store_runtime: u32,
    pub argument_reg: Reg,
    pub prior_consuming_calls: Vec<u32>,
    pub origin: OutgoingStackArgumentOrigin,
}

fn is_call(instruction: &Instruction) -> bool {
    matches!(instruction.kind, K::Jal { .. } | K::Jalr { .. })
}

pub fn find_outgoing_stack_argument_flows(
    instructions: &[Instruction],
    stack_offset: i16,
) -> Vec<OutgoingStackArgumentFlow> {
    let mut rows = Vec::new();
    for (call_index, call) in instructions.iter().enumerate() {
        let K::Jalr {
            rd: Reg::RA,
            rs: target_reg,
        } = call.kind
        else {
            continue;
        };
        let Some(frame_start) = instructions[..=call_index]
            .iter()
            .rposition(|instruction| {
                matches!(instruction.kind, K::Addiu { rt: Reg::SP, rs: Reg::SP, imm } if imm < 0)
            })
        else {
            continue;
        };
        let K::Addiu { imm, .. } = instructions[frame_start].kind else {
            unreachable!("frame 시작은 addiu sp");
        };
        let Some(frame_bytes) = imm.checked_neg() else {
            continue;
        };
        let frame_before_call = &instructions[frame_start..call_index];
        let Some(ra_save) = frame_before_call.iter().position(|instruction| {
            matches!(
                instruction.kind,
                K::Sw {
                    rt: Reg::RA,
                    base: Reg::SP,
                    ..
                }
            )
        }) else {
            continue;
        };
        if frame_before_call[ra_save + 1..].iter().any(|instruction| {
            matches!(
                instruction.kind,
                K::Addiu {
                    rt: Reg::SP,
                    rs: Reg::SP,
                    imm,
                } if imm == frame_bytes
            )
        }) {
            continue;
        }

        let delay_store = instructions.get(call_index + 1).filter(|instruction| {
            matches!(
                instruction.kind,
                K::Sw {
                    base: Reg::SP,
                    offset,
                    ..
                } if offset == stack_offset
            )
        });
        let before_store_index = frame_before_call[ra_save + 1..]
            .iter()
            .rposition(|instruction| {
                matches!(
                    instruction.kind,
                    K::Sw {
                        base: Reg::SP,
                        offset,
                        ..
                    } if offset == stack_offset
                )
            })
            .map(|index| frame_start + ra_save + 1 + index);
        let (argument_store_index, argument_store) = if let Some(delay_store) = delay_store {
            (call_index + 1, delay_store)
        } else if let Some(store_index) = before_store_index {
            (store_index, &instructions[store_index])
        } else {
            continue;
        };
        let K::Sw {
            rt: argument_reg, ..
        } = argument_store.kind
        else {
            unreachable!("outgoing argument source는 sw");
        };
        let is_callee_save_slot = instructions[call_index + 1..]
            .iter()
            .take_while(|instruction| {
                !matches!(
                    instruction.kind,
                    K::Addiu {
                        rt: Reg::SP,
                        rs: Reg::SP,
                        imm,
                    } if imm == frame_bytes
                )
            })
            .any(|instruction| {
                matches!(
                    instruction.kind,
                    K::Lw {
                        rt,
                        base: Reg::SP,
                        offset,
                    } if rt == argument_reg && offset == stack_offset
                )
            });
        if is_callee_save_slot {
            continue;
        }

        let mut prior_consuming_calls = Vec::new();
        if argument_store_index <= call_index {
            prior_consuming_calls.extend(
                instructions[argument_store_index + 1..call_index]
                    .iter()
                    .filter(|instruction| is_call(instruction))
                    .map(|instruction| instruction.addr),
            );
            if argument_store_index > 0 && is_call(&instructions[argument_store_index - 1]) {
                prior_consuming_calls.insert(0, instructions[argument_store_index - 1].addr);
            }
        }
        let origin = if argument_store_index == call_index + 1 {
            OutgoingStackArgumentOrigin::CurrentCallDelaySlot
        } else if prior_consuming_calls.is_empty() {
            OutgoingStackArgumentOrigin::CurrentCallSetup
        } else {
            OutgoingStackArgumentOrigin::PreviouslyConsumed
        };
        rows.push(OutgoingStackArgumentFlow {
            call_runtime: call.addr,
            target_reg,
            argument_store_runtime: argument_store.addr,
            argument_reg,
            prior_consuming_calls,
            origin,
        });
    }
    rows
}

#[cfg(test)]
#[path = "outgoing_stack_arguments_tests.rs"]
mod outgoing_stack_arguments_tests;
