//! `TOWN00.SEQ`에서 확인한 구조 경계와 무변경 왕복.
//!
//! 선두 `0x0000..0x0140`의 20바이트 OP85 레코드 16개와 본문 시작
//! `0x0140`의 OP03 direct branch 레코드를 구조로 승인한다. `QUEST.EXE`
//! 소비 코드로 확인한 필드만 표현하며, 나머지 본문의 내부 참조 규칙은
//! 아직 확정하지 않았으므로 원본을 보존한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::RuntimeAssetMap;

use super::{parse_quest_tokens, QuestToken, QuestTokenError};

pub const QUEST_SEQ_ACTIVE_RUNTIME_BASE: u32 = 0x8019_9A70;
pub const TOWN00_SEQ_RUNTIME_BASE: u32 = QUEST_SEQ_ACTIVE_RUNTIME_BASE;
pub const TOWN00_SEQ_LEN: usize = 0x5764;
pub const TOWN00_SEQ_SHA256: &str =
    "e9fc26b5d4aa08f5f041aed35ef6d52ca8b7460a66d0c3a302117bd21b113cea";
pub const TOWN00_PREFIX_RECORDS: usize = 16;
pub const QUEST_SEQ_PREFIX_RECORD_LEN: usize = 20;
pub const TOWN00_ROOT_STREAM_OFFSET: usize = 0x140;
pub const TOWN00_ROOT_TARGET_OFFSET: usize = 0x148;
pub const QUEST_SEQ_DIRECT_BRANCH_RECORD_LEN: usize = 8;
pub const QUEST_SEQ_GLOBAL_WORD_BRANCH_HEADER_LEN: usize = 4;
pub const QUEST_SEQ_OP88_RECORD_LEN: usize = 8;
pub const QUEST_SEQ_OP88_MAX_TABLE_ENTRIES: u8 = 32;
pub const QUEST_SEQ_OP8F_RECORD_LEN: usize = 8;
pub const QUEST_SEQ_OP8F_MAX_TABLE_ENTRIES: u8 = 16;

const PREFIX_MARKER: u8 = 0x85;
const DIRECT_BRANCH_OPCODE: u8 = 0x03;
const GLOBAL_WORD_BRANCH_OPCODE: u8 = 0x0D;
const TABLE_LOAD_OPCODE: u8 = 0x88;
const HALFWORD_PAIR_TABLE_LOAD_OPCODE: u8 = 0x8F;
const PREFIX_TRAILING_ZEROES: usize = 8;
const FIRST_DIALOGUE_START: usize = 0x21E4;
const FIRST_DIALOGUE_END: usize = 0x2280;
const EXPECTED_INTERNAL_ADDRESS_FIELDS: usize = 167;
const EXPECTED_INTERNAL_ADDRESS_TARGETS: usize = 122;
const EXPECTED_SAMPLE_GLYPHS: usize = 48;
const EXPECTED_SAMPLE_CONTROLS: usize = 17;

/// runtime breakpoint에서 실제 32-bit 소비를 확인한 내부주소 field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Town00ConfirmedPointerField {
    pub field_offset: usize,
    pub target_offset: usize,
    pub consumer_runtime: u32,
}

pub const TOWN00_CONFIRMED_POINTER_FIELDS: [Town00ConfirmedPointerField; 7] = [
    Town00ConfirmedPointerField {
        field_offset: 0x014C,
        target_offset: 0x015C,
        consumer_runtime: 0x8002_2970,
    },
    Town00ConfirmedPointerField {
        field_offset: 0x0168,
        target_offset: 0x51AC,
        consumer_runtime: 0x8007_832C,
    },
    Town00ConfirmedPointerField {
        field_offset: 0x016C,
        target_offset: 0x03C0,
        consumer_runtime: 0x8007_8340,
    },
    Town00ConfirmedPointerField {
        field_offset: 0x0174,
        target_offset: 0x0354,
        consumer_runtime: 0x8007_83E8,
    },
    Town00ConfirmedPointerField {
        field_offset: 0x017C,
        target_offset: 0x034C,
        consumer_runtime: 0x8007_8564,
    },
    Town00ConfirmedPointerField {
        field_offset: 0x1B34,
        target_offset: 0x1B28,
        consumer_runtime: 0x8007_8A80,
    },
    Town00ConfirmedPointerField {
        field_offset: 0x3618,
        target_offset: 0x360C,
        consumer_runtime: 0x8007_8A80,
    },
];

/// `TOWN00.SEQ` 안의 정렬된 runtime 주소 후보 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Town00InternalAddressCandidate {
    pub field_offset: usize,
    pub field_runtime: u32,
    pub target_offset: usize,
    pub target_runtime: u32,
    pub target_references: usize,
    /// `None`은 미소비가 아니라 현재 bounded trace에서 미관측이라는 뜻이다.
    pub runtime_consumer: Option<u32>,
}

impl Town00InternalAddressCandidate {
    fn direction(self) -> &'static str {
        match self.target_offset.cmp(&self.field_offset) {
            std::cmp::Ordering::Less => "backward",
            std::cmp::Ordering::Equal => "self",
            std::cmp::Ordering::Greater => "forward",
        }
    }

    fn evidence(self) -> &'static str {
        if self.runtime_consumer.is_some() {
            "runtime_read"
        } else {
            "range_candidate"
        }
    }
}

/// OP85가 소비하는 일반 20바이트 레코드.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqOp85Record {
    marker: u8,
    seq_resource_id: u8,
    map_resource_ids: [u16; 3],
    grp_resource_ids: [u16; 2],
    renderer_data_pointers: [u32; 2],
}

