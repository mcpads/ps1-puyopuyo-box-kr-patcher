//! Mode2 Form1 위 ISO 9660 파일시스템의 엄격한 읽기 경로.
//!
//! dual-endian 필드, extent 범위, 디렉터리 순환과 multi-extent 종결을
//! 검증한다. 파일 경로는 디스크의 `;1` version을 보존한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::source::FORM1_DATA_LEN;

use super::image::{DiscImage, ImageError};

const PVD_LBA: u32 = 16;
const DIRECTORY_FLAG: u8 = 0x02;
const MULTI_EXTENT_FLAG: u8 = 0x80;

/// ISO 파일을 구성하는 연속 논리 extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    pub lba: u32,
    pub len: u32,
}

/// PVD metadata·directory·파일 allocation보다 뒤에 있는 Form1 volume tail.
///
/// 생성자는 공개하지 않는다. [`Iso9660::form1_volume_tail`]의 전체 allocation
/// 검증을 통과한 결과만 이 타입이 된다.
#[derive(Debug, Clone, Copy)]
pub struct VolumeTail<'a> {
    image: &'a DiscImage,
    extent: Extent,
}

impl PartialEq for VolumeTail<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.image, other.image) && self.extent == other.extent
    }
}

impl Eq for VolumeTail<'_> {}

impl<'a> VolumeTail<'a> {
    /// tail의 논리 LBA와 바이트 길이.
    pub fn extent(&self) -> Extent {
        self.extent
    }

    pub(super) fn from_extent(image: &'a DiscImage, extent: Extent) -> Self {
        Self { image, extent }
    }

    pub(super) fn image(&self) -> &'a DiscImage {
        self.image
    }
}

/// 한 extent를 선언한 ISO directory record의 Form1 사용자 데이터 위치.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectoryRecordLocation {
    pub lba: u32,
    pub user_data_offset: usize,
    pub record_len: usize,
}

/// ISO 디렉터리 또는 파일.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsoEntry {
    path: String,
    is_directory: bool,
    extents: Vec<Extent>,
    record_locations: Vec<DirectoryRecordLocation>,
}

impl IsoEntry {
    /// 루트 기준 절대 ISO 경로. 파일 version(`;1`)을 보존한다.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// 디렉터리 여부.
    pub fn is_directory(&self) -> bool {
        self.is_directory
    }

    /// multi-extent를 포함한 논리 조각.
    pub fn extents(&self) -> &[Extent] {
        &self.extents
    }

    /// 각 extent를 선언한 directory record 위치. extent와 같은 순서다.
    pub fn record_locations(&self) -> &[DirectoryRecordLocation] {
        &self.record_locations
    }

    /// 모든 extent의 논리 길이 합.
    pub fn len(&self) -> u64 {
        self.extents
            .iter()
            .map(|extent| u64::from(extent.len))
            .sum()
    }

