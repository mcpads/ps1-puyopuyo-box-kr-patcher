//! PS1 ぷよぷよBOX (SLPS-03114) 한글 패치 빌드.
//!
//! 경계와 검증 계약은 `docs/pipeline_design.md`, 범위와 판정 순서는
//! `docs/project_strategy.md`, 관측된 사실은 `docs/rom_analysis.md`가
//! 소유한다.
//!
//! # 현재 구현 상태
//!
//! | 모듈 | 상태 |
//! |------|------|
//! | [`analysis`] | 부분 구현 — QUEST·서브게임 EXE consumer와 SEQ·text·BTX model의 source-bound 통합 분석 |
//! | [`btx`] | 구현됨 — BTX indexed bitmap codec·layout 검증 |
//! | [`coords`] | 구현됨 — 좌표계 변환, EXE·RAM 연속 자산 적재 사상 |
//! | [`isa`] | 구현됨 — R3000A 해석·조립·기계어 검산 |
//! | [`disasm`] | 구현됨 — 선형·재귀 디스어셈블, fan-in/fan-out, EXE별 DRAC message consumer, generic text 생성자 bridge와 FF12/15 scratch target |
//! | [`patch`] | 구현됨 — 영역 선언, Expected Write |
//! | [`qmp`] | 구현됨 — QUEST QMP tile map codec·원본 inventory 검증 |
//! | [`disc`] | 부분 구현 — 읽기·같은 크기 Form1 쓰기·32M 예약 extent; ISO 성장 재조립 미완 |
//! | [`archive`] | 구현됨 — VDD lossless 파싱·재조립·제자리 교체 |
//! | [`compress`] | 구현됨 — CNX v2 해제·압축 |
//! | [`script`] | 부분 구현 — QUEST SEQ dispatch 256종·base 직접 전진 71곳·공용 branch/return 전이 8종·OP28 하위 32종·고위 직접 전진 76곳·비증가 전이 4종·text control 32종, TOWN00 선두·root OP03·OP0D·본문 OP85, RPG_SEQ 주소 후보와 OP21 token 보존 추출·113개 lossless 문서 왕복·605개 번역 검토 단위, ITEM_DAT 이름·설명 222개 source 단위, 비-QUEST SEQ overlay/raw 207/117과 DRAC 내부 root·5개 message·270B relink; opaque 구조·초과 relocation 미완 |
//! | [`graphics`] | 부분 구현 — BTX·QMP·TUU CEL 왕복, runtime layout·text block 계약·category별 합성 |
//! | [`font`] | 부분 구현 — Maplestory Light 원천·QUEST·DEMOFONT·COM_FONT 물리/소비 경계 |
//! | [`text`] | 구현됨 — 엄격한 Shift-JIS decode·encode |
//! | [`runtime`] | 부분 구현 — Mednafen PSX state Main RAM·raw 2 MiB RAM 읽기, TOWN00 root와 PUYO2 합성 text record·DRAC overlay/message object 관측 |
//! | [`pipeline`] | 부분 구현 — QUEST 첫/두 대사 page hook PoC·580개 route·65,492B 행 사전 sparse cache와 1,144B typed hook의 full-disc Expected Write control, 셸 첫 baked 문장 control·한글 PoC, typed no-op hook control |
//!
//! 기계어 쓰기는 typed [`isa::InstructionKind`] 원천을
//! [`isa::verify_typed_source`]로 조립·검산한 경우에만 승인한다.
//! [`pipeline::quest_hook_poc`]의 no-op control과 QUEST font page 전환
//! PoC body가 이 경로로 실제 기계어 Expected Write를 만든다. 580개 OP21
//! 번역 route는 비배포 development build에 기술 삽입된다. ITEM_DAT 이름
//! writer는 development build에 결속됐고 설명 111개는 source-bound 수동
//! 초안까지 닫혔다. 승인 layout을 제품 입력으로 받는 gate와 설명 writer는
//! 아직 열려 있다.
//!
//! 조사 단계의 재현 스크립트는 `tools/survey/`에 있으며 **제품 빌드의
//! 입력이 아니다**.

pub mod analysis;
pub mod archive;
pub mod btx;
pub mod compress;
pub mod coords;
pub mod disasm;
pub mod disc;
pub mod font;
pub mod graphics;
pub mod isa;
pub mod patch;
pub mod pipeline;
pub mod qmp;
pub mod review;
pub mod runtime;
pub mod script;
pub mod text;

/// 지원하는 원본 리비전.
///
/// 이 값과 다른 입력은 빌드 대상이 아니다. 다른 리비전에서의 성공은 이
/// 프로젝트의 배포 가능 판정으로 승격하지 않는다.
pub mod source {
    /// 원본 BIN 크기 (바이트).
    pub const BIN_LEN: u64 = 141_853_824;

    /// 원본 BIN SHA-1.
    pub const BIN_SHA1: &str = "ca01a98ccc5ca796df082782a6dd1adee856c064";

    /// ISO 9660 볼륨 식별자.
    pub const VOLUME_ID: &str = "SLPS03114";

    /// 트랙 섹터 크기 (Mode2/2352).
    pub const SECTOR_LEN: usize = 2352;

    /// Mode2 Form1 사용자 데이터 오프셋과 길이.
    pub const FORM1_DATA_OFFSET: usize = 24;
    pub const FORM1_DATA_LEN: usize = 2048;

    /// 볼륨 총 섹터 수.
    pub const VOLUME_SECTORS: u32 = 60_312;

    /// 셸과 서브게임 10개의 지원 원본 ISO 경로와 EXE 이름.
    pub const BOX_EXE_ISO_PATHS: [(&str, &str); 11] = [
        ("/SLPS_031.14;1", "SLPS_031.14"),
        ("/GOCHA1/GOCHA1.EXE;1", "GOCHA1.EXE"),
        ("/GOCHA2/GOCHA2.EXE;1", "GOCHA2.EXE"),
        ("/HORI2/HORI2.EXE;1", "HORI2.EXE"),
        ("/HORI4/HORI4.EXE;1", "HORI4.EXE"),
        ("/KOUSIKI/KOUSIKI.EXE;1", "KOUSIKI.EXE"),
        ("/MINPUYO/MINPUYO.EXE;1", "MINPUYO.EXE"),
        ("/PCOMI/PCOMI.EXE;1", "PCOMI.EXE"),
        ("/PUYO1/PUYO1.EXE;1", "PUYO1.EXE"),
        ("/PUYO2/PUYO2.EXE;1", "PUYO2.EXE"),
        ("/QUEST/QUEST.EXE;1", "QUEST.EXE"),
    ];
}