impl QuestSeqOp85Record {
    fn parse(raw: &[u8], stream_offset: usize) -> Result<Self, QuestSeqError> {
        if raw.len() != QUEST_SEQ_PREFIX_RECORD_LEN {
            return Err(QuestSeqError::TruncatedOp85Record {
                stream_offset,
                available: raw.len(),
            });
        }
        if raw[0] != PREFIX_MARKER {
            return Err(QuestSeqError::InvalidOp85Opcode {
                stream_offset,
                actual: raw[0],
            });
        }

        let mut resource_ids = [0u16; 5];
        for (value, raw_pair) in resource_ids.iter_mut().zip(raw[2..12].chunks_exact(2)) {
            *value =
                u16::from_le_bytes(raw_pair.try_into().expect("2바이트 halfword 범위를 확인함"));
        }
        let renderer_data_pointers = [
            u32::from_le_bytes(
                raw[12..16]
                    .try_into()
                    .expect("4바이트 pointer 범위를 확인함"),
            ),
            u32::from_le_bytes(
                raw[16..20]
                    .try_into()
                    .expect("4바이트 pointer 범위를 확인함"),
            ),
        ];
        Ok(Self {
            marker: raw[0],
            seq_resource_id: raw[1],
            map_resource_ids: resource_ids[..3]
                .try_into()
                .expect("MAP resource ID 3개 범위를 확인함"),
            grp_resource_ids: resource_ids[3..]
                .try_into()
                .expect("GRP resource ID 2개 범위를 확인함"),
            renderer_data_pointers,
        })
    }

    fn encode(self) -> [u8; QUEST_SEQ_PREFIX_RECORD_LEN] {
        let mut raw = [0u8; QUEST_SEQ_PREFIX_RECORD_LEN];
        raw[0] = self.marker;
        raw[1] = self.seq_resource_id;
        for (raw_pair, value) in raw[2..8].chunks_exact_mut(2).zip(self.map_resource_ids) {
            raw_pair.copy_from_slice(&value.to_le_bytes());
        }
        for (raw_pair, value) in raw[8..12].chunks_exact_mut(2).zip(self.grp_resource_ids) {
            raw_pair.copy_from_slice(&value.to_le_bytes());
        }
        raw[12..16].copy_from_slice(&self.renderer_data_pointers[0].to_le_bytes());
        raw[16..20].copy_from_slice(&self.renderer_data_pointers[1].to_le_bytes());
        raw
    }

    pub const fn seq_resource_id(self) -> u8 {
        self.seq_resource_id
    }

    pub const fn map_resource_ids(self) -> [u16; 3] {
        self.map_resource_ids
    }

    pub const fn grp_resource_ids(self) -> [u16; 2] {
        self.grp_resource_ids
    }

    pub const fn renderer_data_pointers(self) -> [u32; 2] {
        self.renderer_data_pointers
    }
}

/// OP88이 소비하는 8바이트 pointer table load 레코드.
///
/// handler는 `+1`을 32 이하로 clamp한 entry 수로 쓰고 `+4`의 pointer를
/// base로 삼아 u32 entry를 순서대로 역참조한다. `+2..+3`은 현재 확인한
/// 소비 코드가 읽지 않으므로 의미를 붙이지 않고 그대로 보존한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqOp88Record {
    opcode: u8,
    raw_entry_count: u8,
    unconsumed_parameters: [u8; 2],
    table_runtime: u32,
}

impl QuestSeqOp88Record {
    fn parse(raw: &[u8], stream_offset: usize) -> Result<Self, QuestSeqError> {
        if raw.len() != QUEST_SEQ_OP88_RECORD_LEN {
            return Err(QuestSeqError::TruncatedOp88Record {
                stream_offset,
                available: raw.len(),
            });
        }
        if raw[0] != TABLE_LOAD_OPCODE {
            return Err(QuestSeqError::InvalidOp88Opcode {
                stream_offset,
                actual: raw[0],
            });
        }
        Ok(Self {
            opcode: raw[0],
            raw_entry_count: raw[1],
            unconsumed_parameters: raw[2..4]
                .try_into()
                .expect("OP88 미소비 2바이트 범위를 확인함"),
            table_runtime: u32::from_le_bytes(
                raw[4..8]
                    .try_into()
                    .expect("OP88 pointer 4바이트 범위를 확인함"),
            ),
        })
    }

    pub fn encode(self) -> [u8; QUEST_SEQ_OP88_RECORD_LEN] {
        let mut raw = [0u8; QUEST_SEQ_OP88_RECORD_LEN];
        raw[0] = self.opcode;
        raw[1] = self.raw_entry_count;
        raw[2..4].copy_from_slice(&self.unconsumed_parameters);
        raw[4..8].copy_from_slice(&self.table_runtime.to_le_bytes());
        raw
    }

    pub const fn raw_entry_count(self) -> u8 {
        self.raw_entry_count
    }

    pub const fn effective_entry_count(self) -> u8 {
        if self.raw_entry_count > QUEST_SEQ_OP88_MAX_TABLE_ENTRIES {
            QUEST_SEQ_OP88_MAX_TABLE_ENTRIES
        } else {
            self.raw_entry_count
        }
    }

    pub const fn unconsumed_parameters(self) -> [u8; 2] {
        self.unconsumed_parameters
    }

    pub const fn table_runtime(self) -> u32 {
        self.table_runtime
    }
}

/// OP8F가 소비하는 8바이트 halfword pair table load 레코드.
///
/// handler는 `+1`을 16 이하로 clamp한 entry 수로 쓰고 `+4`의 pointer를
/// base로 삼아 entry마다 u16 두 개를 역참조한다. `+2..+3`은 현재 확인한
/// 소비 코드가 읽지 않으므로 원본 byte를 보존한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqOp8fRecord {
    opcode: u8,
    raw_entry_count: u8,
    unconsumed_parameters: [u8; 2],
    table_runtime: u32,
}

impl QuestSeqOp8fRecord {
    fn parse(raw: &[u8], stream_offset: usize) -> Result<Self, QuestSeqError> {
        if raw.len() != QUEST_SEQ_OP8F_RECORD_LEN {
            return Err(QuestSeqError::TruncatedOp8fRecord {
                stream_offset,
                available: raw.len(),
            });
        }
        if raw[0] != HALFWORD_PAIR_TABLE_LOAD_OPCODE {
            return Err(QuestSeqError::InvalidOp8fOpcode {
                stream_offset,
                actual: raw[0],
            });
        }
        Ok(Self {
            opcode: raw[0],
            raw_entry_count: raw[1],
            unconsumed_parameters: raw[2..4]
                .try_into()
                .expect("OP8F 미소비 2바이트 범위를 확인함"),
            table_runtime: u32::from_le_bytes(
                raw[4..8]
                    .try_into()
                    .expect("OP8F pointer 4바이트 범위를 확인함"),
            ),
        })
    }