    /// 길이가 0인가.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// ISO 9660 파싱·읽기 오류.
#[derive(Debug)]
pub enum IsoError {
    Image(ImageError),
    InvalidPvd,
    InvalidVolumeDescriptor(u32),
    MissingVolumeDescriptorTerminator,
    UnsupportedBlockSize(u16),
    TruncatedRecord,
    InvalidRecordLength(usize),
    DualEndianMismatch {
        field: &'static str,
        offset: usize,
    },
    InvalidIdentifier,
    InterleavedRecord(String),
    DirectoryRecordOutOfExtent,
    DirectoryRecordCrossesSector {
        lba: u32,
        user_data_offset: usize,
        record_len: usize,
    },
    ExtentOverflow,
    ExtentOutsideVolume {
        owner: String,
        extent: Extent,
        volume_sectors: u32,
    },
    EmptyExtent(String),
    UnterminatedMultiExtent(String),
    MultiExtentMismatch(String),
    DirectoryCycle(String),
    DuplicatePath(String),
    NotFound(String),
    IsDirectory(String),
    UnexpectedFileExtents {
        path: String,
        expected: Extent,
        actual: Vec<Extent>,
    },
}

impl fmt::Display for IsoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(error) => write!(f, "ISO 이미지 읽기 실패: {error}"),
            Self::InvalidPvd => write!(f, "ISO 9660 Primary Volume Descriptor 불일치"),
            Self::InvalidVolumeDescriptor(lba) => {
                write!(f, "ISO volume descriptor 불일치: LBA {lba}")
            }
            Self::MissingVolumeDescriptorTerminator => {
                write!(f, "ISO volume descriptor terminator가 없음")
            }
            Self::UnsupportedBlockSize(size) => {
                write!(f, "지원하지 않는 ISO 논리 블록 크기: {size}")
            }
            Self::TruncatedRecord => write!(f, "잘린 ISO directory record"),
            Self::InvalidRecordLength(length) => {
                write!(f, "잘못된 ISO directory record 길이: {length}")
            }
            Self::DualEndianMismatch { field, offset } => {
                write!(f, "ISO dual-endian {field} 불일치: offset=0x{offset:x}")
            }
            Self::InvalidIdentifier => write!(f, "지원하지 않는 ISO identifier"),
            Self::InterleavedRecord(path) => {
                write!(f, "지원하지 않는 ISO interleaved record: {path}")
            }
            Self::DirectoryRecordOutOfExtent => {
                write!(f, "ISO directory record가 부모 directory extent 밖임")
            }
            Self::DirectoryRecordCrossesSector {
                lba,
                user_data_offset,
                record_len,
            } => write!(
                f,
                "ISO directory record가 Form1 sector를 가로지름: LBA {lba}, \
                 offset={user_data_offset}, len={record_len}"
            ),
            Self::ExtentOverflow => write!(f, "ISO extent LBA 계산 overflow"),
            Self::ExtentOutsideVolume {
                owner,
                extent,
                volume_sectors,
            } => write!(
                f,
                "ISO allocation이 volume 밖임: {owner}, extent={extent:?}, volume_sectors={volume_sectors}"
            ),
            Self::EmptyExtent(path) => write!(f, "ISO extent 길이가 0: {path}"),
            Self::UnterminatedMultiExtent(path) => {
                write!(f, "종결 record가 없는 ISO multi-extent: {path}")
            }
            Self::MultiExtentMismatch(path) => {
                write!(f, "연속 ISO multi-extent record 불일치: {path}")
            }
            Self::DirectoryCycle(path) => write!(f, "ISO 디렉터리 순환: {path}"),
            Self::DuplicatePath(path) => write!(f, "ISO 경로 중복: {path}"),
            Self::NotFound(path) => write!(f, "ISO 경로를 찾지 못함: {path}"),
            Self::IsDirectory(path) => write!(f, "ISO 파일 대신 디렉터리임: {path}"),
            Self::UnexpectedFileExtents {
                path,
                expected,
                actual,
            } => write!(
                f,
                "ISO 파일 extent 불일치: {path}, expected={expected:?}, actual={actual:?}"
            ),
        }
    }
}

