//! 주소 로드 패턴 추출.
//!
//! MIPS는 32비트 상수를 한 명령에 담을 수 없어 `lui` + `ori`/`addiu` 쌍으로
//! 만든다. 이 쌍을 찾으면 코드가 참조하는 **데이터 주소**를 알 수 있다 —
//! 폰트 테이블, 문자열 포인터, VDD 디스크립터 배열을 찾는 출발점이다.
//!
//! 두 명령이 반드시 인접하지는 않는다. 컴파일러가 사이에 다른 명령을 끼워
//! 넣으므로 창(window) 안에서 같은 레지스터를 찾는다.
//!
//! 결과는 **후보**다. 그 주소를 실제로 소비하는지는 소비 코드로 확정한다.

use std::collections::BTreeMap;

use crate::isa::{Instruction, InstructionKind, Reg};

/// `lui`+하위 결합으로 만들어진 주소 참조.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressLoad {
    /// `lui`가 있는 주소.
    pub lui_addr: u32,
    /// 하위 비트를 더한 명령의 주소.
    pub combine_addr: u32,
    /// 대상 레지스터.
    pub reg: Reg,
    /// 조립된 32비트 값.
    pub value: u32,
    /// 결합 방식.
    pub form: LoadForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadForm {
    /// `lui` + `ori` — 비트 OR. 부호 확장 없음.
    Ori,
    /// `lui` + `addiu` — 부호 확장 덧셈.
    Addiu,
    /// `lui` + 로드/스토어의 offset — 베이스 주소 + 변위.
    MemoryOffset,
}

/// 기본 탐색 창. `lui`와 결합 명령 사이에 낄 수 있는 명령 수.
pub const DEFAULT_WINDOW: usize = 8;

/// 주소 로드 쌍을 찾는다.
///
/// `instructions`는 주소 오름차순이어야 한다.
pub fn find_address_loads(instructions: &[Instruction], window: usize) -> Vec<AddressLoad> {
    let mut out = Vec::new();

    for (i, ins) in instructions.iter().enumerate() {
        let InstructionKind::Lui { rt: lui_rt, imm } = ins.kind else {
            continue;
        };
        let hi = (imm as u32) << 16;

        for probe in instructions.iter().skip(i + 1).take(window) {
            // 같은 레지스터를 덮어쓰면 그 lui의 상위 비트는 죽는다.
            let clobbered = probe.dest_reg() == Some(lui_rt);

            let hit = match probe.kind {
                InstructionKind::Ori { rt, rs, imm } if rs == lui_rt => {
                    Some((rt, hi | imm as u32, LoadForm::Ori))
                }
                InstructionKind::Addiu { rt, rs, imm } if rs == lui_rt => {
                    Some((rt, hi.wrapping_add(imm as i32 as u32), LoadForm::Addiu))
                }
                _ => memory_base(&probe.kind, lui_rt).map(|off| {
                    (
                        lui_rt,
                        hi.wrapping_add(off as i32 as u32),
                        LoadForm::MemoryOffset,
                    )
                }),
            };

            if let Some((reg, value, form)) = hit {
                out.push(AddressLoad {
                    lui_addr: ins.addr,
                    combine_addr: probe.addr,
                    reg,
                    value,
                    form,
                });
                // 메모리 접근은 베이스를 소비만 하므로 계속 훑는다.
                if form != LoadForm::MemoryOffset {
                    break;
                }
            }

            if clobbered {
                break;
            }
        }
    }

    out
}

/// 로드/스토어가 `base`를 베이스로 쓰면 그 변위를 돌려준다.
fn memory_base(kind: &InstructionKind, base_reg: Reg) -> Option<i16> {
    use InstructionKind as K;
    let (base, offset) = match *kind {
        K::Lb { base, offset, .. }
        | K::Lbu { base, offset, .. }
        | K::Lh { base, offset, .. }
        | K::Lhu { base, offset, .. }
        | K::Lw { base, offset, .. }
        | K::Lwl { base, offset, .. }
        | K::Lwr { base, offset, .. }
        | K::Sb { base, offset, .. }
        | K::Sh { base, offset, .. }
        | K::Sw { base, offset, .. }
        | K::Swl { base, offset, .. }
        | K::Swr { base, offset, .. }
        | K::Lwc2 { base, offset, .. }
        | K::Swc2 { base, offset, .. } => (base, offset),
        _ => return None,
    };
    (base == base_reg).then_some(offset)
}

/// 참조된 주소별로 참조 지점을 모은다 — 데이터 fan-in.
pub fn group_by_target(loads: &[AddressLoad]) -> BTreeMap<u32, Vec<u32>> {
    let mut out: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for l in loads {
        out.entry(l.value).or_default().push(l.lui_addr);
    }
    out
}

#[cfg(test)]
#[path = "xref_tests.rs"]
mod xref_tests;