    pub fn encode(self) -> [u8; QUEST_SEQ_OP8F_RECORD_LEN] {
        let mut raw = [0u8; QUEST_SEQ_OP8F_RECORD_LEN];
        raw[0] = self.opcode;
        raw[1] = self.raw_entry_count;
        raw[2..4].copy_from_slice(&self.unconsumed_parameters);
        raw[4..8].copy_from_slice(&self.table_runtime.to_le_bytes());
        raw
    }

    pub const fn raw_entry_count(self) -> u8 {
        self.raw_entry_count
    }

    pub const fn effective_entry_count(self) -> u8 {
        if self.raw_entry_count > QUEST_SEQ_OP8F_MAX_TABLE_ENTRIES {
            QUEST_SEQ_OP8F_MAX_TABLE_ENTRIES
        } else {
            self.raw_entry_count
        }
    }

    pub const fn unconsumed_parameters(self) -> [u8; 2] {
        self.unconsumed_parameters
    }

    pub const fn table_runtime(self) -> u32 {
        self.table_runtime
    }
}

/// 선두 16개 OP85 레코드.
///
/// 일반 OP85와 같은 소비 필드를 가지지만 지원 원본의 이 배열은
/// renderer pointer 두 개가 모두 0이어야 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqPrefixRecord {
    record: QuestSeqOp85Record,
}

impl QuestSeqPrefixRecord {
    fn parse(raw: &[u8], index: usize) -> Result<Self, QuestSeqError> {
        if raw.len() != QUEST_SEQ_PREFIX_RECORD_LEN {
            return Err(QuestSeqError::TruncatedPrefixRecord {
                index,
                available: raw.len(),
            });
        }
        if raw[0] != PREFIX_MARKER {
            return Err(QuestSeqError::InvalidPrefixMarker {
                index,
                actual: raw[0],
            });
        }
        let trailing_offset = QUEST_SEQ_PREFIX_RECORD_LEN - PREFIX_TRAILING_ZEROES;
        if let Some((relative_offset, actual)) = raw[trailing_offset..]
            .iter()
            .copied()
            .enumerate()
            .find(|(_, byte)| *byte != 0)
        {
            return Err(QuestSeqError::NonZeroPrefixTail {
                index,
                offset: trailing_offset + relative_offset,
                actual,
            });
        }
        Ok(Self {
            record: QuestSeqOp85Record::parse(raw, index * QUEST_SEQ_PREFIX_RECORD_LEN)?,
        })
    }

    fn encode(self) -> [u8; QUEST_SEQ_PREFIX_RECORD_LEN] {
        self.record.encode()
    }

    pub const fn seq_resource_id(self) -> u8 {
        self.record.seq_resource_id()
    }

    pub const fn map_resource_ids(self) -> [u16; 3] {
        self.record.map_resource_ids()
    }

    pub const fn grp_resource_ids(self) -> [u16; 2] {
        self.record.grp_resource_ids()
    }

    pub const fn renderer_data_pointers(self) -> [u32; 2] {
        self.record.renderer_data_pointers()
    }
}

/// OP03이 소비하는 8바이트 direct branch 레코드.
///
/// handler는 opcode와 `+4` u32 pointer만 소비한다. `+1..+3`은 의미를
/// 붙이지 않고 그대로 보존한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqDirectBranchRecord {
    opcode: u8,
    unconsumed_parameters: [u8; 3],
    target_runtime: u32,
    target_offset: usize,
}

impl QuestSeqDirectBranchRecord {
    fn parse(raw: &[u8], address_map: RuntimeAssetMap) -> Result<Self, QuestSeqError> {
        if raw.len() != QUEST_SEQ_DIRECT_BRANCH_RECORD_LEN {
            return Err(QuestSeqError::TruncatedRootRecord {
                available: raw.len(),
            });
        }
        if raw[0] != DIRECT_BRANCH_OPCODE {
            return Err(QuestSeqError::InvalidRootOpcode { actual: raw[0] });
        }

        let target_runtime = u32::from_le_bytes(
            raw[4..8]
                .try_into()
                .expect("OP03 pointer의 4바이트 범위를 확인함"),
        );
        let target_offset =
            address_map
                .file_offset(target_runtime)
                .ok_or(QuestSeqError::RootTargetOutOfRange {
                    target_runtime,
                    base: address_map.base,
                    len: address_map.len,
                })?;
        Ok(Self {
            opcode: raw[0],
            unconsumed_parameters: raw[1..4]
                .try_into()
                .expect("OP03 미소비 3바이트 범위를 확인함"),
            target_runtime,
            target_offset,
        })
    }

    fn encode(self) -> [u8; QUEST_SEQ_DIRECT_BRANCH_RECORD_LEN] {
        let mut raw = [0u8; QUEST_SEQ_DIRECT_BRANCH_RECORD_LEN];
        raw[0] = self.opcode;
        raw[1..4].copy_from_slice(&self.unconsumed_parameters);
        raw[4..8].copy_from_slice(&self.target_runtime.to_le_bytes());
        raw
    }

    pub const fn unconsumed_parameters(self) -> [u8; 3] {
        self.unconsumed_parameters
    }

    pub const fn target_runtime(self) -> u32 {
        self.target_runtime
    }

    pub const fn target_offset(self) -> usize {
        self.target_offset
    }
}

/// OP0D의 고정 4바이트 header.
///
/// `+1`은 `0x800DB6C0`의 word index다. 그 word 값이 inline pointer
/// table의 index가 되며 handler에는 table 길이 검사가 없다. 따라서
/// `+2..+3`을 count로 추정하거나 전체 record 길이를 선언하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqGlobalWordBranchHeader {
    opcode: u8,
    global_state_word_index: u8,
    unconsumed_parameters: [u8; 2],
}