impl Error for IsoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Image(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ImageError> for IsoError {
    fn from(value: ImageError) -> Self {
        Self::Image(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Identifier {
    Current,
    Parent,
    Named(String),
}

#[derive(Debug, Clone)]
struct DirectoryRecord {
    identifier: Identifier,
    extent: Extent,
    is_directory: bool,
    continues: bool,
    location: DirectoryRecordLocation,
}

/// 파싱된 ISO 9660 파일시스템.
#[derive(Debug)]
pub struct Iso9660<'a> {
    image: &'a DiscImage,
    volume_id: String,
    volume_sectors: u32,
    allocated_end_lba: u32,
    entries: Vec<IsoEntry>,
}

impl<'a> Iso9660<'a> {
    /// PVD부터 모든 디렉터리를 재귀 열거한다.
    pub fn parse(image: &'a DiscImage) -> Result<Self, IsoError> {
        let pvd = image.form1_data(PVD_LBA)?;
        if pvd[0] != 1 || &pvd[1..6] != b"CD001" || pvd[6] != 1 {
            return Err(IsoError::InvalidPvd);
        }
        let volume_sectors = read_u32_both(pvd, 80, "volume space size")?;
        let block_size = read_u16_both(pvd, 128, "logical block size")?;
        if usize::from(block_size) != FORM1_DATA_LEN {
            return Err(IsoError::UnsupportedBlockSize(block_size));
        }
        let volume_id = parse_volume_id(&pvd[40..72])?;
        let root_length = usize::from(pvd[156]);
        if root_length == 0 || 156 + root_length > pvd.len() {
            return Err(IsoError::TruncatedRecord);
        }
        let root_record = parse_record(&pvd[156..156 + root_length])?;
        if !root_record.is_directory {
            return Err(IsoError::InvalidPvd);
        }
        let root = IsoEntry {
            path: "/".to_string(),
            is_directory: true,
            extents: vec![root_record.extent],
            record_locations: vec![DirectoryRecordLocation {
                lba: PVD_LBA,
                user_data_offset: 156,
                record_len: root_length,
            }],
        };
        let mut allocated_end_lba = volume_descriptor_end_lba(image, volume_sectors)?;
        allocated_end_lba =
            allocated_end_lba.max(checked_extent_end("/", root_record.extent, volume_sectors)?);
        let path_table_size = read_u32_both(pvd, 132, "path table size")?;
        if path_table_size != 0 {
            let path_table_locations = [
                read_u32_le(pvd, 140)?,
                read_u32_le(pvd, 144)?,
                read_u32_be(pvd, 148)?,
                read_u32_be(pvd, 152)?,
            ];
            for lba in path_table_locations.into_iter().filter(|lba| *lba != 0) {
                allocated_end_lba = allocated_end_lba.max(checked_extent_end(
                    "PVD path table",
                    Extent {
                        lba,
                        len: path_table_size,
                    },
                    volume_sectors,
                )?);
            }
        }
        let mut iso = Self {
            image,
            volume_id,
            volume_sectors,
            allocated_end_lba,
            entries: Vec::new(),
        };
        let mut visited = BTreeSet::new();
        iso.walk_directory(&root, &mut visited)?;
        iso.entries
            .sort_by(|left, right| left.path.cmp(&right.path));
        for pair in iso.entries.windows(2) {
            if pair[0].path == pair[1].path {
                return Err(IsoError::DuplicatePath(pair[0].path.clone()));
            }
        }
        for entry in &iso.entries {
            for &extent in &entry.extents {
                iso.allocated_end_lba = iso.allocated_end_lba.max(checked_extent_end(
                    entry.path(),
                    extent,
                    volume_sectors,
                )?);
            }
        }
        Ok(iso)
    }

    /// PVD의 volume identifier.
    pub fn volume_id(&self) -> &str {
        &self.volume_id
    }

    /// PVD가 선언한 논리 블록 수.
    pub fn volume_sectors(&self) -> u32 {
        self.volume_sectors
    }

    /// 루트를 제외한 전체 디렉터리·파일 목록.
    pub fn entries(&self) -> &[IsoEntry] {
        &self.entries
    }

    /// 알려진 모든 ISO allocation 뒤의 volume tail이 전부 Form1이면 반환한다.
    ///
    /// 파일 extent뿐 아니라 PVD, root directory와 PVD가 선언한 네 path table
    /// 위치를 high-water 계산에 포함한다. 중간 hole은 이 보수적 경계가
    /// allocation 대상으로 승격하지 않는다.
    pub fn form1_volume_tail(&self) -> Result<Option<VolumeTail<'a>>, IsoError> {
        if self.allocated_end_lba >= self.volume_sectors {
            return Ok(None);
        }
        for lba in self.allocated_end_lba..self.volume_sectors {
            self.image.form1_data(lba)?;
        }
        let sectors = self.volume_sectors - self.allocated_end_lba;
        let len = sectors
            .checked_mul(FORM1_DATA_LEN as u32)
            .ok_or(IsoError::ExtentOverflow)?;
        Ok(Some(VolumeTail::from_extent(
            self.image,
            Extent {
                lba: self.allocated_end_lba,
                len,
            },
        )))
    }

    /// version을 포함한 정확한 절대 경로를 찾는다.
    pub fn find(&self, path: &str) -> Result<&IsoEntry, IsoError> {
        self.entries
            .binary_search_by(|entry| entry.path.as_str().cmp(path))
            .map(|index| &self.entries[index])
            .map_err(|_| IsoError::NotFound(path.to_string()))
    }

    /// 한 ISO 파일의 multi-extent를 논리 순서로 합쳐 읽는다.
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, IsoError> {
        let entry = self.find(path)?;
        if entry.is_directory {
            return Err(IsoError::IsDirectory(path.to_string()));
        }
        self.read_entry(entry)
    }

    /// 파일이 기대한 단일 extent를 가리키는지 검증한다.
    pub fn verify_single_extent_file(
        &self,
        path: &str,
        expected: Extent,
    ) -> Result<&IsoEntry, IsoError> {
        let entry = self.find(path)?;
        if entry.is_directory {
            return Err(IsoError::IsDirectory(path.to_string()));
        }
        if entry.extents.as_slice() != [expected] {
            return Err(IsoError::UnexpectedFileExtents {
                path: path.to_string(),
                expected,
                actual: entry.extents.clone(),
            });
        }
        Ok(entry)
    }

    /// 기대한 단일 extent를 먼저 검증한 뒤 파일 payload를 읽는다.
    pub fn read_file_at_extent(&self, path: &str, expected: Extent) -> Result<Vec<u8>, IsoError> {
        let entry = self.verify_single_extent_file(path, expected)?;
        self.read_entry(entry)
    }

    fn walk_directory(
        &mut self,
        directory: &IsoEntry,
        visited: &mut BTreeSet<u32>,
    ) -> Result<(), IsoError> {
        let first_lba = directory.extents[0].lba;
        if !visited.insert(first_lba) {
            return Err(IsoError::DirectoryCycle(directory.path.clone()));
        }
        let raw = self.read_entry(directory)?;
        let records = parse_directory_records(&raw, directory)?;
        for entry in group_records(records, &directory.path)? {
            if entry.is_directory {
                self.walk_directory(&entry, visited)?;
            }
            self.entries.push(entry);
        }
        Ok(())
    }

    fn read_entry(&self, entry: &IsoEntry) -> Result<Vec<u8>, IsoError> {
        let capacity = usize::try_from(entry.len()).map_err(|_| IsoError::ExtentOverflow)?;
        let mut output = Vec::with_capacity(capacity);
        for extent in &entry.extents {
            if extent.len == 0 {
                return Err(IsoError::EmptyExtent(entry.path.clone()));
            }
            let mut remaining = extent.len as usize;
            let mut lba = extent.lba;
            while remaining > 0 {
                let sector = self.image.form1_data(lba)?;
                let take = remaining.min(FORM1_DATA_LEN);
                output.extend_from_slice(&sector[..take]);
                remaining -= take;
                lba = lba.checked_add(1).ok_or(IsoError::ExtentOverflow)?;
            }
        }
        Ok(output)
    }
}

fn parse_volume_id(raw: &[u8]) -> Result<String, IsoError> {
    let end = raw
        .iter()
        .rposition(|&byte| byte != b' ')
        .map_or(0, |index| index + 1);
    if end == 0 || !raw[..end].iter().all(u8::is_ascii_graphic) {
        return Err(IsoError::InvalidIdentifier);
    }
    std::str::from_utf8(&raw[..end])
        .map(str::to_string)
        .map_err(|_| IsoError::InvalidIdentifier)
}

fn volume_descriptor_end_lba(image: &DiscImage, volume_sectors: u32) -> Result<u32, IsoError> {
    let mut lba = PVD_LBA;
    while lba < volume_sectors {
        let descriptor = image.form1_data(lba)?;
        if &descriptor[1..6] != b"CD001" || descriptor[6] != 1 {
            return Err(IsoError::InvalidVolumeDescriptor(lba));
        }
        if descriptor[0] == 255 {
            return lba.checked_add(1).ok_or(IsoError::ExtentOverflow);
        }
        lba = lba.checked_add(1).ok_or(IsoError::ExtentOverflow)?;
    }
    Err(IsoError::MissingVolumeDescriptorTerminator)
}

fn read_u16_both(raw: &[u8], offset: usize, field: &'static str) -> Result<u16, IsoError> {
    if offset + 4 > raw.len() {
        return Err(IsoError::TruncatedRecord);
    }
    let little = u16::from_le_bytes([raw[offset], raw[offset + 1]]);
    let big = u16::from_be_bytes([raw[offset + 2], raw[offset + 3]]);
    if little != big {
        return Err(IsoError::DualEndianMismatch { field, offset });
    }
    Ok(little)
}

fn read_u32_both(raw: &[u8], offset: usize, field: &'static str) -> Result<u32, IsoError> {
    if offset + 8 > raw.len() {
        return Err(IsoError::TruncatedRecord);
    }
    let little = u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap());
    let big = u32::from_be_bytes(raw[offset + 4..offset + 8].try_into().unwrap());
    if little != big {
        return Err(IsoError::DualEndianMismatch { field, offset });
    }
    Ok(little)
}

fn read_u32_le(raw: &[u8], offset: usize) -> Result<u32, IsoError> {
    let bytes = raw
        .get(offset..offset + 4)
        .ok_or(IsoError::TruncatedRecord)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32_be(raw: &[u8], offset: usize) -> Result<u32, IsoError> {
    let bytes = raw
        .get(offset..offset + 4)
        .ok_or(IsoError::TruncatedRecord)?;
    Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
}

fn checked_extent_end(owner: &str, extent: Extent, volume_sectors: u32) -> Result<u32, IsoError> {
    if extent.len == 0 {
        return Err(IsoError::EmptyExtent(owner.to_string()));
    }
    let sectors = extent.len.div_ceil(FORM1_DATA_LEN as u32);
    let end = extent
        .lba
        .checked_add(sectors)
        .ok_or(IsoError::ExtentOverflow)?;
    if end > volume_sectors {
        return Err(IsoError::ExtentOutsideVolume {
            owner: owner.to_string(),
            extent,
            volume_sectors,
        });
    }
    Ok(end)
}

fn parse_record(raw: &[u8]) -> Result<DirectoryRecord, IsoError> {
    if raw.len() < 34 {
        return Err(IsoError::TruncatedRecord);
    }
    let record_length = usize::from(raw[0]);
    if record_length < 34 || record_length > raw.len() {
        return Err(IsoError::InvalidRecordLength(record_length));
    }
    if raw[26] != 0 || raw[27] != 0 {
        return Err(IsoError::InterleavedRecord("<record>".to_string()));
    }
    let name_length = usize::from(raw[32]);
    let name_end = 33usize
        .checked_add(name_length)
        .ok_or(IsoError::TruncatedRecord)?;
    if name_end > record_length {
        return Err(IsoError::TruncatedRecord);
    }
    let identifier = match &raw[33..name_end] {
        [0] => Identifier::Current,
        [1] => Identifier::Parent,
        name if !name.is_empty() && name.iter().all(u8::is_ascii) => Identifier::Named(
            std::str::from_utf8(name)
                .map_err(|_| IsoError::InvalidIdentifier)?
                .to_string(),
        ),
        _ => return Err(IsoError::InvalidIdentifier),
    };
    let lba = read_u32_both(raw, 2, "extent LBA")?
        .checked_add(u32::from(raw[1]))
        .ok_or(IsoError::ExtentOverflow)?;
    let len = read_u32_both(raw, 10, "extent length")?;
    let _volume_sequence = read_u16_both(raw, 28, "volume sequence")?;
    Ok(DirectoryRecord {
        identifier,
        extent: Extent { lba, len },
        is_directory: raw[25] & DIRECTORY_FLAG != 0,
        continues: raw[25] & MULTI_EXTENT_FLAG != 0,
        location: DirectoryRecordLocation {
            lba: 0,
            user_data_offset: 0,
            record_len: record_length,
        },
    })
}

fn locate_directory_record(
    directory: &IsoEntry,
    file_offset: usize,
    record_len: usize,
) -> Result<DirectoryRecordLocation, IsoError> {
    let mut remaining = file_offset;
    for extent in &directory.extents {
        let extent_len = usize::try_from(extent.len).map_err(|_| IsoError::ExtentOverflow)?;
        if remaining >= extent_len {
            remaining -= extent_len;
            continue;
        }
        let sector_delta = remaining / FORM1_DATA_LEN;
        let user_data_offset = remaining % FORM1_DATA_LEN;
        let lba = extent
            .lba
            .checked_add(u32::try_from(sector_delta).map_err(|_| IsoError::ExtentOverflow)?)
            .ok_or(IsoError::ExtentOverflow)?;
        let record_end_in_sector = user_data_offset
            .checked_add(record_len)
            .ok_or(IsoError::ExtentOverflow)?;
        if record_end_in_sector > FORM1_DATA_LEN {
            return Err(IsoError::DirectoryRecordCrossesSector {
                lba,
                user_data_offset,
                record_len,
            });
        }
        let record_end_in_extent = remaining
            .checked_add(record_len)
            .ok_or(IsoError::ExtentOverflow)?;
        if record_end_in_extent > extent_len {
            return Err(IsoError::DirectoryRecordOutOfExtent);
        }
        return Ok(DirectoryRecordLocation {
            lba,
            user_data_offset,
            record_len,
        });
    }
    Err(IsoError::DirectoryRecordOutOfExtent)
}

fn parse_directory_records(
    raw: &[u8],
    directory: &IsoEntry,
) -> Result<Vec<DirectoryRecord>, IsoError> {
    let mut records = Vec::new();
    let mut offset = 0usize;
    while offset < raw.len() {
        let length = usize::from(raw[offset]);
        if length == 0 {
            offset = (offset / FORM1_DATA_LEN + 1) * FORM1_DATA_LEN;
            continue;
        }
        if offset + length > raw.len() {
            return Err(IsoError::TruncatedRecord);
        }
        let mut record = parse_record(&raw[offset..offset + length])?;
        record.location = locate_directory_record(directory, offset, length)?;
        records.push(record);
        offset += length;
    }
    Ok(records)
}

fn group_records(records: Vec<DirectoryRecord>, parent: &str) -> Result<Vec<IsoEntry>, IsoError> {
    let mut output = Vec::new();
    let mut pending: Option<IsoEntry> = None;
    for record in records {
        let name = match record.identifier {
            Identifier::Current | Identifier::Parent => {
                if pending.is_some() {
                    return Err(IsoError::MultiExtentMismatch(parent.to_string()));
                }
                continue;
            }
            Identifier::Named(name) => name,
        };
        let path = if parent == "/" {
            format!("/{name}")
        } else {
            format!("{parent}/{name}")
        };
        if let Some(mut entry) = pending.take() {
            if entry.path != path || entry.is_directory != record.is_directory {
                return Err(IsoError::MultiExtentMismatch(entry.path));
            }
            entry.extents.push(record.extent);
            entry.record_locations.push(record.location);
            if record.continues {
                pending = Some(entry);
            } else {
                output.push(entry);
            }
        } else {
            let entry = IsoEntry {
                path,
                is_directory: record.is_directory,
                extents: vec![record.extent],
                record_locations: vec![record.location],
            };
            if record.continues {
                pending = Some(entry);
            } else {
                output.push(entry);
            }
        }
    }
    if let Some(entry) = pending {
        return Err(IsoError::UnterminatedMultiExtent(entry.path));
    }
    Ok(output)
}

#[cfg(test)]
#[path = "iso9660_tests.rs"]
pub(crate) mod tests;
