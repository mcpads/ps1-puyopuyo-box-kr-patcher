//! 선언한 대상 ISA 프로필.
//!
//! 기계어 생성·이동은 **선언한 프로필의 전체**를 검산해야 승인된다. 패치에
//! 필요한 일부 명령만 검증하고 완전성을 주장하지 않는다.
//!
//! 프로필은 코드가 아니라 **선언**이다. 여기 없는 명령이 패치에 필요해지면
//! 프로필을 먼저 넓히고, 넓힌 범위 전체를 다시 검산한다.

use super::instruction::InstructionKind;

/// 대상 ISA 프로필 식별자.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsaProfile {
    /// MIPS R3000A + COP0 + COP2/GTE. PlayStation (SLPS-03114) 대상.
    ///
    /// 포함: R/I/J-type 정수 연산, 시프트, 곱셈·나눗셈과 HI/LO 전송,
    /// 정렬·비정렬 로드/스토어, 분기·점프, `syscall`/`break`, COP0 전송과
    /// `rfe`, COP2 전송·메모리 접근, GTE 커맨드 22종.
    ///
    /// 제외: COP1(FPU) — PS1에 없음. MIPS II 이상 전용 명령, 64비트 명령,
    /// 캐시 명령.
    R3000aGte,
}

impl IsaProfile {
    /// 이 프로젝트가 선언한 프로필.
    pub const DECLARED: IsaProfile = IsaProfile::R3000aGte;

    pub fn name(self) -> &'static str {
        match self {
            Self::R3000aGte => "mips-r3000a+cop0+cop2gte",
        }
    }

    /// 이 명령이 프로필 안에 있는가.
    ///
    /// `Unknown`과 미지 GTE funct는 프로필 밖이다. 디스어셈블 조사에서는
    /// 이들을 남겨 두지만, 기계어 쓰기 검산에서는 거부한다.
    pub fn contains(self, kind: &InstructionKind) -> bool {
        match self {
            Self::R3000aGte => kind.is_decoded(),
        }
    }
}

impl std::fmt::Display for IsaProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