impl QuestSeqGlobalWordBranchHeader {
    fn parse(raw: &[u8], stream_offset: usize) -> Result<Self, QuestSeqError> {
        if raw.len() != QUEST_SEQ_GLOBAL_WORD_BRANCH_HEADER_LEN {
            return Err(QuestSeqError::TruncatedGlobalWordBranchHeader {
                stream_offset,
                available: raw.len(),
            });
        }
        if raw[0] != GLOBAL_WORD_BRANCH_OPCODE {
            return Err(QuestSeqError::InvalidGlobalWordBranchOpcode {
                stream_offset,
                actual: raw[0],
            });
        }
        Ok(Self {
            opcode: raw[0],
            global_state_word_index: raw[1],
            unconsumed_parameters: raw[2..4]
                .try_into()
                .expect("OP0D 미소비 2바이트 범위를 확인함"),
        })
    }

    fn encode(self) -> [u8; QUEST_SEQ_GLOBAL_WORD_BRANCH_HEADER_LEN] {
        let mut raw = [0u8; QUEST_SEQ_GLOBAL_WORD_BRANCH_HEADER_LEN];
        raw[0] = self.opcode;
        raw[1] = self.global_state_word_index;
        raw[2..4].copy_from_slice(&self.unconsumed_parameters);
        raw
    }

    pub const fn global_state_word_index(self) -> u8 {
        self.global_state_word_index
    }

    pub const fn unconsumed_parameters(self) -> [u8; 2] {
        self.unconsumed_parameters
    }
}

/// 실행 시 OP0D branch index 하나로 선택되는 inline pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqResolvedBranchTarget {
    pub branch_index: u32,
    pub pointer_field_offset: usize,
    pub target_runtime: u32,
    pub target_offset: usize,
}

/// runtime index로 도달한 `TOWN00` 본문 OP85 node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Town00Op85Node {
    pub stream_offset: usize,
    pub next_stream_offset: usize,
    pub record: QuestSeqOp85Record,
    pub renderer_target_offsets: [Option<usize>; 2],
}

/// root index 경로에서 OP85 다음에 순차 도달하는 OP88 node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Town00Op88Node {
    pub stream_offset: usize,
    pub next_stream_offset: usize,
    pub record: QuestSeqOp88Record,
    pub table_target_offset: usize,
    pub table_end_offset: usize,
}

/// root index 경로에서 OP88 다음에 순차 도달하는 OP8F node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Town00Op8fNode {
    pub stream_offset: usize,
    pub next_stream_offset: usize,
    pub record: QuestSeqOp8fRecord,
    pub table_target_offset: usize,
    pub table_end_offset: usize,
}

/// 구조로 확정한 선두와 불투명 본문을 분리한 `TOWN00.SEQ`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Town00Seq<'a> {
    prefix_records: Vec<QuestSeqPrefixRecord>,
    opaque_body: &'a [u8],
}

impl<'a> Town00Seq<'a> {
    /// 지원 원본 `TOWN00.SEQ`를 확인된 경계만으로 파싱한다.
    pub fn parse_source(raw: &'a [u8]) -> Result<Self, QuestSeqError> {
        if raw.len() != TOWN00_SEQ_LEN {
            return Err(QuestSeqError::SourceLength {
                expected: TOWN00_SEQ_LEN,
                actual: raw.len(),
            });
        }
        let actual_sha256 = format!("{:x}", Sha256::digest(raw));
        if actual_sha256 != TOWN00_SEQ_SHA256 {
            return Err(QuestSeqError::SourceDigest {
                expected: TOWN00_SEQ_SHA256,
                actual: actual_sha256,
            });
        }

        let prefix_len = TOWN00_PREFIX_RECORDS
            .checked_mul(QUEST_SEQ_PREFIX_RECORD_LEN)
            .ok_or(QuestSeqError::PrefixLengthOverflow)?;
        let prefix = raw
            .get(..prefix_len)
            .ok_or(QuestSeqError::TruncatedPrefixRecord {
                index: raw.len() / QUEST_SEQ_PREFIX_RECORD_LEN,
                available: raw.len() % QUEST_SEQ_PREFIX_RECORD_LEN,
            })?;
        let mut prefix_records = Vec::with_capacity(TOWN00_PREFIX_RECORDS);
        for (index, record) in prefix.chunks_exact(QUEST_SEQ_PREFIX_RECORD_LEN).enumerate() {
            prefix_records.push(QuestSeqPrefixRecord::parse(record, index)?);
        }

        let opaque_body = &raw[prefix_len..];
        if looks_like_prefix_record(opaque_body) {
            return Err(QuestSeqError::PrefixBoundaryContinues { offset: prefix_len });
        }
        Ok(Self {
            prefix_records,
            opaque_body,
        })
    }

    pub fn prefix_records(&self) -> &[QuestSeqPrefixRecord] {
        &self.prefix_records
    }

