//! 참조 그래프 — fan-in / fan-out.
//!
//! **fan-in**: 이 주소를 누가 참조하는가. 훅 지점을 정할 때, 그리고 어떤
//! 함수를 바꾸면 어디가 영향받는지 볼 때 쓴다.
//!
//! **fan-out**: 이 함수가 무엇을 참조하는가. 폰트 렌더 경로처럼 저장에서
//! 소비까지 따라갈 때 쓴다.
//!
//! 그래프는 정적 참조만 담는다. 레지스터 간접 분기와 런타임 생성 주소는
//! 여기 없다. **엣지가 없다는 것이 도달 불가의 증거가 아니다.**

use std::collections::{BTreeMap, BTreeSet};

use crate::coords::{normalize_kseg, ExeMap};
use crate::isa::{FlowKind, Instruction};

use super::xref::{self, AddressLoad};

/// 참조 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefKind {
    /// `jal` — 서브루틴 호출.
    Call,
    /// `j` — 무조건 분기.
    Jump,
    /// 조건 분기.
    Branch,
    /// `lui`+결합으로 만든 데이터 주소 참조.
    Data,
}

/// 참조 엣지 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ref {
    pub from: u32,
    pub to: u32,
    pub kind: RefKind,
}

/// 한 EXE의 정적 참조 그래프.
#[derive(Debug, Clone)]
pub struct RefGraph {
    map: ExeMap,
    instructions: BTreeMap<u32, Instruction>,
    incoming: BTreeMap<u32, Vec<Ref>>,
    outgoing: BTreeMap<u32, Vec<Ref>>,
    functions: BTreeSet<u32>,
    /// 정적 `jal` 타깃이 아니라 외부에서 넣은 함수 시작 후보.
    seeded: BTreeSet<u32>,
}

impl RefGraph {
    /// 명령 목록에서 그래프를 만든다.
    ///
    /// `instructions`는 선형 스윕이든 재귀 추적이든 상관없지만, 어느 쪽을
    /// 넣었는지가 결과의 의미를 바꾼다. 선형 스윕은 데이터를 코드로 오인한
    /// 가짜 엣지를 만들 수 있다.
    pub fn build(map: ExeMap, instructions: &[Instruction]) -> Self {
        Self::build_with_seeds(map, instructions, &[])
    }

    /// 함수 시작 후보를 추가로 넣어 그래프를 만든다.
    ///
    /// `function_seeds`에는 정적 `jal` 타깃이 아닌 함수 시작 후보를 넣는다 —
    /// 함수 포인터 테이블 항목, 인터럽트 핸들러, 관측으로 얻은 주소 등이다.
    /// 이 시드가 있어야 [`Self::seeded_without_callers`]가 의미를 가진다.
    pub fn build_with_seeds(
        map: ExeMap,
        instructions: &[Instruction],
        function_seeds: &[u32],
    ) -> Self {
        let mut g = Self {
            map,
            instructions: instructions.iter().map(|i| (i.addr, *i)).collect(),
            incoming: BTreeMap::new(),
            outgoing: BTreeMap::new(),
            functions: BTreeSet::new(),
            seeded: BTreeSet::new(),
        };

        for ins in instructions {
            let (to, kind) = match ins.flow_kind() {
                FlowKind::Call { target } => (target, RefKind::Call),
                FlowKind::Jump { target } => (target, RefKind::Jump),
                FlowKind::Branch { taken } => (taken, RefKind::Branch),
                _ => continue,
            };
            let to = normalize_kseg(to);
            if kind == RefKind::Call {
                g.functions.insert(to);
            }
            g.add(Ref {
                from: ins.addr,
                to,
                kind,
            });
        }

        g.functions.insert(map.entry_pc);
        for &seed in function_seeds {
            g.functions.insert(normalize_kseg(seed));
        }
        g.seeded = function_seeds.iter().map(|&s| normalize_kseg(s)).collect();

        for load in xref::find_address_loads(instructions, xref::DEFAULT_WINDOW) {
            g.add(Ref {
                from: load.lui_addr,
                to: load.value,
                kind: RefKind::Data,
            });
        }

        for v in g.incoming.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        for v in g.outgoing.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        g
    }

    fn add(&mut self, r: Ref) {
        self.incoming.entry(r.to).or_default().push(r);
        self.outgoing.entry(r.from).or_default().push(r);
    }

    /// 이 주소를 가리키는 참조들.
    pub fn fan_in(&self, addr: u32) -> &[Ref] {
        self.incoming
            .get(&normalize_kseg(addr))
            .map_or(&[][..], |v| v)
    }

    /// 이 명령이 만드는 참조들.
    pub fn fan_out(&self, addr: u32) -> &[Ref] {
        self.outgoing
            .get(&normalize_kseg(addr))
            .map_or(&[][..], |v| v)
    }

    /// 이 주소를 호출하는 지점들.
    pub fn callers(&self, addr: u32) -> Vec<u32> {
        self.fan_in(addr)
            .iter()
            .filter(|r| r.kind == RefKind::Call)
            .map(|r| r.from)
            .collect()
    }

