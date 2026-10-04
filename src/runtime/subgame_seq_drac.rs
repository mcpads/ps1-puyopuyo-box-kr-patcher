//! PUYO2 `DRAC_SEQ` overlay와 runtime object의 raw RAM 관측.
//!
//! source-verified `DRAC_SEQ.SEQ`의 initialized bytes가 고정 목적지에 있는지
//! 먼저 대조하고, allocator가 object `+0x18`에 저장한 update callback으로
//! message object와 glyph object 후보를 따로 찾는다. glyph 후보는
//! `+0x6A/+0x6C`의 cell/resource를 보존한다. 별도로 RAM의 exact E0 command
//! pointer를 script object `+0x1C` 후보로 분류한다. 세 object를 합치지
//! 않으며 화면 표시나 시간·좌표의 UX 단위로 승격하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::{
    PUYO2_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME, PUYO2_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME,
};

pub const SUBGAME_SEQ_DRAC_RAM_SCHEMA_VERSION: &str = "subgame-seq-drac-ram-v4";
pub const PUYO2_DRAC_SEQUENCE_BYTES: usize = 0x1D4C;
pub const PUYO2_DRAC_INITIALIZED_BYTES: usize = 0x1D20;
pub const PUYO2_DRAC_BSS_BYTES: usize = 0x30;
pub const PUYO2_DRAC_OVERLAY_RUNTIME: u32 = 0x801B_5270;
pub const PUYO2_DRAC_MESSAGE_UPDATE_RUNTIME: u32 = 0x8001_5FA4;
pub const PUYO2_DRAC_MESSAGE_CALLBACK_FIELD: usize = 0x18;
pub const PUYO2_DRAC_GLYPH_UPDATE_RUNTIME: u32 = 0x8001_7400;
pub const PUYO2_DRAC_GLYPH_CALLBACK_FIELD: usize = 0x18;
pub const PUYO2_DRAC_GLYPH_CELL_FIELD: usize = 0x6A;
pub const PUYO2_DRAC_GLYPH_RESOURCE_FIELD: usize = 0x6C;
pub const PUYO2_DRAC_GLYPH_ACTIVE_FIELD: usize = 0x70;
pub const PUYO2_DRAC_GLYPH_KIND_FIELD: usize = 0x73;
pub const PUYO2_DRAC_GLYPH_RESOURCE: u16 = 140;
pub const PUYO2_DRAC_GLYPH_SAME_NAMED_RESOURCE: u16 = 141;
pub const PUYO2_DRAC_GLYPH_CELL_FIRST: u16 = 30;
pub const PUYO2_DRAC_GLYPH_CELL_LAST: u16 = 85;
pub const PUYO2_DRAC_SCRIPT_CURRENT_RECORD_FIELD: usize = 0x1C;
pub const PUYO2_DRAC_COMMAND_RECORDS: [u32; 5] = [
    0x801B_5EEC,
    0x801B_5F08,
    0x801B_5F28,
    0x801B_5F48,
    0x801B_5F64,
];