    pub const fn opaque_body(&self) -> &'a [u8] {
        self.opaque_body
    }

    /// 본문 첫 OP03 레코드를 검증된 runtime/file 좌표로 읽는다.
    pub fn root_direct_branch(&self) -> Result<QuestSeqDirectBranchRecord, QuestSeqError> {
        let address_map = town00_address_map()?;
        let raw = self
            .opaque_body
            .get(..QUEST_SEQ_DIRECT_BRANCH_RECORD_LEN)
            .ok_or(QuestSeqError::TruncatedRootRecord {
                available: self.opaque_body.len(),
            })?;
        QuestSeqDirectBranchRecord::parse(raw, address_map)
    }

    /// root OP03 target의 OP0D header까지만 읽는다.
    ///
    /// branch index를 모르는 상태에서는 여기서 frontier를 멈춘다.
    pub fn root_global_word_branch(&self) -> Result<QuestSeqGlobalWordBranchHeader, QuestSeqError> {
        let direct = self.root_direct_branch()?;
        let body_offset = direct
            .target_offset()
            .checked_sub(TOWN00_ROOT_STREAM_OFFSET)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        let available = self.opaque_body.len().saturating_sub(body_offset);
        let raw = self
            .opaque_body
            .get(body_offset..body_offset + QUEST_SEQ_GLOBAL_WORD_BRANCH_HEADER_LEN)
            .ok_or(QuestSeqError::TruncatedGlobalWordBranchHeader {
                stream_offset: direct.target_offset(),
                available,
            })?;
        QuestSeqGlobalWordBranchHeader::parse(raw, direct.target_offset())
    }

    /// runtime에서 얻은 OP0D branch index로 실제 inline pointer를 해석한다.
    ///
    /// handler와 같이 `current + 4 + index×4`를 사용하되, overflow·파일
    /// 범위·target 범위를 모두 실패시킨다.
    pub fn resolve_root_global_word_branch(
        &self,
        branch_index: u32,
    ) -> Result<QuestSeqResolvedBranchTarget, QuestSeqError> {
        self.root_global_word_branch()?;
        let branch_offset =
            usize::try_from(branch_index).map_err(|_| QuestSeqError::CoordinateOverflow)?;
        let pointer_field_offset = TOWN00_ROOT_TARGET_OFFSET
            .checked_add(QUEST_SEQ_GLOBAL_WORD_BRANCH_HEADER_LEN)
            .and_then(|offset| {
                branch_offset
                    .checked_mul(4)
                    .and_then(|delta| offset.checked_add(delta))
            })
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        let body_offset = pointer_field_offset
            .checked_sub(TOWN00_ROOT_STREAM_OFFSET)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        let available = self.opaque_body.len().saturating_sub(body_offset);
        let raw_pointer = self.opaque_body.get(body_offset..body_offset + 4).ok_or(
            QuestSeqError::TruncatedGlobalWordBranchPointer {
                branch_index,
                pointer_field_offset,
                available,
            },
        )?;
        let target_runtime = u32::from_le_bytes(
            raw_pointer
                .try_into()
                .expect("OP0D inline pointer의 4바이트 범위를 확인함"),
        );
        let address_map = town00_address_map()?;
        let target_offset = address_map.file_offset(target_runtime).ok_or(
            QuestSeqError::GlobalWordBranchTargetOutOfRange {
                branch_index,
                pointer_field_offset,
                target_runtime,
            },
        )?;
        Ok(QuestSeqResolvedBranchTarget {
            branch_index,
            pointer_field_offset,
            target_runtime,
            target_offset,
        })
    }

    /// root OP0D의 선택 target을 일반 OP85 20바이트 node로 읽는다.
    pub fn root_op85_node(&self, branch_index: u32) -> Result<Town00Op85Node, QuestSeqError> {
        let target = self.resolve_root_global_word_branch(branch_index)?;
        let body_offset = target
            .target_offset
            .checked_sub(TOWN00_ROOT_STREAM_OFFSET)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        let available = self.opaque_body.len().saturating_sub(body_offset);
        let raw = self
            .opaque_body
            .get(body_offset..body_offset + QUEST_SEQ_PREFIX_RECORD_LEN)
            .ok_or(QuestSeqError::TruncatedOp85Record {
                stream_offset: target.target_offset,
                available,
            })?;
        let record = QuestSeqOp85Record::parse(raw, target.target_offset)?;
        let address_map = town00_address_map()?;
        let pointers = record.renderer_data_pointers();
        let mut renderer_target_offsets = [None; 2];
        for (index, pointer) in pointers.into_iter().enumerate() {
            if pointer != 0 {
                renderer_target_offsets[index] = Some(address_map.file_offset(pointer).ok_or(
                    QuestSeqError::Op85RendererTargetOutOfRange {
                        stream_offset: target.target_offset,
                        pointer_index: index,
                        target_runtime: pointer,
                    },
                )?);
            }
        }
        let next_stream_offset = target
            .target_offset
            .checked_add(QUEST_SEQ_PREFIX_RECORD_LEN)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        Ok(Town00Op85Node {
            stream_offset: target.target_offset,
            next_stream_offset,
            record,
            renderer_target_offsets,
        })
    }

    /// root OP85 다음의 OP88 table load node를 읽고 table 전체 범위를 검증한다.
    pub fn root_op88_node(&self, branch_index: u32) -> Result<Town00Op88Node, QuestSeqError> {
        let op85 = self.root_op85_node(branch_index)?;
        let stream_offset = op85.next_stream_offset;
        let body_offset = stream_offset
            .checked_sub(TOWN00_ROOT_STREAM_OFFSET)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        let available = self.opaque_body.len().saturating_sub(body_offset);
        let raw = self
            .opaque_body
            .get(body_offset..body_offset + QUEST_SEQ_OP88_RECORD_LEN)
            .ok_or(QuestSeqError::TruncatedOp88Record {
                stream_offset,
                available,
            })?;
        let record = QuestSeqOp88Record::parse(raw, stream_offset)?;
        let address_map = town00_address_map()?;
        let table_target_offset = address_map.file_offset(record.table_runtime()).ok_or(
            QuestSeqError::Op88TableTargetOutOfRange {
                stream_offset,
                target_runtime: record.table_runtime(),
                effective_entry_count: record.effective_entry_count(),
            },
        )?;
        let table_bytes = usize::from(record.effective_entry_count())
            .checked_mul(4)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        let table_end_offset = table_target_offset
            .checked_add(table_bytes)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        if table_end_offset > TOWN00_SEQ_LEN {
            return Err(QuestSeqError::Op88TableTargetOutOfRange {
                stream_offset,
                target_runtime: record.table_runtime(),
                effective_entry_count: record.effective_entry_count(),
            });
        }
        let next_stream_offset = stream_offset
            .checked_add(QUEST_SEQ_OP88_RECORD_LEN)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        Ok(Town00Op88Node {
            stream_offset,
            next_stream_offset,
            record,
            table_target_offset,
            table_end_offset,
        })
    }

    /// root OP88 다음의 OP8F halfword pair table node와 table 전체를 검증한다.
    pub fn root_op8f_node(&self, branch_index: u32) -> Result<Town00Op8fNode, QuestSeqError> {
        let op88 = self.root_op88_node(branch_index)?;
        let stream_offset = op88.next_stream_offset;
        let body_offset = stream_offset
            .checked_sub(TOWN00_ROOT_STREAM_OFFSET)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        let available = self.opaque_body.len().saturating_sub(body_offset);
        let raw = self
            .opaque_body
            .get(body_offset..body_offset + QUEST_SEQ_OP8F_RECORD_LEN)
            .ok_or(QuestSeqError::TruncatedOp8fRecord {
                stream_offset,
                available,
            })?;
        let record = QuestSeqOp8fRecord::parse(raw, stream_offset)?;
        let address_map = town00_address_map()?;
        let table_target_offset = address_map.file_offset(record.table_runtime()).ok_or(
            QuestSeqError::Op8fTableTargetOutOfRange {
                stream_offset,
                target_runtime: record.table_runtime(),
                effective_entry_count: record.effective_entry_count(),
            },
        )?;
        let table_bytes = usize::from(record.effective_entry_count())
            .checked_mul(4)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        let table_end_offset = table_target_offset
            .checked_add(table_bytes)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        if table_end_offset > TOWN00_SEQ_LEN {
            return Err(QuestSeqError::Op8fTableTargetOutOfRange {
                stream_offset,
                target_runtime: record.table_runtime(),
                effective_entry_count: record.effective_entry_count(),
            });
        }
        let next_stream_offset = stream_offset
            .checked_add(QUEST_SEQ_OP8F_RECORD_LEN)
            .ok_or(QuestSeqError::CoordinateOverflow)?;
        Ok(Town00Op8fNode {
            stream_offset,
            next_stream_offset,
            record,
            table_target_offset,
            table_end_offset,
        })
    }

    /// 구조 필드를 다시 인코딩하고 불투명 본문을 붙인다.
    pub fn serialize(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(
            self.prefix_records.len() * QUEST_SEQ_PREFIX_RECORD_LEN + self.opaque_body.len(),
        );
        for record in &self.prefix_records {
            output.extend_from_slice(&record.encode());
        }
        output.extend_from_slice(self.opaque_body);
        output
    }
}

