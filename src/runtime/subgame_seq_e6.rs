//! PUYO2 `E6` overlay와 glyph object의 raw RAM 관측.
//!
//! 지원 원본에서 검증한 33개 overlay initialized byte와 raw 2 MiB RAM을
//! 대조해 현재 상주 자산을 식별한다. 같은 snapshot에서 descriptor `+0x10`,
//! object pool의 E6 callback, cell/resource/active/type과 E6 current-record
//! pointer 후보를 읽는다. 이 결과는 PC hit·GPU upload·draw·화면 표시를
//! 대신하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};

pub const SUBGAME_SEQ_E6_RAM_SCHEMA_VERSION: &str = "subgame-seq-e6-ram-v1";
pub const PUYO2_E6_OVERLAY_RUNTIME: u32 = 0x801B_5270;
pub const PUYO2_E6_RESOURCE_FIRST: u16 = 0x003F;
pub const PUYO2_E6_RESOURCE_LAST: u16 = 0x005F;
pub const PUYO2_E6_SEQUENCE_COUNT: usize = 33;
pub const PUYO2_E6_DESCRIPTOR_BASE_RUNTIME: u32 = 0x8006_6824;
pub const PUYO2_E6_DESCRIPTOR_STRIDE: u32 = 36;
pub const PUYO2_E6_DESCRIPTOR_DESTINATION_FIELD: u32 = 0x10;
pub const PUYO2_E6_SELECTOR_RUNTIME: u32 = 0x8001_9780;
pub const PUYO2_E6_COMPLETION_CALLBACK_RUNTIME: u32 = 0x8001_9D0C;
pub const PUYO2_E6_COMMAND_HANDLER_RUNTIME: u32 = 0x8001_7A5C;
pub const PUYO2_E6_PARSER_RUNTIME: u32 = 0x8001_6F30;
pub const PUYO2_E6_GLYPH_CALLBACK_RUNTIME: u32 = 0x8001_79C8;
pub const PUYO2_E6_SHORT_RENDERER_RUNTIME: u32 = 0x8002_3AF0;
pub const PUYO2_E6_OBJECT_POOL_RUNTIME: u32 = 0x8009_82F8;
pub const PUYO2_E6_OBJECT_SLOTS: usize = 500;
pub const PUYO2_E6_OBJECT_BYTES: usize = 0xEC;
pub const PUYO2_E6_OBJECT_CALLBACK_FIELD: usize = 0x18;
pub const PUYO2_E6_OBJECT_CURRENT_RECORD_FIELD: usize = 0x1C;
pub const PUYO2_E6_OBJECT_TYPE_FIELD: usize = 0x41;
pub const PUYO2_E6_OBJECT_X_FIELD: usize = 0x50;
pub const PUYO2_E6_OBJECT_Y_FIELD: usize = 0x54;
pub const PUYO2_E6_OBJECT_CELL_FIELD: usize = 0x6A;
pub const PUYO2_E6_OBJECT_RESOURCE_FIELD: usize = 0x6C;
pub const PUYO2_E6_OBJECT_ACTIVE_FIELD: usize = 0x70;
pub const PUYO2_E6_OBJECT_OVERRIDE_FIELD: usize = 0x78;
pub const PUYO2_E6_OBJECT_TYPE: u8 = 4;
pub const PUYO2_E6_GLYPH_RESOURCE: u16 = 0x0088;
pub const PUYO2_E6_USED_CELL_COUNT: usize = 210;
pub const PUYO2_E6_MIN_CELL: u16 = 0;
pub const PUYO2_E6_MAX_CELL: u16 = 246;

const OVERLAY_FOOTER_BYTES: usize = 44;
const E6_COMMAND: [u8; 4] = [0xE6, 0, 0, 0];

