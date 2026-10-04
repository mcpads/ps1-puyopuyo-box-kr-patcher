//! block 분할 대기 BTX의 raw RAM·VRAM snapshot 관측.
//!
//! 지원 원본에서 검증한 EXE 선택 code window로 현재 pending 실행 경계 중 하나를
//! 식별하고, 그 EXE의 pending descriptor identity와 working descriptor를
//! 비교한다. 같은 snapshot의 VRAM에서는 해제 BTX 전체의 exact 직사각형만
//! 열거한다. 이 결과는 descriptor read PC hit, 실제 load·primitive draw,
//! 화면 표시를 대신하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::btx::PixelFormat;
use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::graphics::runtime_layout::{
    exact_vram_placements, RuntimeLayoutError, VramPlacement, VRAM_BYTE_LEN,
};

pub const BTX_PENDING_RUNTIME_SCHEMA_VERSION: &str = "btx-pending-runtime-v1";
pub const BTX_DESCRIPTOR_IDENTITY_BYTES: usize = 16;
pub const BTX_SELECTION_CODE_MIN_BYTES: usize = 20;

#[derive(Debug, Clone, Copy)]
pub struct PendingBtxRuntimeSource<'a> {
    pub finding_id: &'a str,
    pub exe: &'static str,
    pub archive: &'static str,
    pub asset_name: &'a str,
    pub format: PixelFormat,
    pub raw: &'a [u8],
    pub expected_sha256: &'a str,
    pub descriptor_entry_runtime: u32,
    pub original_source_descriptor_identity: &'a [u8],
    pub working_descriptor_runtime: u32,
    pub selection_code_runtime: u32,
    pub selection_code: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingBtxRuntimeRow {
    pub finding_id: String,
    pub exe: &'static str,
    pub archive: &'static str,
    pub asset_name: String,
    pub asset_sha256: [u8; 32],
    pub format: PixelFormat,
    pub descriptor_entry_runtime: u32,
    pub working_descriptor_runtime: u32,
    pub source_descriptor_populated: bool,
    pub source_descriptor_original_match: bool,
    pub descriptor_selected: bool,
    pub vram_layouts: Vec<(usize, usize)>,
    pub vram_placements: Vec<VramPlacement>,
}

