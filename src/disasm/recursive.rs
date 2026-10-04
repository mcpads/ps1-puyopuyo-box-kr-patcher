//! 재귀 하강 디스어셈블.
//!
//! 엔트리에서 제어 흐름을 따라가며 **실제로 도달하는 명령만** 해석한다.
//! 선형 스윕과 달리 데이터를 코드로 오인하지 않지만, 레지스터 간접 분기
//! (`jr $t2`, 점프 테이블) 너머는 따라가지 못한다.
//!
//! 따라서 이 결과의 미도달 구간은 "도달 불가"의 증거가 **아니다.** 도달
//! 불가를 주장하려면 간접 분기 타깃을 별도로 판정해야 한다.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::coords::{normalize_kseg, ExeMap};
use crate::isa::{decode, FlowKind, Instruction};

/// 재귀 하강 결과.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    /// 도달한 명령. 주소 오름차순.
    pub instructions: BTreeMap<u32, Instruction>,
    /// `jal` 타깃 — 함수 시작 후보.
    pub call_targets: BTreeSet<u32>,
    /// 따라가지 못한 레지스터 간접 분기의 위치.
    ///
    /// 비어 있지 않으면 이 추적의 커버리지는 완전하지 않다.
    pub indirect_sites: BTreeSet<u32>,
    /// 적재 범위 밖을 가리킨 분기·호출 (from, to).
    pub out_of_range: Vec<(u32, u32)>,
}

impl Trace {
    /// 커버리지가 완전하다고 주장할 수 있는가.
    ///
    /// 간접 분기가 하나라도 남아 있으면 완전성을 주장하지 않는다.
    pub fn coverage_is_complete(&self) -> bool {
        self.indirect_sites.is_empty()
    }
}

/// `map.entry_pc`와 추가 시드에서 시작해 도달 가능한 코드를 해석한다.
///
/// `text`는 PSX-EXE 헤더를 제외한 텍스트 바이트다.
pub fn trace(map: &ExeMap, text: &[u8], extra_seeds: &[u32]) -> Trace {
    let mut out = Trace::default();
    let mut queue: VecDeque<u32> = VecDeque::new();
    let mut seen: BTreeSet<u32> = BTreeSet::new();

    let push = |q: &mut VecDeque<u32>, seen: &mut BTreeSet<u32>, addr: u32| {
        let a = normalize_kseg(addr);
        if map.contains(a) && a.is_multiple_of(4) && seen.insert(a) {
            q.push_back(a);
        }
    };

    push(&mut queue, &mut seen, map.entry_pc);
    for &s in extra_seeds {
        push(&mut queue, &mut seen, s);
    }

    let fetch = |addr: u32| -> Option<Instruction> {
        let idx = map.text_offset(normalize_kseg(addr))?;
        let bytes = text.get(idx..idx + 4)?;
        Some(decode(
            normalize_kseg(addr),
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        ))
    };

    // 흐름을 실제로 따라간 지점. 딜레이 슬롯만 해석한 주소와 구분한다.
    // 구분하지 않으면, 딜레이 슬롯이자 다른 분기의 타깃이기도 한 명령에서
    // 이후 흐름을 놓친다.
    let mut walked: BTreeSet<u32> = BTreeSet::new();

    while let Some(start) = queue.pop_front() {
        let mut pc = start;
        while let Some(ins) = fetch(pc) {
            if !walked.insert(pc) {
                // 이미 흐름을 따라간 지점에 합류했다.
                break;
            }
            out.instructions.insert(pc, ins);

            let flow = ins.flow_kind();

            // 딜레이 슬롯은 분기 성립 여부와 무관하게 실행된다.
            if flow.has_delay_slot() {
                if let Some(slot) = fetch(pc.wrapping_add(4)) {
                    out.instructions.entry(pc.wrapping_add(4)).or_insert(slot);
                }
            }

            match flow {
                FlowKind::Sequential => {
                    pc = pc.wrapping_add(4);
                }
                FlowKind::Trap => {
                    // syscall/break 뒤로도 복귀할 수 있다.
                    pc = pc.wrapping_add(4);
                }
                FlowKind::Branch { taken } => {
                    let t = normalize_kseg(taken);
                    if map.contains(t) {
                        push(&mut queue, &mut seen, t);
                    } else {
                        out.out_of_range.push((pc, t));
                    }
                    // 미성립 경로는 딜레이 슬롯 다음으로 이어진다.
                    pc = pc.wrapping_add(8);
                }
                FlowKind::Call { target } => {
                    let t = normalize_kseg(target);
                    out.call_targets.insert(t);
                    if map.contains(t) {
                        push(&mut queue, &mut seen, t);
                    } else {
                        out.out_of_range.push((pc, t));
                    }
                    pc = pc.wrapping_add(8);
                }
                FlowKind::Jump { target } => {
                    let t = normalize_kseg(target);
                    if map.contains(t) {
                        push(&mut queue, &mut seen, t);
                    } else {
                        out.out_of_range.push((pc, t));
                    }
                    break; // 무조건 분기 — 뒤로 이어지지 않는다
                }
                FlowKind::JumpRegister { .. } => {
                    out.indirect_sites.insert(pc);
                    break;
                }
                FlowKind::Return => break,
            }
        }
    }

    out
}

#[cfg(test)]
#[path = "recursive_tests.rs"]
mod recursive_tests;