fn looks_like_prefix_record(raw: &[u8]) -> bool {
    raw.len() >= QUEST_SEQ_PREFIX_RECORD_LEN
        && raw[0] == PREFIX_MARKER
        && raw[12..QUEST_SEQ_PREFIX_RECORD_LEN]
            .iter()
            .all(|byte| *byte == 0)
}

/// 지원 원본의 현재 자동 검증 범위.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqRoundtripReport {
    pub prefix_records: usize,
    pub prefix_bytes: usize,
    pub opaque_body_bytes: usize,
    pub root_stream_offset: usize,
    pub root_target_offset: usize,
    pub root_frontier_state_word_index: u8,
    pub internal_address_candidate_fields: usize,
    pub internal_address_candidate_targets: usize,
    pub sample_glyphs: usize,
    pub sample_controls: usize,
}

/// 지원 원본에서 정렬된 파일 내부주소 후보를 전수 열거한다.
pub fn town00_internal_address_candidates(
    raw: &[u8],
) -> Result<Vec<Town00InternalAddressCandidate>, QuestSeqError> {
    Town00Seq::parse_source(raw)?;
    scan_internal_address_candidates(raw, TOWN00_SEQ_RUNTIME_BASE)
}

/// 후보와 bounded runtime 소비 증거를 결정적 TSV로 직렬화한다.
pub fn render_town00_internal_address_candidates_tsv(
    candidates: &[Town00InternalAddressCandidate],
) -> String {
    let mut output = String::from(
        "field_offset\tfield_runtime\ttarget_offset\ttarget_runtime\t\
         target_references\tdirection\tevidence\tconsumer_runtime\n",
    );
    for candidate in candidates {
        let consumer = candidate
            .runtime_consumer
            .map_or_else(String::new, |address| format!("0x{address:08X}"));
        writeln!(
            output,
            "0x{:04X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t{}\t{}\t{}\t{}",
            candidate.field_offset,
            candidate.field_runtime,
            candidate.target_offset,
            candidate.target_runtime,
            candidate.target_references,
            candidate.direction(),
            candidate.evidence(),
            consumer,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// `TOWN00.SEQ` 구조·왕복 검증 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqError {
    SourceLength {
        expected: usize,
        actual: usize,
    },
    SourceDigest {
        expected: &'static str,
        actual: String,
    },
    PrefixLengthOverflow,
    TruncatedPrefixRecord {
        index: usize,
        available: usize,
    },
    InvalidPrefixMarker {
        index: usize,
        actual: u8,
    },
    NonZeroPrefixTail {
        index: usize,
        offset: usize,
        actual: u8,
    },
    TruncatedOp85Record {
        stream_offset: usize,
        available: usize,
    },
    InvalidOp85Opcode {
        stream_offset: usize,
        actual: u8,
    },
    Op85RendererTargetOutOfRange {
        stream_offset: usize,
        pointer_index: usize,
        target_runtime: u32,
    },
    TruncatedOp88Record {
        stream_offset: usize,
        available: usize,
    },
    InvalidOp88Opcode {
        stream_offset: usize,
        actual: u8,
    },
    Op88TableTargetOutOfRange {
        stream_offset: usize,
        target_runtime: u32,
        effective_entry_count: u8,
    },
    TruncatedOp8fRecord {
        stream_offset: usize,
        available: usize,
    },
    InvalidOp8fOpcode {
        stream_offset: usize,
        actual: u8,
    },
    Op8fTableTargetOutOfRange {
        stream_offset: usize,
        target_runtime: u32,
        effective_entry_count: u8,
    },
    PrefixBoundaryContinues {
        offset: usize,
    },
    TruncatedRootRecord {
        available: usize,
    },
    InvalidRootOpcode {
        actual: u8,
    },
    RootTargetOutOfRange {
        target_runtime: u32,
        base: u32,
        len: u32,
    },
    RootTargetDrift {
        expected: usize,
        actual: usize,
    },
    CoordinateOverflow,
    TruncatedGlobalWordBranchHeader {
        stream_offset: usize,
        available: usize,
    },
    InvalidGlobalWordBranchOpcode {
        stream_offset: usize,
        actual: u8,
    },
    TruncatedGlobalWordBranchPointer {
        branch_index: u32,
        pointer_field_offset: usize,
        available: usize,
    },
    GlobalWordBranchTargetOutOfRange {
        branch_index: u32,
        pointer_field_offset: usize,
        target_runtime: u32,
    },
    RoundtripMismatch,
    DialogueSampleOutOfRange,
    Token(QuestTokenError),
    ObservationDrift {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for QuestSeqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceLength { expected, actual } => write!(
                f,
                "TOWN00.SEQ 길이 불일치: expected=0x{expected:X}, actual=0x{actual:X}"
            ),
            Self::SourceDigest { expected, actual } => write!(
                f,
                "TOWN00.SEQ SHA-256 불일치: expected={expected}, actual={actual}"
            ),
            Self::PrefixLengthOverflow => f.write_str("SEQ 선두 레코드 범위 overflow"),
            Self::TruncatedPrefixRecord { index, available } => write!(
                f,
                "SEQ 선두 레코드 부족: index={index}, available={available}"
            ),
            Self::InvalidPrefixMarker { index, actual } => write!(
                f,
                "SEQ 선두 레코드 marker 불일치: index={index}, expected=85, actual={actual:02X}"
            ),
            Self::NonZeroPrefixTail {
                index,
                offset,
                actual,
            } => write!(
                f,
                "SEQ 선두 레코드 trailing zero 불일치: index={index}, \
                 offset=0x{offset:02X}, actual={actual:02X}"
            ),
            Self::TruncatedOp85Record {
                stream_offset,
                available,
            } => write!(
                f,
                "SEQ OP85 레코드 부족: stream=0x{stream_offset:X}, \
                 expected=20, available={available}"
            ),
            Self::InvalidOp85Opcode {
                stream_offset,
                actual,
            } => write!(
                f,
                "SEQ OP85 opcode 불일치: stream=0x{stream_offset:X}, \
                 expected=85, actual={actual:02X}"
            ),
            Self::Op85RendererTargetOutOfRange {
                stream_offset,
                pointer_index,
                target_runtime,
            } => write!(
                f,
                "TOWN00 OP85 renderer pointer 범위 이탈: stream=0x{stream_offset:X}, \
                 index={pointer_index}, target=0x{target_runtime:08X}"
            ),
            Self::TruncatedOp88Record {
                stream_offset,
                available,
            } => write!(
                f,
                "SEQ OP88 레코드 부족: stream=0x{stream_offset:X}, \
                 expected=8, available={available}"
            ),
            Self::InvalidOp88Opcode {
                stream_offset,
                actual,
            } => write!(
                f,
                "SEQ OP88 opcode 불일치: stream=0x{stream_offset:X}, \
                 expected=88, actual={actual:02X}"
            ),
            Self::Op88TableTargetOutOfRange {
                stream_offset,
                target_runtime,
                effective_entry_count,
            } => write!(
                f,
                "TOWN00 OP88 table 범위 이탈: stream=0x{stream_offset:X}, \
                 target=0x{target_runtime:08X}, entries={effective_entry_count}"
            ),
            Self::TruncatedOp8fRecord {
                stream_offset,
                available,
            } => write!(
                f,
                "SEQ OP8F 레코드 부족: stream=0x{stream_offset:X}, \
                 expected=8, available={available}"
            ),
            Self::InvalidOp8fOpcode {
                stream_offset,
                actual,
            } => write!(
                f,
                "SEQ OP8F opcode 불일치: stream=0x{stream_offset:X}, \
                 expected=8F, actual={actual:02X}"
            ),
            Self::Op8fTableTargetOutOfRange {
                stream_offset,
                target_runtime,
                effective_entry_count,
            } => write!(
                f,
                "TOWN00 OP8F table 범위 이탈: stream=0x{stream_offset:X}, \
                 target=0x{target_runtime:08X}, entries={effective_entry_count}"
            ),
            Self::PrefixBoundaryContinues { offset } => write!(
                f,
                "SEQ 선두 레코드 선언 뒤에도 같은 형태가 이어짐: offset=0x{offset:X}"
            ),
            Self::TruncatedRootRecord { available } => write!(
                f,
                "TOWN00.SEQ root OP03 레코드 부족: expected=8, available={available}"
            ),
            Self::InvalidRootOpcode { actual } => write!(
                f,
                "TOWN00.SEQ root opcode 불일치: expected=03, actual={actual:02X}"
            ),
            Self::RootTargetOutOfRange {
                target_runtime,
                base,
                len,
            } => write!(
                f,
                "TOWN00.SEQ root target 범위 이탈: target=0x{target_runtime:08X}, \
                 base=0x{base:08X}, len=0x{len:X}"
            ),
            Self::RootTargetDrift { expected, actual } => write!(
                f,
                "TOWN00.SEQ root target drift: expected=0x{expected:X}, actual=0x{actual:X}"
            ),
            Self::CoordinateOverflow => f.write_str("TOWN00.SEQ runtime/file 좌표 overflow"),
            Self::TruncatedGlobalWordBranchHeader {
                stream_offset,
                available,
            } => write!(
                f,
                "TOWN00.SEQ OP0D header 부족: stream=0x{stream_offset:X}, \
                 expected=4, available={available}"
            ),
            Self::InvalidGlobalWordBranchOpcode {
                stream_offset,
                actual,
            } => write!(
                f,
                "TOWN00.SEQ global word branch opcode 불일치: \
                 stream=0x{stream_offset:X}, expected=0D, actual={actual:02X}"
            ),
            Self::TruncatedGlobalWordBranchPointer {
                branch_index,
                pointer_field_offset,
                available,
            } => write!(
                f,
                "TOWN00.SEQ OP0D inline pointer 부족: index={branch_index}, \
                 field=0x{pointer_field_offset:X}, expected=4, available={available}"
            ),
            Self::GlobalWordBranchTargetOutOfRange {
                branch_index,
                pointer_field_offset,
                target_runtime,
            } => write!(
                f,
                "TOWN00.SEQ OP0D target 범위 이탈: index={branch_index}, \
                 field=0x{pointer_field_offset:X}, target=0x{target_runtime:08X}"
            ),
            Self::RoundtripMismatch => f.write_str("TOWN00.SEQ 무변경 직렬화 불일치"),
            Self::DialogueSampleOutOfRange => {
                f.write_str("TOWN00.SEQ 첫 대사 token 표본 범위 이탈")
            }
            Self::Token(source) => write!(f, "TOWN00.SEQ 첫 대사 token 파싱 실패: {source}"),
            Self::ObservationDrift {
                field,
                expected,
                actual,
            } => write!(
                f,
                "TOWN00.SEQ 관측값 drift: {field}, expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for QuestSeqError {}

impl From<QuestTokenError> for QuestSeqError {
    fn from(value: QuestTokenError) -> Self {
        Self::Token(value)
    }
}

fn require_observation(
    field: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), QuestSeqError> {
    if actual != expected {
        return Err(QuestSeqError::ObservationDrift {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn town00_address_map() -> Result<RuntimeAssetMap, QuestSeqError> {
    RuntimeAssetMap::new(
        TOWN00_SEQ_RUNTIME_BASE,
        u32::try_from(TOWN00_SEQ_LEN).map_err(|_| QuestSeqError::CoordinateOverflow)?,
    )
    .ok_or(QuestSeqError::CoordinateOverflow)
}

fn scan_internal_address_candidates(
    raw: &[u8],
    runtime_base: u32,
) -> Result<Vec<Town00InternalAddressCandidate>, QuestSeqError> {
    let raw_len = u32::try_from(raw.len()).map_err(|_| QuestSeqError::PrefixLengthOverflow)?;
    let address_map =
        RuntimeAssetMap::new(runtime_base, raw_len).ok_or(QuestSeqError::PrefixLengthOverflow)?;
    let fields = raw
        .chunks_exact(4)
        .enumerate()
        .filter_map(|(index, word)| {
            let target_runtime = u32::from_le_bytes(word.try_into().ok()?);
            address_map
                .contains(target_runtime)
                .then_some((index * 4, target_runtime))
        })
        .collect::<Vec<_>>();
    let target_references =
        fields
            .iter()
            .fold(BTreeMap::<u32, usize>::new(), |mut counts, (_, target)| {
                *counts.entry(*target).or_default() += 1;
                counts
            });

    fields
        .into_iter()
        .map(|(field_offset, target_runtime)| {
            let target_offset = address_map
                .file_offset(target_runtime)
                .ok_or(QuestSeqError::PrefixLengthOverflow)?;
            let runtime_consumer = TOWN00_CONFIRMED_POINTER_FIELDS
                .iter()
                .find(|confirmed| {
                    confirmed.field_offset == field_offset
                        && confirmed.target_offset == target_offset
                })
                .map(|confirmed| confirmed.consumer_runtime);
            Ok(Town00InternalAddressCandidate {
                field_offset,
                field_runtime: address_map
                    .runtime_addr(field_offset)
                    .ok_or(QuestSeqError::PrefixLengthOverflow)?,
                target_offset,
                target_runtime,
                target_references: target_references[&target_runtime],
                runtime_consumer,
            })
        })
        .collect()
}

/// 지원 원본 `TOWN00.SEQ`의 현재 확인 경계를 한 번에 검증한다.
pub fn verify_town00_seq_roundtrip(raw: &[u8]) -> Result<QuestSeqRoundtripReport, QuestSeqError> {
    let seq = Town00Seq::parse_source(raw)?;
    if seq.serialize() != raw {
        return Err(QuestSeqError::RoundtripMismatch);
    }
    let root = seq.root_direct_branch()?;
    if root.encode()
        != raw[TOWN00_ROOT_STREAM_OFFSET
            ..TOWN00_ROOT_STREAM_OFFSET + QUEST_SEQ_DIRECT_BRANCH_RECORD_LEN]
    {
        return Err(QuestSeqError::RoundtripMismatch);
    }
    if root.target_offset() != TOWN00_ROOT_TARGET_OFFSET {
        return Err(QuestSeqError::RootTargetDrift {
            expected: TOWN00_ROOT_TARGET_OFFSET,
            actual: root.target_offset(),
        });
    }
    let root_frontier = seq.root_global_word_branch()?;
    let root_frontier_offset = TOWN00_ROOT_TARGET_OFFSET - TOWN00_ROOT_STREAM_OFFSET;
    if root_frontier.encode()
        != seq.opaque_body()
            [root_frontier_offset..root_frontier_offset + QUEST_SEQ_GLOBAL_WORD_BRANCH_HEADER_LEN]
    {
        return Err(QuestSeqError::RoundtripMismatch);
    }

    let internal_addresses = scan_internal_address_candidates(raw, TOWN00_SEQ_RUNTIME_BASE)?;
    let unique_targets = internal_addresses
        .iter()
        .map(|candidate| candidate.target_runtime)
        .collect::<BTreeSet<_>>();

    let sample = raw
        .get(FIRST_DIALOGUE_START..FIRST_DIALOGUE_END)
        .ok_or(QuestSeqError::DialogueSampleOutOfRange)?;
    let sample_address = TOWN00_SEQ_RUNTIME_BASE
        .checked_add(
            u32::try_from(FIRST_DIALOGUE_START).map_err(|_| QuestSeqError::PrefixLengthOverflow)?,
        )
        .ok_or(QuestSeqError::PrefixLengthOverflow)?;
    let tokens = parse_quest_tokens(sample, sample_address)?;
    let sample_glyphs = tokens
        .iter()
        .filter(|token| matches!(token, QuestToken::Glyph(_)))
        .count();
    let sample_controls = tokens.len() - sample_glyphs;

    require_observation(
        "prefix records",
        seq.prefix_records().len(),
        TOWN00_PREFIX_RECORDS,
    )?;
    require_observation(
        "internal address candidate fields",
        internal_addresses.len(),
        EXPECTED_INTERNAL_ADDRESS_FIELDS,
    )?;
    require_observation(
        "internal address candidate targets",
        unique_targets.len(),
        EXPECTED_INTERNAL_ADDRESS_TARGETS,
    )?;
    require_observation("sample glyphs", sample_glyphs, EXPECTED_SAMPLE_GLYPHS)?;
    require_observation("sample controls", sample_controls, EXPECTED_SAMPLE_CONTROLS)?;

    Ok(QuestSeqRoundtripReport {
        prefix_records: seq.prefix_records().len(),
        prefix_bytes: seq.prefix_records().len() * QUEST_SEQ_PREFIX_RECORD_LEN,
        opaque_body_bytes: seq.opaque_body().len(),
        root_stream_offset: TOWN00_ROOT_STREAM_OFFSET,
        root_target_offset: root.target_offset(),
        root_frontier_state_word_index: root_frontier.global_state_word_index(),
        internal_address_candidate_fields: internal_addresses.len(),
        internal_address_candidate_targets: unique_targets.len(),
        sample_glyphs,
        sample_controls,
    })
}

#[cfg(test)]
#[path = "quest_seq_tests.rs"]
mod tests;