impl PendingBtxRuntimeRow {
    pub fn observation_status(&self) -> &'static str {
        if self.vram_layouts.len() > 1 {
            "vram-exact-layout-ambiguous-screen-unverified"
        } else if !self.vram_placements.is_empty() && self.descriptor_selected {
            "descriptor-selected-vram-resident-screen-unverified"
        } else if !self.vram_placements.is_empty() {
            "vram-resident-descriptor-selection-unobserved-screen-unverified"
        } else if self.descriptor_selected {
            "descriptor-selected-vram-not-resident-at-snapshot"
        } else {
            "not-observed-at-snapshot"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingBtxRuntimeObservation {
    pub exe: &'static str,
    pub ram_sha256: [u8; 32],
    pub vram_sha256: [u8; 32],
    pub selection_code_runtime: u32,
    pub selection_code_bytes_verified: usize,
    pub working_descriptor_runtime: u32,
    pub rows: Vec<PendingBtxRuntimeRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingBtxRuntimeError {
    WrongRamLength {
        expected: usize,
        actual: usize,
    },
    WrongVramLength {
        expected: usize,
        actual: usize,
    },
    EmptySources,
    EmptyIdentity(&'static str),
    WrongDescriptorIdentityLength {
        exe: &'static str,
        finding_id: String,
        expected: usize,
        actual: usize,
    },
    ShortSelectionCode {
        exe: &'static str,
        expected_at_least: usize,
        actual: usize,
    },
    UnalignedSelectionCode {
        exe: &'static str,
        actual: usize,
    },
    InvalidExpectedDigest {
        finding_id: String,
        value: String,
    },
    SourceDigestMismatch {
        finding_id: String,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    DuplicatePhysicalSource {
        exe: &'static str,
        archive: &'static str,
        asset_name: String,
    },
    InconsistentExecutableBoundary(&'static str),
    CoordinateOutOfRange(u32),
    NoMatchingExecutable,
    AmbiguousMatchingExecutables(Vec<&'static str>),
    MultipleSelectedPendingDescriptors {
        exe: &'static str,
        findings: Vec<String>,
    },
    VramLayout(RuntimeLayoutError),
}

impl fmt::Display for PendingBtxRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongRamLength { expected, actual } => write!(
                f,
                "pending BTX RAM 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::WrongVramLength { expected, actual } => write!(
                f,
                "pending BTX VRAM 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::EmptySources => f.write_str("pending BTX runtime source가 비어 있음"),
            Self::EmptyIdentity(field) => write!(f, "pending BTX {field}가 비어 있음"),
            Self::WrongDescriptorIdentityLength {
                exe,
                finding_id,
                expected,
                actual,
            } => write!(
                f,
                "{exe}:{finding_id} descriptor identity 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::ShortSelectionCode {
                exe,
                expected_at_least,
                actual,
            } => write!(
                f,
                "{exe} selection code window가 너무 짧음: expected>={expected_at_least}, actual={actual}"
            ),
            Self::UnalignedSelectionCode { exe, actual } => {
                write!(f, "{exe} selection code window가 4바이트 정렬이 아님: {actual}")
            }
            Self::InvalidExpectedDigest { finding_id, value } => write!(
                f,
                "{finding_id} expected SHA-256 형식 불일치: {value}"
            ),
            Self::SourceDigestMismatch {
                finding_id,
                expected,
                actual,
            } => write!(
                f,
                "{finding_id} 해제 BTX SHA-256 불일치: expected={}, actual={}",
                hex(expected),
                hex(actual)
            ),
            Self::DuplicatePhysicalSource {
                exe,
                archive,
                asset_name,
            } => write!(
                f,
                "pending BTX 물리 source 중복: {exe}:{archive}:{asset_name}"
            ),
            Self::InconsistentExecutableBoundary(exe) => {
                write!(f, "{exe} pending BTX runtime 실행 경계가 행마다 다름")
            }
            Self::CoordinateOutOfRange(runtime) => {
                write!(f, "pending BTX RAM 주소 범위 이탈: 0x{runtime:08X}")
            }
            Self::NoMatchingExecutable => {
                f.write_str("RAM과 selection code window가 일치하는 pending BTX EXE가 없음")
            }
            Self::AmbiguousMatchingExecutables(executables) => write!(
                f,
                "RAM과 selection code window가 여러 pending BTX EXE에 일치: {executables:?}"
            ),
            Self::MultipleSelectedPendingDescriptors { exe, findings } => write!(
                f,
                "{exe} working descriptor와 여러 pending source가 일치: {}",
                findings.join(",")
            ),
            Self::VramLayout(source) => write!(f, "pending BTX VRAM 대조 실패: {source}"),
        }
    }
}

impl Error for PendingBtxRuntimeError {}

impl From<RuntimeLayoutError> for PendingBtxRuntimeError {
    fn from(source: RuntimeLayoutError) -> Self {
        Self::VramLayout(source)
    }
}

#[derive(Debug, Clone, Copy)]
struct ExecutableBoundary<'a> {
    selection_code_runtime: u32,
    selection_code: &'a [u8],
    working_descriptor_runtime: u32,
}

fn runtime_slice(ram: &[u8], runtime: u32, len: usize) -> Result<&[u8], PendingBtxRuntimeError> {
    let offset = psx_main_ram_offset(runtime, len)
        .ok_or(PendingBtxRuntimeError::CoordinateOutOfRange(runtime))?;
    Ok(&ram[offset..offset + len])
}

fn parse_sha256(finding_id: &str, value: &str) -> Result<[u8; 32], PendingBtxRuntimeError> {
    if value.len() != 64 {
        return Err(PendingBtxRuntimeError::InvalidExpectedDigest {
            finding_id: finding_id.to_string(),
            value: value.to_string(),
        });
    }
    let mut bytes = [0u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let digits = std::str::from_utf8(chunk).map_err(|_| {
            PendingBtxRuntimeError::InvalidExpectedDigest {
                finding_id: finding_id.to_string(),
                value: value.to_string(),
            }
        })?;
        bytes[index] = u8::from_str_radix(digits, 16).map_err(|_| {
            PendingBtxRuntimeError::InvalidExpectedDigest {
                finding_id: finding_id.to_string(),
                value: value.to_string(),
            }
        })?;
    }
    Ok(bytes)
}

fn validate_source(source: PendingBtxRuntimeSource<'_>) -> Result<(), PendingBtxRuntimeError> {
    for (field, value) in [
        ("finding ID", source.finding_id),
        ("EXE", source.exe),
        ("archive", source.archive),
        ("asset name", source.asset_name),
    ] {
        if value.is_empty() {
            return Err(PendingBtxRuntimeError::EmptyIdentity(field));
        }
    }
    if source.original_source_descriptor_identity.len() != BTX_DESCRIPTOR_IDENTITY_BYTES {
        return Err(PendingBtxRuntimeError::WrongDescriptorIdentityLength {
            exe: source.exe,
            finding_id: source.finding_id.to_string(),
            expected: BTX_DESCRIPTOR_IDENTITY_BYTES,
            actual: source.original_source_descriptor_identity.len(),
        });
    }
    if source.selection_code.len() < BTX_SELECTION_CODE_MIN_BYTES {
        return Err(PendingBtxRuntimeError::ShortSelectionCode {
            exe: source.exe,
            expected_at_least: BTX_SELECTION_CODE_MIN_BYTES,
            actual: source.selection_code.len(),
        });
    }
    if !source.selection_code.len().is_multiple_of(4) {
        return Err(PendingBtxRuntimeError::UnalignedSelectionCode {
            exe: source.exe,
            actual: source.selection_code.len(),
        });
    }
    for (runtime, len) in [
        (
            source.descriptor_entry_runtime,
            BTX_DESCRIPTOR_IDENTITY_BYTES,
        ),
        (
            source.working_descriptor_runtime,
            BTX_DESCRIPTOR_IDENTITY_BYTES,
        ),
        (source.selection_code_runtime, source.selection_code.len()),
    ] {
        if psx_main_ram_offset(runtime, len).is_none() {
            return Err(PendingBtxRuntimeError::CoordinateOutOfRange(runtime));
        }
    }
    let expected = parse_sha256(source.finding_id, source.expected_sha256)?;
    let actual: [u8; 32] = Sha256::digest(source.raw).into();
    if actual != expected {
        return Err(PendingBtxRuntimeError::SourceDigestMismatch {
            finding_id: source.finding_id.to_string(),
            expected,
            actual,
        });
    }
    Ok(())
}

pub fn inspect_pending_btx_runtime<'a>(
    ram: &[u8],
    vram: &[u8],
    sources: impl IntoIterator<Item = PendingBtxRuntimeSource<'a>>,
) -> Result<PendingBtxRuntimeObservation, PendingBtxRuntimeError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(PendingBtxRuntimeError::WrongRamLength {
            expected: PSX_MAIN_RAM_LEN,
            actual: ram.len(),
        });
    }
    if vram.len() != VRAM_BYTE_LEN {
        return Err(PendingBtxRuntimeError::WrongVramLength {
            expected: VRAM_BYTE_LEN,
            actual: vram.len(),
        });
    }
    let sources = sources.into_iter().collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(PendingBtxRuntimeError::EmptySources);
    }

