//! `QUEST.EXE`의 `FFxx` 제어코드 layout.
//!
//! opcode는 SEQ에 `xx FF` 순서의 u16 LE로 저장된다. 이 표는
//! `QUEST.EXE`의 dispatch table `0x800B2998`과 각 handler가 현재 stream
//! pointer를 전진시키는 코드를 근거로 한다. layout은 32종 전부 확정했고,
//! 의미는 handler와 하위 소비 코드까지 닫힌 것만 별도 표로 노출한다.
//! 나머지는 handler가 실제로 읽는 인자와 읽지 않고 건너뛰는 reserved
//! byte만 구분한다.

use std::error::Error;
use std::fmt;

/// QUEST 제어코드의 바이트 layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestControlLayout {
    /// opcode 2바이트만 소비한다.
    OpcodeOnly,
    /// opcode 뒤의 u16 하나를 읽는다.
    U16,
    /// opcode 뒤의 u8 하나를 읽고 다음 1바이트는 건너뛴다.
    U8Reserved,
    /// opcode 뒤의 u8 두 개를 각각 읽는다.
    TwoU8,
    /// opcode 뒤의 부호 의미 미확정 16-bit word 두 개를 읽는다.
    TwoWords,
    /// opcode 뒤의 u8 두 개와 부호 의미 미확정 16-bit word 하나를 읽는다.
    TwoU8Word,
    /// opcode 뒤의 u8 두 개와 부호 의미 미확정 16-bit word 두 개를 읽는다.
    TwoU8TwoWords,
    /// opcode 뒤의 4바이트를 읽지 않고 건너뛴다.
    FourReserved,
    /// opcode 뒤의 4바이트를 건너뛴 뒤 다음 4바이트 경계로 정렬한다.
    FourReservedAlign4,
}

impl QuestControlLayout {
    /// 주어진 runtime stream 주소에서 token이 차지하는 전체 바이트 수.
    ///
    /// QUEST stream은 u16 단위이므로 홀수 주소는 승인하지 않는다. 고정 길이
    /// layout은 주소와 무관하고, `FourReservedAlign4`는 handler의
    /// `(pointer + 9) & !3` 계산을 그대로 재현한다.
    pub fn encoded_len(self, stream_address: u32) -> Option<usize> {
        if !stream_address.is_multiple_of(2) {
            return None;
        }
        match self {
            Self::OpcodeOnly => Some(2),
            Self::U16 | Self::U8Reserved | Self::TwoU8 => Some(4),
            Self::TwoWords | Self::TwoU8Word | Self::FourReserved => Some(6),
            Self::TwoU8TwoWords => Some(8),
            Self::FourReservedAlign4 => {
                let aligned_end = stream_address.checked_add(9)? & !3;
                usize::try_from(aligned_end.checked_sub(stream_address)?).ok()
            }
        }
    }

    /// handler가 opcode 뒤에서 실제로 읽는 인자 바이트 수.
    pub const fn argument_len(self) -> usize {
        match self {
            Self::OpcodeOnly | Self::FourReserved | Self::FourReservedAlign4 => 0,
            Self::U8Reserved => 1,
            Self::U16 | Self::TwoU8 => 2,
            Self::TwoWords | Self::TwoU8Word => 4,
            Self::TwoU8TwoWords => 6,
        }
    }

    /// handler가 opcode 뒤에서 읽지 않고 건너뛰는 고정 reserved 바이트 수.
    ///
    /// `FourReservedAlign4`의 끝 alignment padding 0–2바이트는 포함하지 않는다.
    pub const fn reserved_len(self) -> usize {
        match self {
            Self::U8Reserved => 1,
            Self::FourReserved | Self::FourReservedAlign4 => 4,
            Self::OpcodeOnly
            | Self::U16
            | Self::TwoU8
            | Self::TwoWords
            | Self::TwoU8Word
            | Self::TwoU8TwoWords => 0,
        }
    }
}

/// 소비 코드로 경계를 확정한 QUEST 제어코드.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestControlSpec {
    pub code: u8,
    pub handler: u32,
    pub layout: QuestControlLayout,
}

