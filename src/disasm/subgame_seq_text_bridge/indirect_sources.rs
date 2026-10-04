//! 제5인자를 새로 쓰는 간접 jalr caller와 target table 검증.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::coords::{normalize_kseg, ExeMap};
use crate::disasm::outgoing_stack_arguments::{
    find_outgoing_stack_argument_flows, OutgoingStackArgumentOrigin,
};
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::subgame_seq_text_consumer::SubgameTextConsumerAnalysis;
use super::trace_analysis::{high_low, require};
use super::{
    error, hex, SubgameSeqTextBridgeError, SubgameSeqTextBridgeSpec, CALLER_STACK_ARGUMENT_OFFSET,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StackArg5JalrCandidate {
    pub(super) call_runtime: u32,
    pub(super) target_reg: Reg,
    pub(super) argument_store_runtime: u32,
}

/// 선형 initialized word에서 표준 frame의 직전 call delay 뒤 `sp+16`을
/// 새로 쓴 `jalr`를 찾는다. 현재 `jalr` delay slot의 write도 포함한다.
/// 함수 경계나 실제 호출 인자 의미를 확정하는 분석은 아니므로 결과는
/// caller 후보 분모로만 쓴다.
pub(super) fn find_stack_arg5_jalr_candidates(
    instructions: &[Instruction],
) -> Vec<StackArg5JalrCandidate> {
    find_outgoing_stack_argument_flows(instructions, CALLER_STACK_ARGUMENT_OFFSET)
        .into_iter()
        .filter(|flow| flow.origin != OutgoingStackArgumentOrigin::PreviouslyConsumed)
        .map(|flow| StackArg5JalrCandidate {
            call_runtime: flow.call_runtime,
            target_reg: flow.target_reg,
            argument_store_runtime: flow.argument_store_runtime,
        })
        .collect()
}

pub(super) fn verify_indirect_candidate(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    spec: SubgameSeqTextBridgeSpec,
    consumers: &SubgameTextConsumerAnalysis,
) -> Result<(usize, Vec<usize>), SubgameSeqTextBridgeError> {
    let candidates = find_stack_arg5_jalr_candidates(instructions);
    if candidates.len() != spec.stack_arg5_jalr.len() {
        return Err(error(format!(
            "{} per-call stack-arg5 jalr 후보 분모 불일치: expected={}, actual={candidates:?}",
            spec.exe,
            spec.stack_arg5_jalr.len(),
        )));
    }

    let mut constructor_target_counts = Vec::with_capacity(candidates.len());
    for (candidate, candidate_spec) in candidates.iter().zip(spec.stack_arg5_jalr) {
        if candidate.call_runtime != candidate_spec.call_runtime
            || candidate.argument_store_runtime != candidate_spec.argument_store_runtime
        {
            return Err(error(format!(
                "{} per-call stack-arg5 jalr 후보 주소 불일치: expected={candidate_spec:?}, actual={candidate:?}",
                spec.exe
            )));
        }

        match candidate_spec.table_kind {
            "text-control-ff00-ff2f" => {
                let consumer = consumers
                    .consumers
                    .iter()
                    .find(|consumer| {
                        consumer.exe == spec.exe && consumer.role == "argument-immediate"
                    })
                    .ok_or_else(|| {
                        error(format!("{} argument-immediate consumer 누락", spec.exe))
                    })?;
                let control_table = consumers
                    .control_tables
                    .iter()
                    .find(|table| table.exe == spec.exe)
                    .ok_or_else(|| error(format!("{} control table 누락", spec.exe)))?;
                if consumer.indirect_call_runtime != candidate.call_runtime
                    || consumer.control_table_runtime != candidate_spec.table_runtime
                    || control_table.runtime != candidate_spec.table_runtime
                    || candidate.target_reg != Reg::V1
                {
                    return Err(error(format!(
                        "{} per-call stack-arg5 jalr/text consumer 교차 불일치",
                        spec.exe
                    )));
                }
            }
            "format-callback-slot" => {
                let call = candidate.call_runtime;
                let high_runtime = call
                    .checked_sub(0x10)
                    .ok_or_else(|| error(format!("{} format callback 주소 underflow", spec.exe)))?;
                let load_runtime = call
                    .checked_sub(0x0C)
                    .ok_or_else(|| error(format!("{} format callback 주소 underflow", spec.exe)))?;
                let (high, low) = high_low(candidate_spec.table_runtime);
                require(
                    instructions,
                    spec,
                    high_runtime,
                    K::Lui {
                        rt: Reg::V0,
                        imm: high,
                    },
                )?;
                require(
                    instructions,
                    spec,
                    load_runtime,
                    K::Lw {
                        rt: Reg::V0,
                        base: Reg::V0,
                        offset: low,
                    },
                )?;
                if candidate.target_reg != Reg::V0
                    || candidate.argument_store_runtime != call.wrapping_add(4)
                {
                    return Err(error(format!(
                        "{} format callback jalr/delay 구조 불일치",
                        spec.exe
                    )));
                }
            }
            kind => {
                return Err(error(format!(
                    "{} 알 수 없는 per-call stack-arg5 target 종류: {kind}",
                    spec.exe
                )));
            }
        }

        let table_offset = map
            .text_offset(candidate_spec.table_runtime)
            .ok_or_else(|| error(format!("{} 간접 target source가 EXE 밖임", spec.exe)))?;
        let table_len = candidate_spec
            .table_entries
            .checked_mul(4)
            .ok_or_else(|| error(format!("{} 간접 target source 길이 overflow", spec.exe)))?;
        let table_end = table_offset
            .checked_add(table_len)
            .ok_or_else(|| error(format!("{} 간접 target source 끝 overflow", spec.exe)))?;
        let table = text
            .get(table_offset..table_end)
            .ok_or_else(|| error(format!("{} 간접 target source 범위 이탈", spec.exe)))?;
        let targets = table
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().expect("4바이트 word")))
            .collect::<Vec<_>>();
        let unique_targets = targets.iter().copied().collect::<BTreeSet<_>>();
        let zero_targets = targets.iter().filter(|&&target| target == 0).count();
        let outside_exe_targets = unique_targets
            .iter()
            .filter(|&&target| target != 0 && !map.contains(target))
            .count();
        let constructor = normalize_kseg(spec.constructor_runtime);
        let constructor_targets = targets
            .iter()
            .filter(|&&target| normalize_kseg(target) == constructor)
            .count();
        let digest: [u8; 32] = Sha256::digest(table).into();
        if unique_targets.len() != candidate_spec.table_unique_targets
            || zero_targets != candidate_spec.table_zero_targets
            || outside_exe_targets != candidate_spec.table_outside_exe_targets
            || constructor_targets != 0
            || digest != candidate_spec.table_sha256
        {
            return Err(error(format!(
                "{} 간접 target source 기준선 불일치: kind={}, unique={}/{}, zero={}/{}, outside={}/{}, constructor={constructor_targets}, sha={}",
                spec.exe,
                candidate_spec.table_kind,
                unique_targets.len(),
                candidate_spec.table_unique_targets,
                zero_targets,
                candidate_spec.table_zero_targets,
                outside_exe_targets,
                candidate_spec.table_outside_exe_targets,
                hex(&digest),
            )));
        }
        constructor_target_counts.push(constructor_targets);
    }
    Ok((candidates.len(), constructor_target_counts))
}