#[derive(Debug, Clone, Copy)]
pub struct Puyo2E6RuntimeSource<'a> {
    pub resource: u16,
    pub sequence: &'a str,
    pub raw: &'a [u8],
    pub sequence_sha256: [u8; 32],
    pub load_base: u32,
    pub initialized_bytes: usize,
    pub bss_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6OverlayMatch {
    pub resource: u16,
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub initialized_bytes: usize,
    pub matching_bytes: usize,
    pub bss_zero_bytes: usize,
    pub descriptor_runtime: u32,
    pub descriptor_destination: u32,
    pub descriptor_resident: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6GlyphObjectCandidate {
    pub object_runtime: u32,
    pub slot: usize,
    pub object_type: u8,
    pub x: i32,
    pub y: i32,
    pub cell: u16,
    pub resource: u16,
    pub active: u16,
    pub override_value: u32,
    pub contract_matches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6CurrentRecordCandidate {
    pub object_runtime: u32,
    pub slot: usize,
    pub object_type: u8,
    pub callback_runtime: u32,
    pub current_record_runtime: u32,
    pub command_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6RamObservation {
    pub ram_sha256: [u8; 32],
    pub source_sequences: usize,
    pub best_matching_bytes: usize,
    pub best_matching_sequences: Vec<String>,
    pub exact_overlay_matches: Vec<Puyo2E6OverlayMatch>,
    pub pool_callback_candidates: usize,
    pub exact_glyph_candidates: usize,
    pub glyph_candidates: Vec<Puyo2E6GlyphObjectCandidate>,
    pub current_record_candidates: Vec<Puyo2E6CurrentRecordCandidate>,
}

impl Puyo2E6RamObservation {
    pub fn observation_status(&self) -> &'static str {
        let [overlay] = self.exact_overlay_matches.as_slice() else {
            return if self.exact_overlay_matches.is_empty() {
                "e6-overlay-not-loaded"
            } else {
                "e6-overlay-match-ambiguous"
            };
        };
        if !overlay.descriptor_resident {
            "e6-overlay-exact-descriptor-residency-unconfirmed"
        } else if self.exact_glyph_candidates == 0 {
            "e6-overlay-resident-glyph-object-not-observed"
        } else if self.current_record_candidates.is_empty() {
            "e6-overlay-resident-glyph-object-observed-current-record-unobserved-screen-unverified"
        } else {
            "e6-overlay-resident-current-record-and-glyph-object-observed-screen-unverified"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Puyo2E6RamError {
    WrongRamLength { expected: usize, actual: usize },
    WrongSourceCount { expected: usize, actual: usize },
    DuplicateResource(u16),
    ResourceSetMismatch,
    DuplicateSequence(String),
    SourceDigestMismatch(String),
    SourceFooterMismatch(String),
    WrongUsedCellCount { expected: usize, actual: usize },
    UsedCellRangeMismatch,
    CoordinateOutOfRange(u32),
}

impl fmt::Display for Puyo2E6RamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRamLength { expected, actual } => write!(
                f,
                "PUYO2 E6 RAM 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::WrongSourceCount { expected, actual } => write!(
                f,
                "PUYO2 E6 source 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::DuplicateResource(resource) => {
                write!(f, "PUYO2 E6 resource 중복: 0x{resource:04X}")
            }
            Self::ResourceSetMismatch => f.write_str("PUYO2 E6 resource 집합 불일치"),
            Self::DuplicateSequence(sequence) => {
                write!(f, "PUYO2 E6 sequence 이름 중복: {sequence}")
            }
            Self::SourceDigestMismatch(sequence) => {
                write!(f, "PUYO2 E6 source SHA-256 불일치: {sequence}")
            }
            Self::SourceFooterMismatch(sequence) => {
                write!(f, "PUYO2 E6 source footer 불일치: {sequence}")
            }
            Self::WrongUsedCellCount { expected, actual } => write!(
                f,
                "PUYO2 E6 used cell 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::UsedCellRangeMismatch => f.write_str("PUYO2 E6 used cell 범위 불일치"),
            Self::CoordinateOutOfRange(runtime) => {
                write!(f, "PUYO2 E6 RAM 주소 범위 이탈: 0x{runtime:08X}")
            }
        }
    }
}

impl Error for Puyo2E6RamError {}

fn read_u16(raw: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(raw[offset..offset + 2].try_into().expect("2바이트 slice"))
}

fn read_u32(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(raw[offset..offset + 4].try_into().expect("4바이트 slice"))
}

fn read_i32(raw: &[u8], offset: usize) -> i32 {
    read_u32(raw, offset) as i32
}

fn read_runtime_u32(raw: &[u8], runtime: u32) -> Result<u32, Puyo2E6RamError> {
    let offset =
        psx_main_ram_offset(runtime, 4).ok_or(Puyo2E6RamError::CoordinateOutOfRange(runtime))?;
    Ok(read_u32(raw, offset))
}

fn validate_source(source: Puyo2E6RuntimeSource<'_>) -> Result<(), Puyo2E6RamError> {
    if <[u8; 32]>::from(Sha256::digest(source.raw)) != source.sequence_sha256 {
        return Err(Puyo2E6RamError::SourceDigestMismatch(
            source.sequence.to_string(),
        ));
    }
    let Some(footer_offset) = source.raw.len().checked_sub(OVERLAY_FOOTER_BYTES) else {
        return Err(Puyo2E6RamError::SourceFooterMismatch(
            source.sequence.to_string(),
        ));
    };
    if footer_offset != source.initialized_bytes {
        return Err(Puyo2E6RamError::SourceFooterMismatch(
            source.sequence.to_string(),
        ));
    }
    let footer = &source.raw[footer_offset..];
    let words = (0..11)
        .map(|index| read_u32(footer, index * 4))
        .collect::<Vec<_>>();
    let initialized_u32 = u32::try_from(source.initialized_bytes).ok();
    let expected_bss_runtime =
        initialized_u32.and_then(|initialized| source.load_base.checked_add(initialized));
    let expected_next_runtime = expected_bss_runtime.and_then(|bss| bss.checked_add(0x30));
    if words[0..3] != [0, 0, 0]
        || words[3] != source.load_base
        || Some(words[4]) != initialized_u32
        || Some(words[5]) != expected_bss_runtime
        || usize::try_from(words[6]).ok() != Some(source.bss_bytes)
        || Some(words[7]) != expected_next_runtime
        || words[8] != 0
        || words[9] != 0x0000_8000
        || words[10] != 0x0080_0000
    {
        return Err(Puyo2E6RamError::SourceFooterMismatch(
            source.sequence.to_string(),
        ));
    }
    Ok(())
}

pub fn inspect_puyo2_e6_ram<'a>(
    ram: &[u8],
    sources: impl IntoIterator<Item = Puyo2E6RuntimeSource<'a>>,
    used_cell_indices: &[usize],
) -> Result<Puyo2E6RamObservation, Puyo2E6RamError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(Puyo2E6RamError::WrongRamLength {
            expected: PSX_MAIN_RAM_LEN,
            actual: ram.len(),
        });
    }
    let source_map = sources
        .into_iter()
        .map(|source| (source.resource, source))
        .try_fold(BTreeMap::new(), |mut sources, (resource, source)| {
            if sources.insert(resource, source).is_some() {
                return Err(Puyo2E6RamError::DuplicateResource(resource));
            }
            Ok(sources)
        })?;
    if source_map.len() != PUYO2_E6_SEQUENCE_COUNT {
        return Err(Puyo2E6RamError::WrongSourceCount {
            expected: PUYO2_E6_SEQUENCE_COUNT,
            actual: source_map.len(),
        });
    }
    let expected_resources =
        (PUYO2_E6_RESOURCE_FIRST..=PUYO2_E6_RESOURCE_LAST).collect::<BTreeSet<_>>();
    if source_map.keys().copied().collect::<BTreeSet<_>>() != expected_resources {
        return Err(Puyo2E6RamError::ResourceSetMismatch);
    }
    let mut names = BTreeSet::new();
    for source in source_map.values().copied() {
        if !names.insert(source.sequence) {
            return Err(Puyo2E6RamError::DuplicateSequence(
                source.sequence.to_string(),
            ));
        }
        validate_source(source)?;
        if source.load_base != PUYO2_E6_OVERLAY_RUNTIME {
            return Err(Puyo2E6RamError::SourceFooterMismatch(
                source.sequence.to_string(),
            ));
        }
    }

    let used_cells = used_cell_indices
        .iter()
        .copied()
        .map(|value| u16::try_from(value).map_err(|_| Puyo2E6RamError::UsedCellRangeMismatch))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if used_cells.len() != PUYO2_E6_USED_CELL_COUNT {
        return Err(Puyo2E6RamError::WrongUsedCellCount {
            expected: PUYO2_E6_USED_CELL_COUNT,
            actual: used_cells.len(),
        });
    }
    if used_cells.first().copied() != Some(PUYO2_E6_MIN_CELL)
        || used_cells.last().copied() != Some(PUYO2_E6_MAX_CELL)
    {
        return Err(Puyo2E6RamError::UsedCellRangeMismatch);
    }

    let mut best_matching_bytes = 0usize;
    let mut best_matching_sequences = Vec::new();
    let mut exact_overlay_matches = Vec::new();
    for source in source_map.values().copied() {
        let overlay_offset = psx_main_ram_offset(source.load_base, source.initialized_bytes)
            .ok_or(Puyo2E6RamError::CoordinateOutOfRange(source.load_base))?;
        let initialized_ram = &ram[overlay_offset..overlay_offset + source.initialized_bytes];
        let matching_bytes = initialized_ram
            .iter()
            .zip(&source.raw[..source.initialized_bytes])
            .filter(|(actual, expected)| actual == expected)
            .count();
        if matching_bytes > best_matching_bytes {
            best_matching_bytes = matching_bytes;
            best_matching_sequences.clear();
            best_matching_sequences.push(source.sequence.to_string());
        } else if matching_bytes == best_matching_bytes {
            best_matching_sequences.push(source.sequence.to_string());
        }
        if matching_bytes != source.initialized_bytes {
            continue;
        }
        let descriptor_runtime = PUYO2_E6_DESCRIPTOR_BASE_RUNTIME
            + u32::from(source.resource) * PUYO2_E6_DESCRIPTOR_STRIDE;
        let destination_runtime = descriptor_runtime + PUYO2_E6_DESCRIPTOR_DESTINATION_FIELD;
        let descriptor_destination = read_runtime_u32(ram, destination_runtime)?;
        let bss_offset = psx_main_ram_offset(
            source.load_base + u32::try_from(source.initialized_bytes).expect("SEQ 길이는 u32"),
            source.bss_bytes,
        )
        .ok_or(Puyo2E6RamError::CoordinateOutOfRange(source.load_base))?;
        let bss_zero_bytes = ram[bss_offset..bss_offset + source.bss_bytes]
            .iter()
            .filter(|&&byte| byte == 0)
            .count();
        exact_overlay_matches.push(Puyo2E6OverlayMatch {
            resource: source.resource,
            sequence: source.sequence.to_string(),
            sequence_sha256: source.sequence_sha256,
            initialized_bytes: source.initialized_bytes,
            matching_bytes,
            bss_zero_bytes,
            descriptor_runtime,
            descriptor_destination,
            descriptor_resident: descriptor_destination == source.load_base,
        });
    }

    let pool_offset = psx_main_ram_offset(
        PUYO2_E6_OBJECT_POOL_RUNTIME,
        PUYO2_E6_OBJECT_SLOTS * PUYO2_E6_OBJECT_BYTES,
    )
    .ok_or(Puyo2E6RamError::CoordinateOutOfRange(
        PUYO2_E6_OBJECT_POOL_RUNTIME,
    ))?;
    let mut glyph_candidates = Vec::new();
    for slot in 0..PUYO2_E6_OBJECT_SLOTS {
        let object_offset = pool_offset + slot * PUYO2_E6_OBJECT_BYTES;
        let callback_runtime = read_u32(ram, object_offset + PUYO2_E6_OBJECT_CALLBACK_FIELD);
        if callback_runtime != PUYO2_E6_GLYPH_CALLBACK_RUNTIME {
            continue;
        }
        let object_type = ram[object_offset + PUYO2_E6_OBJECT_TYPE_FIELD];
        let cell = read_u16(ram, object_offset + PUYO2_E6_OBJECT_CELL_FIELD);
        let resource = read_u16(ram, object_offset + PUYO2_E6_OBJECT_RESOURCE_FIELD);
        let active = read_u16(ram, object_offset + PUYO2_E6_OBJECT_ACTIVE_FIELD);
        let override_value = read_u32(ram, object_offset + PUYO2_E6_OBJECT_OVERRIDE_FIELD);
        let contract_matches = object_type == PUYO2_E6_OBJECT_TYPE
            && resource == PUYO2_E6_GLYPH_RESOURCE
            && active == 1
            && used_cells.contains(&cell);
        glyph_candidates.push(Puyo2E6GlyphObjectCandidate {
            object_runtime: PUYO2_E6_OBJECT_POOL_RUNTIME
                + u32::try_from(slot * PUYO2_E6_OBJECT_BYTES).expect("object pool은 u32"),
            slot,
            object_type,
            x: read_i32(ram, object_offset + PUYO2_E6_OBJECT_X_FIELD),
            y: read_i32(ram, object_offset + PUYO2_E6_OBJECT_Y_FIELD),
            cell,
            resource,
            active,
            override_value,
            contract_matches,
        });
    }
    let exact_glyph_candidates = glyph_candidates
        .iter()
        .filter(|candidate| candidate.contract_matches)
        .count();

    let mut command_offsets = BTreeMap::<u32, usize>::new();
    for overlay in &exact_overlay_matches {
        let source = source_map
            .get(&overlay.resource)
            .expect("exact match resource는 source map에 있음");
        for offset in (0..=source.initialized_bytes.saturating_sub(4)).step_by(4) {
            if source.raw.get(offset..offset + 4) == Some(E6_COMMAND.as_slice()) {
                command_offsets.insert(
                    source.load_base + u32::try_from(offset).expect("SEQ offset은 u32"),
                    offset,
                );
            }
        }
    }
    let mut current_record_candidates = Vec::new();
    for slot in 0..PUYO2_E6_OBJECT_SLOTS {
        let object_offset = pool_offset + slot * PUYO2_E6_OBJECT_BYTES;
        let current_record_runtime =
            read_u32(ram, object_offset + PUYO2_E6_OBJECT_CURRENT_RECORD_FIELD);
        let Some(&command_offset) = command_offsets.get(&current_record_runtime) else {
            continue;
        };
        current_record_candidates.push(Puyo2E6CurrentRecordCandidate {
            object_runtime: PUYO2_E6_OBJECT_POOL_RUNTIME
                + u32::try_from(slot * PUYO2_E6_OBJECT_BYTES).expect("object pool은 u32"),
            slot,
            object_type: ram[object_offset + PUYO2_E6_OBJECT_TYPE_FIELD],
            callback_runtime: read_u32(ram, object_offset + PUYO2_E6_OBJECT_CALLBACK_FIELD),
            current_record_runtime,
            command_offset,
        });
    }

    Ok(Puyo2E6RamObservation {
        ram_sha256: Sha256::digest(ram).into(),
        source_sequences: source_map.len(),
        best_matching_bytes,
        best_matching_sequences,
        exact_overlay_matches,
        pool_callback_candidates: glyph_candidates.len(),
        exact_glyph_candidates,
        glyph_candidates,
        current_record_candidates,
    })
}

