//! 서브게임 type table literal ingress의 bounded dataflow 분석.
//!
//! 지원 원본의 compiler shape에서 `lui`+`addiu`로 type table root를 만든 뒤
//! `type*4 -> base load -> resource*36 -> descriptor`로 이어지는 경로를 찾는다.
//! 선형 sweep 위의 국소 분석이므로 도달 가능성이나 간접 alias 부재를 주장하지
//! 않는다.

use crate::isa::{Instruction, InstructionKind as K, Reg};

/// `lui`와 같은 register의 `addiu`로 만드는 type table root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescriptorRootLiteral {
    pub upper: u16,
    pub lower: i16,
}

impl DescriptorRootLiteral {
    pub const fn new(upper: u16, lower: i16) -> Self {
        Self { upper, lower }
    }
}

/// 지정한 root literal의 모든 하위 결합 주소를 찾는다.
///
/// `lookback` 안의 같은 register `lui`만 인정한다. 분기 delay slot과 분기
/// target에 같은 결합이 각각 있으면 둘을 모두 보존한다.
pub fn descriptor_root_literal_combines(
    instructions: &[Instruction],
    literal: DescriptorRootLiteral,
    lookback: usize,
) -> Vec<u32> {
    instructions
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| {
            let K::Addiu { rt, rs, imm } = instruction.kind else {
                return None;
            };
            if rt != rs || imm != literal.lower {
                return None;
            }
            instructions[index.saturating_sub(lookback)..index]
                .iter()
                .any(|probe| {
                    probe.kind
                        == (K::Lui {
                            rt,
                            imm: literal.upper,
                        })
                })
                .then_some(instruction.addr)
        })
        .collect()
}

fn operands_are(left: Reg, right: Reg, first: Reg, second: Reg) -> bool {
    (left == first && right == second) || (left == second && right == first)
}

fn register_not_clobbered(
    instructions: &[Instruction],
    producer: usize,
    consumer: usize,
    register: Reg,
) -> bool {
    producer < consumer
        && instructions[producer + 1..consumer]
            .iter()
            .all(|instruction| instruction.dest_reg() != Some(register))
}

/// root 결합 뒤의 `type*4 -> load -> resource*36 -> descriptor`를 찾는다.
///
/// 반환값은 descriptor pointer를 합성하는 `addu` 주소다. 분기 의미 전체를
/// 복원하지 않고 `scan_instructions` 안의 unclobbered register chain만
/// 검산한다.
pub fn descriptor_resource_x36_select(
    instructions: &[Instruction],
    root_combine_runtime: u32,
    literal: DescriptorRootLiteral,
    scan_instructions: usize,
) -> Option<u32> {
    let root_index = instructions
        .binary_search_by_key(&root_combine_runtime, |instruction| instruction.addr)
        .ok()?;
    let K::Addiu { rt: root, rs, imm } = instructions[root_index].kind else {
        return None;
    };
    if root != rs || imm != literal.lower {
        return None;
    }
    let end = (root_index + scan_instructions).min(instructions.len());

    for type_shift_index in root_index + 1..end {
        let K::Sll {
            rd: type_x4, sa: 2, ..
        } = instructions[type_shift_index].kind
        else {
            continue;
        };
        for slot_index in type_shift_index + 1..end {
            let K::Addu {
                rd: slot,
                rs: slot_left,
                rt: slot_right,
            } = instructions[slot_index].kind
            else {
                continue;
            };
            if !operands_are(slot_left, slot_right, root, type_x4)
                || !register_not_clobbered(instructions, root_index, slot_index, root)
                || !register_not_clobbered(instructions, type_shift_index, slot_index, type_x4)
            {
                continue;
            }
            for base_index in slot_index + 1..end {
                let K::Lw {
                    rt: descriptor_base,
                    base,
                    offset: 0,
                } = instructions[base_index].kind
                else {
                    continue;
                };
                if base != slot
                    || !register_not_clobbered(instructions, slot_index, base_index, slot)
                {
                    continue;
                }

                for resource_x8_index in root_index + 1..end {
                    let K::Sll {
                        rd: resource_x8,
                        rt: resource,
                        sa: 3,
                    } = instructions[resource_x8_index].kind
                    else {
                        continue;
                    };
                    for resource_x9_index in resource_x8_index + 1..end {
                        let K::Addu {
                            rd: resource_x9,
                            rs: x9_left,
                            rt: x9_right,
                        } = instructions[resource_x9_index].kind
                        else {
                            continue;
                        };
                        if !operands_are(x9_left, x9_right, resource_x8, resource)
                            || !register_not_clobbered(
                                instructions,
                                resource_x8_index,
                                resource_x9_index,
                                resource_x8,
                            )
                            || !register_not_clobbered(
                                instructions,
                                resource_x8_index,
                                resource_x9_index,
                                resource,
                            )
                        {
                            continue;
                        }
                        for resource_x36_index in resource_x9_index + 1..end {
                            let K::Sll {
                                rd: resource_x36,
                                rt,
                                sa: 2,
                            } = instructions[resource_x36_index].kind
                            else {
                                continue;
                            };
                            if rt != resource_x9
                                || !register_not_clobbered(
                                    instructions,
                                    resource_x9_index,
                                    resource_x36_index,
                                    resource_x9,
                                )
                            {
                                continue;
                            }
                            let descriptor_start = base_index.max(resource_x36_index) + 1;
                            for descriptor_index in descriptor_start..end {
                                let K::Addu {
                                    rs: descriptor_left,
                                    rt: descriptor_right,
                                    ..
                                } = instructions[descriptor_index].kind
                                else {
                                    continue;
                                };
                                if operands_are(
                                    descriptor_left,
                                    descriptor_right,
                                    descriptor_base,
                                    resource_x36,
                                ) && register_not_clobbered(
                                    instructions,
                                    base_index,
                                    descriptor_index,
                                    descriptor_base,
                                ) && register_not_clobbered(
                                    instructions,
                                    resource_x36_index,
                                    descriptor_index,
                                    resource_x36,
                                ) {
                                    return Some(instructions[descriptor_index].addr);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "subgame_seq_descriptor_ingress_tests.rs"]
mod tests;
