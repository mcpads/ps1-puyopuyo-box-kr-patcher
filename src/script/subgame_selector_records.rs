//! 서브게임 descriptor를 고르는 알려진 record 형식.
//!
//! 이 모듈은 byte layout만 해석한다. 실제 parser 도달, record 정렬,
//! descriptor table 종류와 runtime 소비 여부는 각 호출자가 별도로 판정한다.

/// 현재 소비 코드에서 복원한 type/resource selector 형식.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubgameSelectorRecordForm {
    Op28Sub01,
    Op28Sub03,
    Op28Sub04,
    Op28Sub10,
    Op28Sub12,
    MainOp39,
    MainOpC3,
}

impl SubgameSelectorRecordForm {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Op28Sub01 => "op28-sub01",
            Self::Op28Sub03 => "op28-sub03",
            Self::Op28Sub04 => "op28-sub04",
            Self::Op28Sub10 => "op28-sub10",
            Self::Op28Sub12 => "op28-sub12",
            Self::MainOp39 => "main-op39",
            Self::MainOpC3 => "main-opc3",
        }
    }
}

/// 알려진 형식으로 해석되는 type/resource selector 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSelectorRecord {
    pub offset: usize,
    pub form: SubgameSelectorRecordForm,
    /// OP28 형식에만 있는 flags. main opcode의 다른 인자를 flags로 오인하지 않는다.
    pub flags: Option<u8>,
    pub record_type: u8,
    pub resource_id: u16,
}

/// byte stream에서 알려진 일곱 selector 형식을 모두 열거한다.
///
/// byte 단위 후보를 보존하므로 정렬 조건이 필요한 호출자는 `offset`을 별도로
/// 검증해야 한다. 이 함수 자체의 양성은 실행 가능한 record라는 뜻이 아니다.
pub fn scan_subgame_selector_records(raw: &[u8]) -> Vec<SubgameSelectorRecord> {
    raw.windows(8)
        .enumerate()
        .filter_map(|(offset, bytes)| {
            let (form, flags, record_type, resource_id) = match (bytes[0], bytes[1]) {
                (0x28, 0x01) => (
                    SubgameSelectorRecordForm::Op28Sub01,
                    Some(bytes[3]),
                    bytes[2],
                    u16::from_le_bytes([bytes[4], bytes[5]]),
                ),
                (0x28, 0x03) => (
                    SubgameSelectorRecordForm::Op28Sub03,
                    Some(bytes[3]),
                    bytes[2],
                    u16::from_le_bytes([bytes[4], bytes[5]]),
                ),
                (0x28, 0x04) => (
                    SubgameSelectorRecordForm::Op28Sub04,
                    Some(bytes[3]),
                    bytes[2],
                    u16::from_le_bytes([bytes[4], bytes[5]]),
                ),
                (0x28, 0x10) => (
                    SubgameSelectorRecordForm::Op28Sub10,
                    Some(bytes[2]),
                    bytes[3],
                    u16::from_le_bytes([bytes[4], bytes[5]]),
                ),
                (0x28, 0x12) => (
                    SubgameSelectorRecordForm::Op28Sub12,
                    Some(bytes[2]),
                    bytes[3],
                    u16::from_le_bytes([bytes[4], bytes[5]]),
                ),
                (0x39, _) => (
                    SubgameSelectorRecordForm::MainOp39,
                    None,
                    bytes[1],
                    u16::from_le_bytes([bytes[2], bytes[3]]),
                ),
                (0xC3, _) => (
                    SubgameSelectorRecordForm::MainOpC3,
                    None,
                    bytes[3],
                    u16::from_le_bytes([bytes[4], bytes[5]]),
                ),
                _ => return None,
            };
            Some(SubgameSelectorRecord {
                offset,
                form,
                flags,
                record_type,
                resource_id,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "subgame_selector_records_tests.rs"]
mod tests;