pub fn render_puyo2_e6_ram_tsv(observation: &Puyo2E6RamObservation) -> String {
    let exact_overlays = observation
        .exact_overlay_matches
        .iter()
        .map(|overlay| {
            format!(
                "0x{:04X}/{}:{}/init={}/bss-zero={}/descriptor=0x{:08X}/destination=0x{:08X}/resident={}",
                overlay.resource,
                overlay.sequence,
                hex(&overlay.sequence_sha256),
                overlay.initialized_bytes,
                overlay.bss_zero_bytes,
                overlay.descriptor_runtime,
                overlay.descriptor_destination,
                overlay.descriptor_resident,
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let glyph_candidates = observation
        .glyph_candidates
        .iter()
        .map(|candidate| {
            format!(
                "0x{:08X}/slot={}/type={}/x={}/y={}/cell={}/resource={}/active={}/override={}/contract={}",
                candidate.object_runtime,
                candidate.slot,
                candidate.object_type,
                candidate.x,
                candidate.y,
                candidate.cell,
                candidate.resource,
                candidate.active,
                candidate.override_value,
                candidate.contract_matches,
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let current_records = observation
        .current_record_candidates
        .iter()
        .map(|candidate| {
            format!(
                "0x{:08X}/slot={}/type={}/callback=0x{:08X}/record=0x{:08X}/offset=0x{:04X}",
                candidate.object_runtime,
                candidate.slot,
                candidate.object_type,
                candidate.callback_runtime,
                candidate.current_record_runtime,
                candidate.command_offset,
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let mut output = String::from(
        "schema_version\tram_sha256\tsource_sequences\toverlay_runtime\tbest_matching_bytes\t\
         best_matching_sequences\texact_overlay_matches\tpool_runtime\tpool_slots\t\
         glyph_callback_runtime\tpool_callback_candidates\texact_glyph_candidates\t\
         glyph_candidates\tcurrent_record_candidates\tselector_probe_runtime\t\
         completion_probe_runtime\te6_handler_probe_runtime\tparser_probe_runtime\t\
         short_renderer_probe_runtime\tobservation_status\tclaim_boundary\n",
    );
    writeln!(
        output,
        "{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\traw-ram-snapshot-only-pc-hit-gpu-upload-draw-screen-unverified",
        SUBGAME_SEQ_E6_RAM_SCHEMA_VERSION,
        hex(&observation.ram_sha256),
        observation.source_sequences,
        PUYO2_E6_OVERLAY_RUNTIME,
        observation.best_matching_bytes,
        observation.best_matching_sequences.join(","),
        exact_overlays,
        PUYO2_E6_OBJECT_POOL_RUNTIME,
        PUYO2_E6_OBJECT_SLOTS,
        PUYO2_E6_GLYPH_CALLBACK_RUNTIME,
        observation.pool_callback_candidates,
        observation.exact_glyph_candidates,
        glyph_candidates,
        current_records,
        PUYO2_E6_SELECTOR_RUNTIME,
        PUYO2_E6_COMPLETION_CALLBACK_RUNTIME,
        PUYO2_E6_COMMAND_HANDLER_RUNTIME,
        PUYO2_E6_PARSER_RUNTIME,
        PUYO2_E6_SHORT_RENDERER_RUNTIME,
        observation.observation_status(),
    )
    .expect("String 쓰기는 실패하지 않는다");
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_e6_tests.rs"]
mod tests;
