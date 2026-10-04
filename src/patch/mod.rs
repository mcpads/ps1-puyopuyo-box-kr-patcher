//! 패치 적용 — 영역 선언과 Expected Write.
//!
//! 이 모듈의 목적은 **막는 것**이다. 다음은 전부 빌드 실패다.
//!
//! - 원본 바이트 전제 불일치
//! - 쓰기 겹침
//! - 보호 영역 침범, 영역 밖 쓰기, 의도와 영역 종류 불일치
//! - 데이터 의도로 기계어 영역 변경
//! - 검산되지 않은 기계어 (타입 수준에서 차단)
//! - 무변경 선언 경계의 바이트 차이
//! - 등록되지 않은 최종 diff
//!
//! 통과는 "이 변경이 옳다"가 아니라 "이 변경이 선언한 범위를 벗어나지
//! 않았다"만 뜻한다. 번역 품질·UX·실행 범위는 조사와 HITL이 판정한다.

pub mod region;
pub mod write_plan;

pub use region::{ImageRegion, RegionKind, UnchangedBoundary};
pub use write_plan::{
    plan_logical_copy, plan_logical_machine_code, plan_logical_write, ExpectedWrite,
    MachineCodeSegmentProof, PlanError, WriteIntent, WritePlan,
};