/// 하위 소비 코드까지 확인한 QUEST 제어코드 의미.
///
/// 이름은 장면 문맥 추정이 아니라 직접 관측한 상태 변경을 나타낸다.
/// 예를 들어 `FF03`은 interpreter를 비활성화하지만 이를 곧바로
/// "대사 블록 종료"라고 일반화하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestControlMeaning {
    /// interpreter 객체를 해제하고 현재 dispatch pass를 끝낸다.
    ReleaseInterpreterAndYield,
    /// stream만 전진시킨 뒤 현재 dispatch pass를 끝낸다.
    YieldInterpreterDispatch,
    /// text window가 inactive인 동안 u16 tick countdown을 기다린다.
    WaitTicksWhileWindowInactive,
    /// interpreter active word를 0으로 만들고 stream을 4바이트 정렬한다.
    DeactivateInterpreterAndAlign,
    /// 현재 text window의 cursor를 다음 행으로 옮긴다.
    AdvanceTextLine,
    /// 현재 text window를 비활성화하고 연결된 renderer를 해제한다.
    CloseTextWindow,
    /// 뒤따르는 제어와 glyph가 사용할 text window index를 바꾼다.
    SelectTextWindow,
    /// renderer의 현재·기준 fixed-point 위치를 함께 설정한다.
    SetRendererPosition,
    /// renderer의 x/y fixed-point scale을 설정한다.
    SetRendererScale,
    /// text window의 원점 좌표를 설정한다.
    SetWindowOrigin,
    /// glyph 뒤 x 간격과 다음 행 y 간격을 설정한다.
    SetGlyphAndLineSpacing,
    /// glyph 하나를 소비하기 전 기다릴 dispatch pass 수를 설정한다.
    SetGlyphDelay,
    /// glyph renderer가 GPU primitive에 기록할 texture page를 선택한다.
    SetGlyphTexturePage,
    /// glyph descriptor를 찾을 font resource index를 설정한다.
    SetFontResourceIndex,
    /// glyph primitive를 연결할 ordering-table depth를 설정한다.
    SetGlyphOrderingDepth,
    /// 입력 대기 상태를 시작하고 인자에 따라 indicator 객체를 만든다.
    StartInputWait,
    /// glyph의 가로·세로 cell 크기를 설정한다.
    SetGlyphCellSize,
    /// 두 u8 인자에 따라 text window flag를 set 또는 clear한다.
    UpdateWindowFlags,
    /// 현재 행 높이가 인자보다 작을 때만 인자로 올린다.
    RaiseLineHeight,
    /// 지정한 source에서 읽은 값을 decimal glyph로 표시한다.
    RenderDecimalValue,
    /// text window의 현재 cursor 좌표를 설정한다.
    SetCursorPosition,
    /// text window를 활성화하고 renderer가 없으면 생성한다.
    OpenTextWindow,
    /// menu 입력을 기다린 뒤 선택 index 또는 cancel 값을 interpreter에 저장한다.
    WaitMenuChoiceIndex,
    /// 인자 mode에 맞춰 text window renderer transition을 시작하거나 끝낸다.
    RunTextWindowTransition,
    /// 새 glyph 객체의 callback·초기 상태를 고르는 effect mode를 설정한다.
    SetGlyphEffectMode,
    /// controller 입력이 text fast-forward를 켜지 못하게 하는 전역 flag를 설정한다.
    SetButtonFastForwardLock,
    /// parent counter를 올리고 현재 interpreter를 해제한 뒤 dispatch를 끝낸다.
    NotifyParentAndReleaseInterpreter,
    /// index 0–8의 renderer 자산을 움직이는 객체로 생성한다.
    SpawnIndexedRendererAnimation,
    /// 현재 menu item index의 선택값을 기록하고 renderer +0x5C를 16 올린다.
    StoreMenuChoiceValue,
    /// menu 입력을 기다린 뒤 mapping을 적용한 선택값을 interpreter에 저장한다.
    WaitMappedMenuChoiceValue,
    /// mode 0–5가 고른 고정 text 또는 값을 현재 window에 렌더링한다.
    RenderFixedTextOrValue,
    /// 다른 상태를 바꾸지 않고 stream만 opcode 폭만큼 전진시킨다.
    AdvanceStreamOnly,
}