    let mut physical = BTreeSet::new();
    let mut boundaries = BTreeMap::<&'static str, ExecutableBoundary<'a>>::new();
    for &source in &sources {
        validate_source(source)?;
        if !physical.insert((source.exe, source.archive, source.asset_name)) {
            return Err(PendingBtxRuntimeError::DuplicatePhysicalSource {
                exe: source.exe,
                archive: source.archive,
                asset_name: source.asset_name.to_string(),
            });
        }
        let boundary = ExecutableBoundary {
            selection_code_runtime: source.selection_code_runtime,
            selection_code: source.selection_code,
            working_descriptor_runtime: source.working_descriptor_runtime,
        };
        match boundaries.get(source.exe) {
            Some(existing)
                if existing.selection_code_runtime != boundary.selection_code_runtime
                    || existing.selection_code != boundary.selection_code
                    || existing.working_descriptor_runtime
                        != boundary.working_descriptor_runtime =>
            {
                return Err(PendingBtxRuntimeError::InconsistentExecutableBoundary(
                    source.exe,
                ));
            }
            Some(_) => {}
            None => {
                boundaries.insert(source.exe, boundary);
            }
        }
    }

    let mut matching_executables = Vec::new();
    for (&exe, boundary) in &boundaries {
        let actual = runtime_slice(
            ram,
            boundary.selection_code_runtime,
            boundary.selection_code.len(),
        )?;
        if actual == boundary.selection_code {
            matching_executables.push(exe);
        }
    }
    let exe = match matching_executables.as_slice() {
        [exe] => *exe,
        [] => return Err(PendingBtxRuntimeError::NoMatchingExecutable),
        many => {
            return Err(PendingBtxRuntimeError::AmbiguousMatchingExecutables(
                many.to_vec(),
            ))
        }
    };
    let boundary = boundaries
        .get(exe)
        .expect("일치한 EXE는 boundary map에 있음");
    let working_identity = runtime_slice(
        ram,
        boundary.working_descriptor_runtime,
        BTX_DESCRIPTOR_IDENTITY_BYTES,
    )?;