    /// `func`부터 다음 함수 시작 직전까지가 호출하는 함수들.
    pub fn callees(&self, func: u32) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .function_body(func)
            .flat_map(|addr| self.fan_out(addr))
            .filter(|r| r.kind == RefKind::Call)
            .map(|r| r.to)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// `func`가 참조하는 데이터 주소들.
    pub fn data_refs(&self, func: u32) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .function_body(func)
            .flat_map(|addr| self.fan_out(addr))
            .filter(|r| r.kind == RefKind::Data)
            .map(|r| r.to)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// 함수 본문의 주소 범위를 훑는다.
    ///
    /// 경계는 "다음 함수 시작 직전"이라는 **근사**다. 함수 사이에 데이터가
    /// 끼거나 꼬리 호출이 있으면 틀린다.
    fn function_body(&self, func: u32) -> impl Iterator<Item = u32> + '_ {
        let start = normalize_kseg(func);
        let end = self
            .functions
            .range((start + 4)..)
            .next()
            .copied()
            .unwrap_or_else(|| self.map.runtime_range().1);
        self.instructions.range(start..end).map(|(a, _)| *a)
    }

    /// 호출 대상으로 확인된 함수 시작 주소들.
    pub fn functions(&self) -> impl Iterator<Item = u32> + '_ {
        self.functions.iter().copied()
    }

    /// 시드로 넣은 함수 중 정적 호출자가 없는 것 — **간접 호출 후보**.
    ///
    /// 정적 `jal` 타깃은 정의상 호출자가 있으므로 여기 나오지 않는다.
    /// 따라서 시드 없이 부른 그래프에서는 항상 비어 있다.
    ///
    /// 결과는 "이 함수는 간접적으로만 불린다"는 **후보**이지, 도달 불가의
    /// 증거가 아니다. 함수 포인터 테이블·점프 테이블·인터럽트 벡터는 이
    /// 그래프 밖에 있다.
    pub fn seeded_without_callers(&self) -> Vec<u32> {
        self.seeded
            .iter()
            .copied()
            .filter(|&f| f != self.map.entry_pc && self.callers(f).is_empty())
            .collect()
    }

    /// 참조 지점이 가장 많은 데이터 주소부터 돌려준다.
    ///
    /// 폰트 테이블이나 전역 상태처럼 여러 곳이 공유하는 구조를 찾는다.
    pub fn hot_data(&self, limit: usize) -> Vec<(u32, usize)> {
        let mut v: Vec<(u32, usize)> = self
            .incoming
            .iter()
            .filter_map(|(&to, refs)| {
                let n = refs.iter().filter(|r| r.kind == RefKind::Data).count();
                (n > 0).then_some((to, n))
            })
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(limit);
        v
    }

    /// `from`에서 `to`까지 호출로 이어지는 경로 하나. 없으면 `None`.
    ///
    /// 저장 → 소비 경로를 확인할 때 쓴다.
    pub fn call_path(&self, from: u32, to: u32) -> Option<Vec<u32>> {
        let (from, to) = (normalize_kseg(from), normalize_kseg(to));
        let mut prev: BTreeMap<u32, u32> = BTreeMap::new();
        let mut seen: BTreeSet<u32> = BTreeSet::from([from]);
        let mut queue = std::collections::VecDeque::from([from]);

        while let Some(cur) = queue.pop_front() {
            if cur == to {
                let mut path = vec![cur];
                let mut node = cur;
                while let Some(&p) = prev.get(&node) {
                    path.push(p);
                    node = p;
                }
                path.reverse();
                return Some(path);
            }
            for callee in self.callees(cur) {
                if seen.insert(callee) {
                    prev.insert(callee, cur);
                    queue.push_back(callee);
                }
            }
        }
        None
    }

    /// 주소별 참조 요약 한 줄.
    pub fn summary(&self, addr: u32) -> String {
        let a = normalize_kseg(addr);
        let fi = self.fan_in(a);
        let fo = self.fan_out(a);
        let count = |refs: &[Ref], k: RefKind| refs.iter().filter(|r| r.kind == k).count();
        format!(
            "0x{a:08X}  fan-in: call {} jump {} branch {} data {}  |  fan-out: call {} jump {} branch {} data {}",
            count(fi, RefKind::Call),
            count(fi, RefKind::Jump),
            count(fi, RefKind::Branch),
            count(fi, RefKind::Data),
            count(fo, RefKind::Call),
            count(fo, RefKind::Jump),
            count(fo, RefKind::Branch),
            count(fo, RefKind::Data),
        )
    }
}

/// 주소 로드 결과를 대상별로 묶어 요약한다.
pub fn data_reference_report(loads: &[AddressLoad], limit: usize) -> Vec<(u32, Vec<u32>)> {
    let mut v: Vec<(u32, Vec<u32>)> = xref::group_by_target(loads).into_iter().collect();
    v.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    v.truncate(limit);
    v
}

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod analysis_tests;