/// 소비 의미까지 확정한 제어코드 표.
pub const QUEST_CONFIRMED_CONTROL_MEANINGS: [(u8, QuestControlMeaning); 32] = [
    (0x00, QuestControlMeaning::ReleaseInterpreterAndYield),
    (0x01, QuestControlMeaning::YieldInterpreterDispatch),
    (0x02, QuestControlMeaning::WaitTicksWhileWindowInactive),
    (0x03, QuestControlMeaning::DeactivateInterpreterAndAlign),
    (0x04, QuestControlMeaning::AdvanceTextLine),
    (0x05, QuestControlMeaning::CloseTextWindow),
    (0x06, QuestControlMeaning::SelectTextWindow),
    (0x07, QuestControlMeaning::SetRendererPosition),
    (0x08, QuestControlMeaning::SetRendererScale),
    (0x09, QuestControlMeaning::SetWindowOrigin),
    (0x0A, QuestControlMeaning::SetGlyphAndLineSpacing),
    (0x0B, QuestControlMeaning::SetGlyphDelay),
    (0x0C, QuestControlMeaning::SetGlyphTexturePage),
    (0x0D, QuestControlMeaning::SetFontResourceIndex),
    (0x0E, QuestControlMeaning::SetGlyphOrderingDepth),
    (0x0F, QuestControlMeaning::StartInputWait),
    (0x10, QuestControlMeaning::SetGlyphCellSize),
    (0x11, QuestControlMeaning::UpdateWindowFlags),
    (0x12, QuestControlMeaning::RaiseLineHeight),
    (0x13, QuestControlMeaning::RenderDecimalValue),
    (0x14, QuestControlMeaning::SetCursorPosition),
    (0x15, QuestControlMeaning::OpenTextWindow),
    (0x16, QuestControlMeaning::WaitMenuChoiceIndex),
    (0x17, QuestControlMeaning::RunTextWindowTransition),
    (0x18, QuestControlMeaning::SetGlyphEffectMode),
    (0x19, QuestControlMeaning::SetButtonFastForwardLock),
    (0x1A, QuestControlMeaning::NotifyParentAndReleaseInterpreter),
    (0x1B, QuestControlMeaning::SpawnIndexedRendererAnimation),
    (0x1C, QuestControlMeaning::StoreMenuChoiceValue),
    (0x1D, QuestControlMeaning::WaitMappedMenuChoiceValue),
    (0x1E, QuestControlMeaning::RenderFixedTextOrValue),
    (0x1F, QuestControlMeaning::AdvanceStreamOnly),
];