    let active_sources = sources
        .iter()
        .copied()
        .filter(|source| source.exe == exe)
        .collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(active_sources.len());
    for source in active_sources {
        let descriptor_identity = runtime_slice(
            ram,
            source.descriptor_entry_runtime,
            BTX_DESCRIPTOR_IDENTITY_BYTES,
        )?;
        let source_descriptor_populated = descriptor_identity.iter().any(|&byte| byte != 0);
        let source_descriptor_original_match =
            descriptor_identity == source.original_source_descriptor_identity;
        let descriptor_selected =
            source_descriptor_populated && descriptor_identity == working_identity;
        let mut placements = exact_vram_placements(source.raw, source.format, vram)?;
        placements.sort_by_key(|placement| {
            (
                placement.layout.width,
                placement.layout.height,
                placement.x_words,
                placement.y,
            )
        });
        let vram_layouts = placements
            .iter()
            .map(|placement| (placement.layout.width, placement.layout.height))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        rows.push(PendingBtxRuntimeRow {
            finding_id: source.finding_id.to_string(),
            exe,
            archive: source.archive,
            asset_name: source.asset_name.to_string(),
            asset_sha256: Sha256::digest(source.raw).into(),
            format: source.format,
            descriptor_entry_runtime: source.descriptor_entry_runtime,
            working_descriptor_runtime: source.working_descriptor_runtime,
            source_descriptor_populated,
            source_descriptor_original_match,
            descriptor_selected,
            vram_layouts,
            vram_placements: placements,
        });
    }
    rows.sort_by(|left, right| {
        left.finding_id
            .cmp(&right.finding_id)
            .then_with(|| left.archive.cmp(right.archive))
    });
    let selected_findings = rows
        .iter()
        .filter(|row| row.descriptor_selected)
        .map(|row| row.finding_id.clone())
        .collect::<Vec<_>>();
    if selected_findings.len() > 1 {
        return Err(PendingBtxRuntimeError::MultipleSelectedPendingDescriptors {
            exe,
            findings: selected_findings,
        });
    }

    Ok(PendingBtxRuntimeObservation {
        exe,
        ram_sha256: Sha256::digest(ram).into(),
        vram_sha256: Sha256::digest(vram).into(),
        selection_code_runtime: boundary.selection_code_runtime,
        selection_code_bytes_verified: boundary.selection_code.len(),
        working_descriptor_runtime: boundary.working_descriptor_runtime,
        rows,
    })
}

pub fn render_pending_btx_runtime_tsv(observation: &PendingBtxRuntimeObservation) -> String {
    let mut output = String::from(
        "schema_version\texe\tram_sha256\tvram_sha256\tselection_code_runtime\t\
         selection_code_bytes_verified\tfinding_id\tarchive\tasset_name\tasset_sha256\t\
         pixel_format\tdescriptor_entry_runtime\tworking_descriptor_runtime\t\
         source_descriptor_populated\tsource_descriptor_original_match\t\
         descriptor_selected\tvram_layouts\t\
         vram_placements\tobservation_status\tclaim_boundary\n",
    );
    for row in &observation.rows {
        let layouts = row
            .vram_layouts
            .iter()
            .map(|(width, height)| format!("{width}x{height}"))
            .collect::<Vec<_>>()
            .join(",");
        let placements = row
            .vram_placements
            .iter()
            .map(|placement| {
                format!(
                    "{}x{}@{},{}",
                    placement.layout.width, placement.layout.height, placement.x_words, placement.y
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let format = match row.format {
            PixelFormat::Indexed4 => "indexed4",
            PixelFormat::Indexed8 => "indexed8",
        };
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t\
             0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             raw-ram-vram-snapshot-only-load-hit-primitive-draw-screen-unverified",
            BTX_PENDING_RUNTIME_SCHEMA_VERSION,
            observation.exe,
            hex(&observation.ram_sha256),
            hex(&observation.vram_sha256),
            observation.selection_code_runtime,
            observation.selection_code_bytes_verified,
            row.finding_id,
            row.archive,
            row.asset_name,
            hex(&row.asset_sha256),
            format,
            row.descriptor_entry_runtime,
            row.working_descriptor_runtime,
            row.source_descriptor_populated,
            row.source_descriptor_original_match,
            row.descriptor_selected,
            layouts,
            placements,
            row.observation_status(),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "btx_pending_tests.rs"]
mod tests;