const PUYO2_DRAC_SEQUENCE_SHA256: [u8; 32] = [
    0x02, 0xA7, 0x1D, 0x36, 0xD2, 0x34, 0x25, 0x0E, 0x2C, 0x80, 0x10, 0xA9, 0x0C, 0xDE, 0x66, 0x8A,
    0xD0, 0x07, 0x64, 0x3D, 0x6F, 0x48, 0xB2, 0xED, 0xFA, 0xE4, 0x78, 0x47, 0x6D, 0x8A, 0x78, 0xC6,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2DracMessageObjectCandidate {
    pub object_runtime: u32,
    pub column: i32,
    pub row_step: i32,
    pub token_cursor: i32,
    pub end_wait: i32,
    pub current_wait: i32,
    pub glyph_wait: i32,
    pub terminal_state: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2DracCurrentRecordCandidate {
    pub object_runtime: u32,
    pub current_record_runtime: u32,
    pub record_index: usize,
    pub variant: u8,
    pub message_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2DracGlyphObjectCandidate {
    pub object_runtime: u32,
    pub x: i32,
    pub y: i32,
    pub cell: u16,
    pub resource: u16,
    pub active: u16,
    pub kind: u8,
    pub main_contract_matches: bool,
    pub same_named_contract_matches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2DracRamObservation {
    pub ram_sha256: [u8; 32],
    pub overlay_matching_bytes: usize,
    pub overlay_initialized_exact: bool,
    pub overlay_bss_zero_bytes: usize,
    pub callback_occurrences: usize,
    pub glyph_callback_occurrences: usize,
    pub exact_main_glyph_candidates: usize,
    pub exact_same_named_glyph_candidates: usize,
    pub main_resource_global: u32,
    pub auxiliary_resource_global: u32,
    pub exact_record_candidates: usize,
    pub message_candidates: Vec<Puyo2DracMessageObjectCandidate>,
    pub glyph_candidates: Vec<Puyo2DracGlyphObjectCandidate>,
    pub current_record_candidates: Vec<Puyo2DracCurrentRecordCandidate>,
}

impl Puyo2DracRamObservation {
    pub const fn observation_status(&self) -> &'static str {
        if !self.overlay_initialized_exact {
            "drac-overlay-not-loaded"
        } else if self.message_candidates.len() == 1 && self.current_record_candidates.len() == 1 {
            "drac-overlay-message-object-and-e0-record-observed-screen-unverified"
        } else if self.message_candidates.is_empty() && self.current_record_candidates.is_empty() {
            "drac-overlay-loaded-message-and-current-record-not-observed"
        } else if self.message_candidates.is_empty() || self.current_record_candidates.is_empty() {
            "drac-overlay-loaded-partial-runtime-observation"
        } else {
            "drac-overlay-loaded-runtime-observation-ambiguous"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Puyo2DracRamError {
    WrongRamLength { expected: usize, actual: usize },
    WrongSequenceLength { expected: usize, actual: usize },
    SequenceDigestMismatch,
    CoordinateOutOfRange(u32),
}

impl fmt::Display for Puyo2DracRamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRamLength { expected, actual } => write!(
                f,
                "PUYO2 DRAC RAM 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::WrongSequenceLength { expected, actual } => write!(
                f,
                "PUYO2 DRAC SEQ 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::SequenceDigestMismatch => f.write_str("PUYO2 DRAC SEQ SHA-256 불일치"),
            Self::CoordinateOutOfRange(runtime) => {
                write!(f, "PUYO2 DRAC RAM 주소 범위 이탈: 0x{runtime:08X}")
            }
        }
    }
}

impl Error for Puyo2DracRamError {}

fn read_u32(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(raw[offset..offset + 4].try_into().expect("4바이트 slice"))
}

fn read_i32(raw: &[u8], object_offset: usize, field: usize) -> i32 {
    read_u32(raw, object_offset + field) as i32
}

fn read_u16(raw: &[u8], object_offset: usize, field: usize) -> u16 {
    u16::from_le_bytes(
        raw[object_offset + field..object_offset + field + 2]
            .try_into()
            .expect("2바이트 slice"),
    )
}

fn read_runtime_u32(raw: &[u8], runtime: u32) -> Result<u32, Puyo2DracRamError> {
    let offset =
        psx_main_ram_offset(runtime, 4).ok_or(Puyo2DracRamError::CoordinateOutOfRange(runtime))?;
    Ok(read_u32(raw, offset))
}

fn inspect_verified_puyo2_drac_ram(
    ram: &[u8],
    initialized: &[u8],
) -> Result<Puyo2DracRamObservation, Puyo2DracRamError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(Puyo2DracRamError::WrongRamLength {
            expected: PSX_MAIN_RAM_LEN,
            actual: ram.len(),
        });
    }
    if initialized.len() != PUYO2_DRAC_INITIALIZED_BYTES {
        return Err(Puyo2DracRamError::WrongSequenceLength {
            expected: PUYO2_DRAC_INITIALIZED_BYTES,
            actual: initialized.len(),
        });
    }
    let overlay_offset = psx_main_ram_offset(
        PUYO2_DRAC_OVERLAY_RUNTIME,
        PUYO2_DRAC_INITIALIZED_BYTES + PUYO2_DRAC_BSS_BYTES,
    )
    .ok_or(Puyo2DracRamError::CoordinateOutOfRange(
        PUYO2_DRAC_OVERLAY_RUNTIME,
    ))?;
    let overlay_ram = &ram[overlay_offset..overlay_offset + PUYO2_DRAC_INITIALIZED_BYTES];
    let overlay_matching_bytes = overlay_ram
        .iter()
        .zip(initialized)
        .filter(|(actual, expected)| actual == expected)
        .count();
    let overlay_initialized_exact = overlay_matching_bytes == PUYO2_DRAC_INITIALIZED_BYTES;
    let bss_start = overlay_offset + PUYO2_DRAC_INITIALIZED_BYTES;
    let overlay_bss_zero_bytes = ram[bss_start..bss_start + PUYO2_DRAC_BSS_BYTES]
        .iter()
        .filter(|&&byte| byte == 0)
        .count();
    let main_resource_global = read_runtime_u32(ram, PUYO2_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME)?;
    let auxiliary_resource_global =
        read_runtime_u32(ram, PUYO2_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME)?;

    let callback = PUYO2_DRAC_MESSAGE_UPDATE_RUNTIME.to_le_bytes();
    let callback_offsets = ram
        .chunks_exact(4)
        .enumerate()
        .filter_map(|(index, word)| (word == callback).then_some(index * 4))
        .collect::<Vec<_>>();
    let mut message_candidates = Vec::new();
    for &callback_offset in &callback_offsets {
        let Some(object_offset) = callback_offset.checked_sub(PUYO2_DRAC_MESSAGE_CALLBACK_FIELD)
        else {
            continue;
        };
        let Some(object_end) = object_offset.checked_add(0xB0) else {
            continue;
        };
        if object_end > ram.len() {
            continue;
        }
        let object_runtime = 0x8000_0000u32
            .checked_add(u32::try_from(object_offset).expect("2 MiB offset은 u32 범위"))
            .expect("main RAM runtime 주소는 u32 범위");
        message_candidates.push(Puyo2DracMessageObjectCandidate {
            object_runtime,
            column: read_i32(ram, object_offset, 0x50),
            row_step: read_i32(ram, object_offset, 0x54),
            token_cursor: read_i32(ram, object_offset, 0x58),
            end_wait: read_i32(ram, object_offset, 0x98),
            current_wait: read_i32(ram, object_offset, 0xA4),
            glyph_wait: read_i32(ram, object_offset, 0xA8),
            terminal_state: read_i32(ram, object_offset, 0xAC),
        });
    }

    let glyph_callback = PUYO2_DRAC_GLYPH_UPDATE_RUNTIME.to_le_bytes();
    let glyph_callback_offsets = ram
        .chunks_exact(4)
        .enumerate()
        .filter_map(|(index, word)| (word == glyph_callback).then_some(index * 4))
        .collect::<Vec<_>>();
    let mut glyph_candidates = Vec::new();
    for &callback_offset in &glyph_callback_offsets {
        let Some(object_offset) = callback_offset.checked_sub(PUYO2_DRAC_GLYPH_CALLBACK_FIELD)
        else {
            continue;
        };
        let Some(object_end) = object_offset.checked_add(PUYO2_DRAC_GLYPH_KIND_FIELD + 1) else {
            continue;
        };
        if object_end > ram.len() {
            continue;
        }
        let object_runtime = 0x8000_0000u32
            .checked_add(u32::try_from(object_offset).expect("2 MiB offset은 u32 범위"))
            .expect("main RAM runtime 주소는 u32 범위");
        let cell = read_u16(ram, object_offset, PUYO2_DRAC_GLYPH_CELL_FIELD);
        let resource = read_u16(ram, object_offset, PUYO2_DRAC_GLYPH_RESOURCE_FIELD);
        let active = read_u16(ram, object_offset, PUYO2_DRAC_GLYPH_ACTIVE_FIELD);
        let kind = ram[object_offset + PUYO2_DRAC_GLYPH_KIND_FIELD];
        let main_contract_matches = resource == PUYO2_DRAC_GLYPH_RESOURCE
            && (PUYO2_DRAC_GLYPH_CELL_FIRST..=PUYO2_DRAC_GLYPH_CELL_LAST).contains(&cell)
            && active == 1
            && kind == 1;
        let same_named_contract_matches = resource == PUYO2_DRAC_GLYPH_SAME_NAMED_RESOURCE
            && (PUYO2_DRAC_GLYPH_CELL_FIRST..=PUYO2_DRAC_GLYPH_CELL_LAST).contains(&cell)
            && active == 1
            && kind == 1;
        glyph_candidates.push(Puyo2DracGlyphObjectCandidate {
            object_runtime,
            x: read_i32(ram, object_offset, 0x50),
            y: read_i32(ram, object_offset, 0x54),
            cell,
            resource,
            active,
            kind,
            main_contract_matches,
            same_named_contract_matches,
        });
    }
    let exact_main_glyph_candidates = glyph_candidates
        .iter()
        .filter(|candidate| candidate.main_contract_matches)
        .count();
    let exact_same_named_glyph_candidates = glyph_candidates
        .iter()
        .filter(|candidate| candidate.same_named_contract_matches)
        .count();

    let mut current_record_candidates = Vec::new();
    for (word_index, word) in ram.chunks_exact(4).enumerate() {
        let current_record_runtime = u32::from_le_bytes(word.try_into().expect("4바이트 word"));
        let Some(record_index) = PUYO2_DRAC_COMMAND_RECORDS
            .iter()
            .position(|&runtime| runtime == current_record_runtime)
        else {
            continue;
        };
        let current_record_field_offset = word_index * 4;
        let Some(object_offset) =
            current_record_field_offset.checked_sub(PUYO2_DRAC_SCRIPT_CURRENT_RECORD_FIELD)
        else {
            continue;
        };
        let record_offset = psx_main_ram_offset(current_record_runtime, 8).ok_or(
            Puyo2DracRamError::CoordinateOutOfRange(current_record_runtime),
        )?;
        let record = &ram[record_offset..record_offset + 8];
        if record[0] != 0xE0 || record[2..4] != [0, 0] {
            continue;
        }
        let object_runtime = 0x8000_0000u32
            .checked_add(u32::try_from(object_offset).expect("2 MiB offset은 u32 범위"))
            .expect("main RAM runtime 주소는 u32 범위");
        current_record_candidates.push(Puyo2DracCurrentRecordCandidate {
            object_runtime,
            current_record_runtime,
            record_index: record_index + 1,
            variant: record[1],
            message_runtime: read_u32(record, 4),
        });
    }
    let exact_record_candidates = current_record_candidates.len();

    Ok(Puyo2DracRamObservation {
        ram_sha256: Sha256::digest(ram).into(),
        overlay_matching_bytes,
        overlay_initialized_exact,
        overlay_bss_zero_bytes,
        callback_occurrences: callback_offsets.len(),
        glyph_callback_occurrences: glyph_callback_offsets.len(),
        exact_main_glyph_candidates,
        exact_same_named_glyph_candidates,
        main_resource_global,
        auxiliary_resource_global,
        exact_record_candidates,
        message_candidates,
        glyph_candidates,
        current_record_candidates,
    })
}

/// source-verified PUYO2 `DRAC_SEQ.SEQ`와 raw main RAM을 교차한다.
pub fn inspect_puyo2_drac_ram(
    ram: &[u8],
    sequence: &[u8],
) -> Result<Puyo2DracRamObservation, Puyo2DracRamError> {
    if sequence.len() != PUYO2_DRAC_SEQUENCE_BYTES {
        return Err(Puyo2DracRamError::WrongSequenceLength {
            expected: PUYO2_DRAC_SEQUENCE_BYTES,
            actual: sequence.len(),
        });
    }
    let digest: [u8; 32] = Sha256::digest(sequence).into();
    if digest != PUYO2_DRAC_SEQUENCE_SHA256 {
        return Err(Puyo2DracRamError::SequenceDigestMismatch);
    }
    inspect_verified_puyo2_drac_ram(ram, &sequence[..PUYO2_DRAC_INITIALIZED_BYTES])
}

pub fn render_puyo2_drac_ram_tsv(observation: &Puyo2DracRamObservation) -> String {
    let message_candidates = observation
        .message_candidates
        .iter()
        .map(|candidate| {
            format!(
                "0x{:08X}/column={}/row={}/cursor={}/end-wait={}/current-wait={}/glyph-wait={}/terminal={}",
                candidate.object_runtime,
                candidate.column,
                candidate.row_step,
                candidate.token_cursor,
                candidate.end_wait,
                candidate.current_wait,
                candidate.glyph_wait,
                candidate.terminal_state,
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let current_record_candidates = observation
        .current_record_candidates
        .iter()
        .map(|candidate| {
            format!(
                "0x{:08X}/record=0x{:08X}/index={}/variant={}/message=0x{:08X}",
                candidate.object_runtime,
                candidate.current_record_runtime,
                candidate.record_index,
                candidate.variant,
                candidate.message_runtime,
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let glyph_candidates = observation
        .glyph_candidates
        .iter()
        .map(|candidate| {
            format!(
                "0x{:08X}/x={}/y={}/cell={}/resource={}/active={}/kind={}/main-contract={}/same-named-contract={}",
                candidate.object_runtime,
                candidate.x,
                candidate.y,
                candidate.cell,
                candidate.resource,
                candidate.active,
                candidate.kind,
                candidate.main_contract_matches,
                candidate.same_named_contract_matches,
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let mut output = String::from(
        "schema_version\tram_sha256\toverlay_runtime\toverlay_initialized_bytes\t\
         overlay_matching_bytes\toverlay_initialized_exact\toverlay_bss_zero_bytes\t\
         callback_runtime\tcallback_field\tcallback_occurrences\texact_record_candidates\t\
         message_candidates\tcurrent_record_candidates\tglyph_callback_runtime\t\
         glyph_callback_field\tglyph_callback_occurrences\texact_main_glyph_candidates\t\
         exact_same_named_glyph_candidates\tmain_resource_global_runtime\tmain_resource_global\t\
         auxiliary_resource_global_runtime\tauxiliary_resource_global\tglyph_candidates\t\
         observation_status\tclaim_boundary\n",
    );
    writeln!(
        output,
        "{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t0x{:08X}\t+0x{:02X}\t{}\t{}\t{}\t{}\t0x{:08X}\t+0x{:02X}\t{}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t{}\t{}\traw-ram-only-screen-clut-vram-draw-unverified",
        SUBGAME_SEQ_DRAC_RAM_SCHEMA_VERSION,
        hex(&observation.ram_sha256),
        PUYO2_DRAC_OVERLAY_RUNTIME,
        PUYO2_DRAC_INITIALIZED_BYTES,
        observation.overlay_matching_bytes,
        observation.overlay_initialized_exact,
        observation.overlay_bss_zero_bytes,
        PUYO2_DRAC_MESSAGE_UPDATE_RUNTIME,
        PUYO2_DRAC_MESSAGE_CALLBACK_FIELD,
        observation.callback_occurrences,
        observation.exact_record_candidates,
        message_candidates,
        current_record_candidates,
        PUYO2_DRAC_GLYPH_UPDATE_RUNTIME,
        PUYO2_DRAC_GLYPH_CALLBACK_FIELD,
        observation.glyph_callback_occurrences,
        observation.exact_main_glyph_candidates,
        observation.exact_same_named_glyph_candidates,
        PUYO2_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME,
        observation.main_resource_global,
        PUYO2_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME,
        observation.auxiliary_resource_global,
        glyph_candidates,
        observation.observation_status(),
    )
    .expect("String 쓰기는 실패하지 않는다");
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_drac_tests.rs"]
mod tests;