/// 하위 소비 의미까지 확정한 경우에만 반환한다.
pub const fn quest_control_meaning(code: u8) -> Option<QuestControlMeaning> {
    match code {
        0x00 => Some(QuestControlMeaning::ReleaseInterpreterAndYield),
        0x01 => Some(QuestControlMeaning::YieldInterpreterDispatch),
        0x02 => Some(QuestControlMeaning::WaitTicksWhileWindowInactive),
        0x03 => Some(QuestControlMeaning::DeactivateInterpreterAndAlign),
        0x04 => Some(QuestControlMeaning::AdvanceTextLine),
        0x05 => Some(QuestControlMeaning::CloseTextWindow),
        0x06 => Some(QuestControlMeaning::SelectTextWindow),
        0x07 => Some(QuestControlMeaning::SetRendererPosition),
        0x08 => Some(QuestControlMeaning::SetRendererScale),
        0x09 => Some(QuestControlMeaning::SetWindowOrigin),
        0x0A => Some(QuestControlMeaning::SetGlyphAndLineSpacing),
        0x0B => Some(QuestControlMeaning::SetGlyphDelay),
        0x0C => Some(QuestControlMeaning::SetGlyphTexturePage),
        0x0D => Some(QuestControlMeaning::SetFontResourceIndex),
        0x0E => Some(QuestControlMeaning::SetGlyphOrderingDepth),
        0x0F => Some(QuestControlMeaning::StartInputWait),
        0x10 => Some(QuestControlMeaning::SetGlyphCellSize),
        0x11 => Some(QuestControlMeaning::UpdateWindowFlags),
        0x12 => Some(QuestControlMeaning::RaiseLineHeight),
        0x13 => Some(QuestControlMeaning::RenderDecimalValue),
        0x14 => Some(QuestControlMeaning::SetCursorPosition),
        0x15 => Some(QuestControlMeaning::OpenTextWindow),
        0x16 => Some(QuestControlMeaning::WaitMenuChoiceIndex),
        0x17 => Some(QuestControlMeaning::RunTextWindowTransition),
        0x18 => Some(QuestControlMeaning::SetGlyphEffectMode),
        0x19 => Some(QuestControlMeaning::SetButtonFastForwardLock),
        0x1A => Some(QuestControlMeaning::NotifyParentAndReleaseInterpreter),
        0x1B => Some(QuestControlMeaning::SpawnIndexedRendererAnimation),
        0x1C => Some(QuestControlMeaning::StoreMenuChoiceValue),
        0x1D => Some(QuestControlMeaning::WaitMappedMenuChoiceValue),
        0x1E => Some(QuestControlMeaning::RenderFixedTextOrValue),
        0x1F => Some(QuestControlMeaning::AdvanceStreamOnly),
        _ => None,
    }
}

/// 경계를 확정한 QUEST stream token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestToken<'a> {
    /// 소비 루프가 그대로 글리프 lookup에 넘기는 2바이트 코드.
    Glyph(&'a [u8]),
    /// 확인된 handler layout에 따라 경계를 정한 제어코드.
    Control {
        spec: QuestControlSpec,
        raw: &'a [u8],
    },
}

impl<'a> QuestToken<'a> {
    /// reserved와 alignment padding을 포함한 원본 바이트.
    pub const fn raw(&self) -> &'a [u8] {
        match self {
            Self::Glyph(raw) | Self::Control { raw, .. } => raw,
        }
    }
}

/// QUEST token 파싱 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestTokenError {
    MisalignedAddress(u32),
    TruncatedOpcode {
        address: u32,
        available: usize,
    },
    UnknownControl {
        address: u32,
        code: u8,
    },
    TruncatedControl {
        address: u32,
        code: u8,
        expected: usize,
        available: usize,
    },
    AddressOverflow(u32),
}

impl fmt::Display for QuestTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MisalignedAddress(address) => {
                write!(f, "QUEST token 주소가 u16 정렬이 아님: 0x{address:08X}")
            }
            Self::TruncatedOpcode { address, available } => write!(
                f,
                "QUEST token opcode 부족: address=0x{address:08X}, available={available}"
            ),
            Self::UnknownControl { address, code } => write!(
                f,
                "QUEST 미분류 제어코드: address=0x{address:08X}, code=FF{code:02X}"
            ),
            Self::TruncatedControl {
                address,
                code,
                expected,
                available,
            } => write!(
                f,
                "QUEST 제어코드 부족: address=0x{address:08X}, code=FF{code:02X}, \
                 expected={expected}, available={available}"
            ),
            Self::AddressOverflow(address) => {
                write!(f, "QUEST token 주소 overflow: 0x{address:08X}")
            }
        }
    }
}

impl Error for QuestTokenError {}

