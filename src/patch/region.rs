//! 이미지 영역 선언.
//!
//! 모든 Expected Write는 **정확히 하나의** 선언된 영역 안에 들어가야 한다.
//! 영역 없이 쓰는 것, 영역 종류와 다른 의도로 쓰는 것, 보호 영역에 쓰는
//! 것은 전부 빌드 실패다.

use std::ops::Range;

/// 영역 종류. Expected Write의 의도와 반드시 일치해야 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// 게임 데이터 — 텍스트, 그래픽, 테이블.
    Data,
    /// 구조 메타데이터 — VDD 엔트리, ISO 레코드, 섹터 헤더.
    Metadata,
    /// 실행 코드. 여기 쓰려면 ISA 검산을 통과해야 한다.
    MachineCode,
    /// 같은 논리 파일의 위치만 바꾸는 불변 원본 byte copy.
    ///
    /// 실행 파일을 ISO의 다른 extent로 옮길 때 원본 text를 `Data`로
    /// 위장하지 않도록 별도 종류로 둔다. 내용 변경은 허용하지 않는다.
    FileRelocation,
    /// 건드리면 안 되는 영역. 어떤 쓰기도 허용하지 않는다.
    Protected,
}

impl RegionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Data => "data",
            Self::Metadata => "metadata",
            Self::MachineCode => "machine_code",
            Self::FileRelocation => "file_relocation",
            Self::Protected => "protected",
        }
    }
}

/// 이미지의 한 영역. 범위는 **이미지 바이트 오프셋**이다.
///
/// 논리 파일 오프셋이나 런타임 주소를 그대로 넣으면 안 된다.
/// 변환은 [`crate::coords`]가 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRegion {
    pub id: String,
    pub range: Range<u64>,
    pub kind: RegionKind,
    /// 이 영역을 이렇게 분류한 근거. 비어 있으면 실패다.
    pub reason: String,
}

impl ImageRegion {
    pub fn new(
        id: impl Into<String>,
        range: Range<u64>,
        kind: RegionKind,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            range,
            kind,
            reason: reason.into(),
        }
    }

    pub fn contains(&self, other: &Range<u64>) -> bool {
        self.range.start <= other.start && other.end <= self.range.end
    }
}

/// 두 범위가 겹치는가.
pub fn intersects(a: &Range<u64>, b: &Range<u64>) -> bool {
    a.start < b.end && b.start < a.end
}

/// 무변경을 선언한 경계.
///
/// 빌드 후 이 범위가 원본과 바이트 동일하지 않으면 실패한다. "여긴 안
/// 건드렸다"는 주장을 기계적으로 검증하는 수단이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnchangedBoundary {
    pub id: String,
    pub range: Range<u64>,
    /// 무변경이어야 하는 이유.
    pub reason: String,
}

impl UnchangedBoundary {
    pub fn new(id: impl Into<String>, range: Range<u64>, reason: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            range,
            reason: reason.into(),
        }
    }
}