/// 현재 소비 코드로 경계를 확정한 제어코드만 반환한다.
///
/// dispatch table에 존재한다는 사실만으로 인자 폭을 추정하지 않는다. 아직
/// 분류하지 않은 코드는 `None`으로 실패 폐쇄한다.
pub const fn quest_control_spec(code: u8) -> Option<QuestControlSpec> {
    let (handler, layout) = match code {
        0x00 => (0x8003_DDF0, QuestControlLayout::FourReserved),
        0x01 => (0x8003_DE24, QuestControlLayout::OpcodeOnly),
        0x02 => (0x8003_DE40, QuestControlLayout::U16),
        0x03 => (0x8003_DEE8, QuestControlLayout::FourReservedAlign4),
        0x04 => (0x8003_BA14, QuestControlLayout::OpcodeOnly),
        0x05 => (0x8003_DF04, QuestControlLayout::OpcodeOnly),
        0x06 => (0x8003_DF40, QuestControlLayout::U8Reserved),
        0x07 => (0x8003_BB30, QuestControlLayout::TwoWords),
        0x08 => (0x8003_DF64, QuestControlLayout::TwoWords),
        0x09 => (0x8003_DFF0, QuestControlLayout::TwoWords),
        0x0A => (0x8003_E06C, QuestControlLayout::TwoU8),
        0x0B => (0x8003_E0E8, QuestControlLayout::U8Reserved),
        0x0C => (0x8003_E134, QuestControlLayout::U8Reserved),
        0x0D => (0x8003_E180, QuestControlLayout::U8Reserved),
        0x0E => (0x8003_E1CC, QuestControlLayout::U8Reserved),
        0x0F => (0x8003_E218, QuestControlLayout::U8Reserved),
        0x10 => (0x8003_BC18, QuestControlLayout::TwoU8),
        0x11 => (0x8003_E264, QuestControlLayout::TwoU8),
        0x12 => (0x8003_E308, QuestControlLayout::U8Reserved),
        0x13 => (0x8003_E36C, QuestControlLayout::TwoU8Word),
        0x14 => (0x8003_E450, QuestControlLayout::TwoWords),
        0x15 => (0x8003_E4CC, QuestControlLayout::OpcodeOnly),
        0x16 => (0x8003_BCFC, QuestControlLayout::U8Reserved),
        0x17 => (0x8003_BEE0, QuestControlLayout::U8Reserved),
        0x18 => (0x8003_E580, QuestControlLayout::U8Reserved),
        0x19 => (0x8003_E5CC, QuestControlLayout::U8Reserved),
        0x1A => (0x8003_E5F4, QuestControlLayout::FourReserved),
        0x1B => (0x8003_C23C, QuestControlLayout::TwoU8TwoWords),
        0x1C => (0x8003_E640, QuestControlLayout::U8Reserved),
        0x1D => (0x8003_C490, QuestControlLayout::TwoU8),
        0x1E => (0x8003_C670, QuestControlLayout::U8Reserved),
        0x1F => (0x8003_DDDC, QuestControlLayout::OpcodeOnly),
        _ => return None,
    };
    Some(QuestControlSpec {
        code,
        handler,
        layout,
    })
}

pub const QUEST_CONTROL_DISPATCH_RUNTIME: u32 = 0x800B_2998;
pub const QUEST_CONTROL_COUNT: usize = 0x20;
pub const QUEST_CONTROL_DISPATCH_BYTES: usize = QUEST_CONTROL_COUNT * 4;

/// 원본 dispatch table과 Rust control 선언의 대조 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestControlDispatchReport {
    pub controls: usize,
    pub unique_handlers: usize,
}

/// QUEST control dispatch 대조 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestControlDispatchError {
    WrongLength {
        expected: usize,
        actual: usize,
    },
    MissingSpec {
        code: u8,
    },
    HandlerMismatch {
        code: u8,
        expected: u32,
        actual: u32,
    },
    DuplicateHandler {
        first_code: u8,
        second_code: u8,
        handler: u32,
    },
}

impl fmt::Display for QuestControlDispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { expected, actual } => write!(
                f,
                "QUEST dispatch 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingSpec { code } => {
                write!(f, "QUEST dispatch Rust spec 누락: code=FF{code:02X}")
            }
            Self::HandlerMismatch {
                code,
                expected,
                actual,
            } => write!(
                f,
                "QUEST dispatch handler 불일치: code=FF{code:02X}, \
                 expected=0x{expected:08X}, actual=0x{actual:08X}"
            ),
            Self::DuplicateHandler {
                first_code,
                second_code,
                handler,
            } => write!(
                f,
                "QUEST dispatch handler 중복: FF{first_code:02X}/FF{second_code:02X} \
                 -> 0x{handler:08X}"
            ),
        }
    }
}

impl Error for QuestControlDispatchError {}

/// 정확히 32개인 원본 dispatch table을 Rust spec과 전수 대조한다.
pub fn verify_quest_control_dispatch(
    raw: &[u8],
) -> Result<QuestControlDispatchReport, QuestControlDispatchError> {
    if raw.len() != QUEST_CONTROL_DISPATCH_BYTES {
        return Err(QuestControlDispatchError::WrongLength {
            expected: QUEST_CONTROL_DISPATCH_BYTES,
            actual: raw.len(),
        });
    }

    let mut handlers = std::collections::BTreeMap::<u32, u8>::new();
    for (index, word) in raw.chunks_exact(4).enumerate() {
        let code = u8::try_from(index).expect("QUEST control count는 u8 범위");
        let spec =
            quest_control_spec(code).ok_or(QuestControlDispatchError::MissingSpec { code })?;
        let actual = u32::from_le_bytes(word.try_into().expect("4바이트 dispatch word"));
        if actual != spec.handler {
            return Err(QuestControlDispatchError::HandlerMismatch {
                code,
                expected: spec.handler,
                actual,
            });
        }
        if let Some(first_code) = handlers.insert(actual, code) {
            return Err(QuestControlDispatchError::DuplicateHandler {
                first_code,
                second_code: code,
                handler: actual,
            });
        }
    }
    Ok(QuestControlDispatchReport {
        controls: QUEST_CONTROL_COUNT,
        unique_handlers: handlers.len(),
    })
}

/// 한 token을 읽는다.
///
/// 소비 루프와 같이 `0xFEFF`보다 큰 u16 LE 값만 제어코드로 본다. 미분류
/// `FFxx`의 폭을 추정하지 않고 실패하며, 성공한 token의 `raw()`는
/// reserved·padding까지 원형 그대로 반환한다.
pub fn parse_quest_token(
    input: &[u8],
    stream_address: u32,
) -> Result<QuestToken<'_>, QuestTokenError> {
    if !stream_address.is_multiple_of(2) {
        return Err(QuestTokenError::MisalignedAddress(stream_address));
    }
    if input.len() < 2 {
        return Err(QuestTokenError::TruncatedOpcode {
            address: stream_address,
            available: input.len(),
        });
    }
    if u16::from_le_bytes([input[0], input[1]]) <= 0xFEFF {
        return Ok(QuestToken::Glyph(&input[..2]));
    }

    let code = input[0];
    let spec = quest_control_spec(code).ok_or(QuestTokenError::UnknownControl {
        address: stream_address,
        code,
    })?;
    let expected = spec
        .layout
        .encoded_len(stream_address)
        .ok_or(QuestTokenError::AddressOverflow(stream_address))?;
    if input.len() < expected {
        return Err(QuestTokenError::TruncatedControl {
            address: stream_address,
            code,
            expected,
            available: input.len(),
        });
    }
    Ok(QuestToken::Control {
        spec,
        raw: &input[..expected],
    })
}

/// 주어진 범위 전체를 token으로 읽는다.
///
/// 시작 주소부터 입력 끝까지 빈틈없이 소비해야 성공한다. 아직 분류하지 않은
/// control이나 잘린 마지막 token이 있으면 부분 결과를 반환하지 않는다.
pub fn parse_quest_tokens(
    input: &[u8],
    stream_address: u32,
) -> Result<Vec<QuestToken<'_>>, QuestTokenError> {
    let mut tokens = Vec::new();
    let mut consumed = 0usize;
    while consumed < input.len() {
        let address_delta = u32::try_from(consumed)
            .map_err(|_| QuestTokenError::AddressOverflow(stream_address))?;
        let address = stream_address
            .checked_add(address_delta)
            .ok_or(QuestTokenError::AddressOverflow(stream_address))?;
        let token = parse_quest_token(&input[consumed..], address)?;
        consumed = consumed
            .checked_add(token.raw().len())
            .ok_or(QuestTokenError::AddressOverflow(stream_address))?;
        tokens.push(token);
    }
    Ok(tokens)
}

#[cfg(test)]
#[path = "quest_tests.rs"]
mod tests;
