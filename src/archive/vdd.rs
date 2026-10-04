//! VDD 아카이브의 lossless 파싱·직렬화.
//!
//! VDD는 `u32 LE` 엔트리 수 뒤에 24바이트 엔트리 표가 있고, 각 파일은
//! 2 KiB 섹터 오프셋으로 지정된다. 미해석 패딩·간격·꼬리는 원본 바이트를
//! 보존한다. 따라서 무수정 직렬화는 구조 필드를 다시 기록하되 그 밖의
//! 바이트를 정규화하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::ops::Range;

const ENTRY_LEN: usize = 24;
const NAME_LEN: usize = 16;
const SECTOR_LEN: usize = 2048;

/// VDD 파일 엔트리.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VddEntry {
    name: String,
    raw_name: [u8; NAME_LEN],
    pub sector_offset: u32,
    pub size: u32,
}

impl VddEntry {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn byte_offset(&self) -> usize {
        self.sector_offset as usize * SECTOR_LEN
    }

    pub fn range(&self) -> Range<usize> {
        let start = self.byte_offset();
        start..start + self.size as usize
    }
}

/// VDD 파싱·재조립 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VddError {
    TooShort(usize),
    EmptyArchive,
    TableOverflow,
    TruncatedTable {
        declared_entries: usize,
        available_bytes: usize,
    },
    EmptyName {
        index: usize,
    },
    InvalidNameByte {
        index: usize,
        offset: usize,
        value: u8,
    },
    NonZeroNamePadding {
        index: usize,
        offset: usize,
        value: u8,
    },
    DuplicateName(String),
    EntryBeforeData {
        name: String,
        offset: usize,
        data_start: usize,
    },
    EntryOutOfRange {
        name: String,
        offset: usize,
        size: usize,
        archive_len: usize,
    },
    OverlappingEntries {
        first: String,
        second: String,
    },
    UnknownReplacement(String),
    DuplicateReplacement(String),
    ReplacementDoesNotFit {
        name: String,
        replacement: usize,
        capacity: usize,
    },
    RepackedArchiveDoesNotFit {
        required: usize,
        capacity: usize,
    },
    SizeOverflow {
        field: &'static str,
        value: usize,
    },
}

impl fmt::Display for VddError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort(actual) => write!(f, "VDD 데이터 부족: {actual}B"),
            Self::EmptyArchive => write!(f, "VDD 엔트리 수가 0"),
            Self::TableOverflow => write!(f, "VDD 엔트리 표 크기 overflow"),
            Self::TruncatedTable {
                declared_entries,
                available_bytes,
            } => write!(
                f,
                "VDD 엔트리 표 부족: {declared_entries}개 선언, 전체 {available_bytes}B"
            ),
            Self::EmptyName { index } => write!(f, "VDD 엔트리 {index}의 이름이 비어 있음"),
            Self::InvalidNameByte {
                index,
                offset,
                value,
            } => write!(
                f,
                "VDD 엔트리 {index} 이름 +0x{offset:X}에 비 ASCII 바이트 0x{value:02X}"
            ),
            Self::NonZeroNamePadding {
                index,
                offset,
                value,
            } => write!(
                f,
                "VDD 엔트리 {index} 이름 NUL 뒤 +0x{offset:X}에 0x{value:02X}"
            ),
            Self::DuplicateName(name) => write!(f, "VDD 중복 파일명: {name}"),
            Self::EntryBeforeData {
                name,
                offset,
                data_start,
            } => write!(
                f,
                "VDD 엔트리가 데이터 시작 전을 가리킴: {name} 0x{offset:X} < 0x{data_start:X}"
            ),
            Self::EntryOutOfRange {
                name,
                offset,
                size,
                archive_len,
            } => write!(
                f,
                "VDD 엔트리 범위 초과: {name} offset=0x{offset:X} size={size} archive={archive_len}"
            ),
            Self::OverlappingEntries { first, second } => {
                write!(f, "VDD 엔트리 겹침: {first} / {second}")
            }
            Self::UnknownReplacement(name) => write!(f, "VDD에 없는 교체 파일: {name}"),
            Self::DuplicateReplacement(name) => {
                write!(f, "VDD 교체 파일명이 대소문자 무시 기준으로 중복: {name}")
            }
            Self::ReplacementDoesNotFit {
                name,
                replacement,
                capacity,
            } => write!(
                f,
                "VDD 제자리 교체 공간 부족: {name} replacement={replacement}, capacity={capacity}"
            ),
            Self::RepackedArchiveDoesNotFit { required, capacity } => write!(
                f,
                "VDD 고정 컨테이너 재패킹 공간 부족: required={required}, capacity={capacity}"
            ),
            Self::SizeOverflow { field, value } => {
                write!(f, "VDD {field}가 표현 범위를 초과: {value}")
            }
        }
    }
}

impl Error for VddError {}

/// 검증된 VDD 아카이브.
#[derive(Debug, Clone)]
pub struct VddArchive {
    entries: Vec<VddEntry>,
    raw: Vec<u8>,
}

impl VddArchive {
    /// VDD를 파싱하고 표·이름·범위·겹침을 검증한다.
    pub fn parse(data: &[u8]) -> Result<Self, VddError> {
        if data.len() < 4 {
            return Err(VddError::TooShort(data.len()));
        }
        let count = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
        if count == 0 {
            return Err(VddError::EmptyArchive);
        }
        let table_len = count
            .checked_mul(ENTRY_LEN)
            .and_then(|len| len.checked_add(4))
            .ok_or(VddError::TableOverflow)?;
        if table_len > data.len() {
            return Err(VddError::TruncatedTable {
                declared_entries: count,
                available_bytes: data.len(),
            });
        }
        let data_start = table_len.div_ceil(SECTOR_LEN) * SECTOR_LEN;

        let mut entries = Vec::with_capacity(count);
        let mut names = BTreeSet::new();
        for index in 0..count {
            let base = 4 + index * ENTRY_LEN;
            let raw_name: [u8; NAME_LEN] = data[base..base + NAME_LEN].try_into().unwrap();
            let name_end = raw_name
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(NAME_LEN);
            if name_end == 0 {
                return Err(VddError::EmptyName { index });
            }
            for (offset, &value) in raw_name[..name_end].iter().enumerate() {
                if !(0x20..=0x7E).contains(&value) {
                    return Err(VddError::InvalidNameByte {
                        index,
                        offset,
                        value,
                    });
                }
            }
            for (offset, &value) in raw_name[name_end..].iter().enumerate() {
                if value != 0 {
                    return Err(VddError::NonZeroNamePadding {
                        index,
                        offset: name_end + offset,
                        value,
                    });
                }
            }
            let name = String::from_utf8(raw_name[..name_end].to_vec()).unwrap();
            let canonical = name.to_ascii_uppercase();
            if !names.insert(canonical) {
                return Err(VddError::DuplicateName(name));
            }

            let sector_offset = u32::from_le_bytes(data[base + 16..base + 20].try_into().unwrap());
            let size = u32::from_le_bytes(data[base + 20..base + 24].try_into().unwrap());
            let offset = sector_offset as usize * SECTOR_LEN;
            if offset < data_start {
                return Err(VddError::EntryBeforeData {
                    name,
                    offset,
                    data_start,
                });
            }
            let end =
                offset
                    .checked_add(size as usize)
                    .ok_or_else(|| VddError::EntryOutOfRange {
                        name: name.clone(),
                        offset,
                        size: size as usize,
                        archive_len: data.len(),
                    })?;
            if end > data.len() {
                return Err(VddError::EntryOutOfRange {
                    name,
                    offset,
                    size: size as usize,
                    archive_len: data.len(),
                });
            }
            entries.push(VddEntry {
                name,
                raw_name,
                sector_offset,
                size,
            });
        }

        let mut ranges: Vec<_> = entries
            .iter()
            .filter(|entry| entry.size != 0)
            .map(|entry| (entry.range(), entry.name.clone()))
            .collect();
        ranges.sort_by_key(|(range, _)| range.start);
        for pair in ranges.windows(2) {
            if pair[0].0.end > pair[1].0.start {
                return Err(VddError::OverlappingEntries {
                    first: pair[0].1.clone(),
                    second: pair[1].1.clone(),
                });
            }
        }

        Ok(Self {
            entries,
            raw: data.to_vec(),
        })
    }

    pub fn entries(&self) -> &[VddEntry] {
        &self.entries
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn find(&self, name: &str) -> Option<&VddEntry> {
        self.entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
    }

    pub fn read_file(&self, entry: &VddEntry) -> &[u8] {
        &self.raw[entry.range()]
    }

    /// 구조 필드를 다시 쓰고 미해석 패딩·간격·꼬리를 보존한다.
    ///
    /// 무수정 아카이브에서는 원본과 바이트 동일해야 한다.
    pub fn serialize_lossless(&self) -> Vec<u8> {
        let mut output = self.raw.clone();
        output[..4].copy_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for (index, entry) in self.entries.iter().enumerate() {
            let base = 4 + index * ENTRY_LEN;
            output[base..base + NAME_LEN].copy_from_slice(&entry.raw_name);
            output[base + 16..base + 20].copy_from_slice(&entry.sector_offset.to_le_bytes());
            output[base + 20..base + 24].copy_from_slice(&entry.size.to_le_bytes());
        }
        output
    }

    /// 엔트리 offset과 VDD 전체 크기를 유지한 채 지정 파일만 교체한다.
    ///
    /// 다음 엔트리까지의 기존 공간 안에 들어갈 때만 허용한다. 짧아진 파일
    /// 뒤의 원본 padding은 보존하며, 크기 field와 새 payload 외에는
    /// 정규화하지 않는다.
    pub fn replace_in_place(
        &self,
        replacements: &BTreeMap<String, Vec<u8>>,
    ) -> Result<Vec<u8>, VddError> {
        if replacements.is_empty() {
            return Ok(self.serialize_lossless());
        }

        let mut canonical_names = BTreeSet::new();
        for name in replacements.keys() {
            let canonical = name.to_ascii_uppercase();
            if !canonical_names.insert(canonical) {
                return Err(VddError::DuplicateReplacement(name.clone()));
            }
            if self.find(name).is_none() {
                return Err(VddError::UnknownReplacement(name.clone()));
            }
        }

        let mut output = self.serialize_lossless();
        for (name, replacement) in replacements {
            let index = self
                .entries
                .iter()
                .position(|entry| entry.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| VddError::UnknownReplacement(name.clone()))?;
            let entry = &self.entries[index];
            let start = entry.byte_offset();
            let next_start = self
                .entries
                .iter()
                .enumerate()
                .filter(|(other_index, other)| {
                    *other_index != index && other.byte_offset() >= start
                })
                .map(|(_, other)| other.byte_offset())
                .min()
                .unwrap_or(self.raw.len());
            let capacity = next_start.saturating_sub(start);
            if replacement.len() > capacity {
                return Err(VddError::ReplacementDoesNotFit {
                    name: entry.name.clone(),
                    replacement: replacement.len(),
                    capacity,
                });
            }
            let size = u32::try_from(replacement.len()).map_err(|_| VddError::SizeOverflow {
                field: "엔트리 크기",
                value: replacement.len(),
            })?;
            let table_base = 4 + index * ENTRY_LEN;
            output[table_base + 20..table_base + 24].copy_from_slice(&size.to_le_bytes());
            output[start..start + replacement.len()].copy_from_slice(replacement);
        }

        // 수정 결과 자체도 같은 엄격한 parser를 다시 통과해야 한다.
        let reparsed = Self::parse(&output)?;
        for original in &self.entries {
            let rebuilt = reparsed
                .find(original.name())
                .ok_or_else(|| VddError::UnknownReplacement(original.name().to_string()))?;
            if rebuilt.sector_offset != original.sector_offset {
                return Err(VddError::EntryOutOfRange {
                    name: original.name.clone(),
                    offset: rebuilt.byte_offset(),
                    size: rebuilt.size as usize,
                    archive_len: output.len(),
                });
            }
        }
        Ok(output)
    }

    /// VDD 전체 크기를 유지하며 모든 엔트리를 앞에서부터 섹터 정렬해 재배치한다.
    ///
    /// 개별 엔트리의 기존 슬롯보다 replacement가 커도 전체 컨테이너 안에
    /// 들어가면 허용한다. 표와 새 payload 밖의 기존 바이트는 보존하므로
    /// 호출자는 그 두 범위만 Expected Write로 선언할 수 있다.
    pub fn repack_in_container(
        &self,
        replacements: &BTreeMap<String, Vec<u8>>,
    ) -> Result<Vec<u8>, VddError> {
        let mut canonical_names = BTreeSet::new();
        for name in replacements.keys() {
            let canonical = name.to_ascii_uppercase();
            if !canonical_names.insert(canonical) {
                return Err(VddError::DuplicateReplacement(name.clone()));
            }
            if self.find(name).is_none() {
                return Err(VddError::UnknownReplacement(name.clone()));
            }
        }

        let table_len = 4 + self.entries.len() * ENTRY_LEN;
        let mut current_sector = table_len.div_ceil(SECTOR_LEN);
        let mut output = self.raw.clone();
        for (index, entry) in self.entries.iter().enumerate() {
            let data = replacements
                .iter()
                .find(|(name, _)| entry.name.eq_ignore_ascii_case(name))
                .map_or_else(|| self.read_file(entry), |(_, data)| data.as_slice());
            let sector_offset =
                u32::try_from(current_sector).map_err(|_| VddError::SizeOverflow {
                    field: "섹터 오프셋",
                    value: current_sector,
                })?;
            let size = u32::try_from(data.len()).map_err(|_| VddError::SizeOverflow {
                field: "엔트리 크기",
                value: data.len(),
            })?;
            let start = current_sector
                .checked_mul(SECTOR_LEN)
                .ok_or(VddError::SizeOverflow {
                    field: "payload 오프셋",
                    value: current_sector,
                })?;
            let end = start
                .checked_add(data.len())
                .ok_or(VddError::SizeOverflow {
                    field: "payload 끝",
                    value: data.len(),
                })?;
            let next_sector = current_sector
                .checked_add(data.len().div_ceil(SECTOR_LEN).max(1))
                .ok_or(VddError::SizeOverflow {
                    field: "전체 섹터 수",
                    value: current_sector,
                })?;
            let required = next_sector
                .checked_mul(SECTOR_LEN)
                .ok_or(VddError::SizeOverflow {
                    field: "전체 크기",
                    value: next_sector,
                })?;
            if required > output.len() {
                return Err(VddError::RepackedArchiveDoesNotFit {
                    required,
                    capacity: output.len(),
                });
            }

            let base = 4 + index * ENTRY_LEN;
            output[base + 16..base + 20].copy_from_slice(&sector_offset.to_le_bytes());
            output[base + 20..base + 24].copy_from_slice(&size.to_le_bytes());
            output[start..end].copy_from_slice(data);
            current_sector = next_sector;
        }

        let reparsed = Self::parse(&output)?;
        if reparsed.entry_count() != self.entry_count() {
            return Err(VddError::TruncatedTable {
                declared_entries: self.entry_count(),
                available_bytes: output.len(),
            });
        }
        Ok(output)
    }

    /// 지정한 엔트리를 교체하고 전체 VDD를 섹터 정렬해 재조립한다.
    ///
    /// 교체가 없으면 lossless 직렬화를 사용해 원본 바이트를 그대로 보존한다.
    pub fn rebuild(&self, replacements: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, VddError> {
        if replacements.is_empty() {
            return Ok(self.serialize_lossless());
        }
        for name in replacements.keys() {
            if self.find(name).is_none() {
                return Err(VddError::UnknownReplacement(name.clone()));
            }
        }

        let table_len = 4 + self.entries.len() * ENTRY_LEN;
        let mut current_sector = table_len.div_ceil(SECTOR_LEN);
        let mut files = Vec::with_capacity(self.entries.len());
        let mut sectors = Vec::with_capacity(self.entries.len());

        for entry in &self.entries {
            let replacement = replacements
                .iter()
                .find(|(name, _)| entry.name.eq_ignore_ascii_case(name))
                .map(|(_, data)| data.clone());
            let data = replacement.unwrap_or_else(|| self.read_file(entry).to_vec());
            let sector = u32::try_from(current_sector).map_err(|_| VddError::SizeOverflow {
                field: "섹터 오프셋",
                value: current_sector,
            })?;
            sectors.push(sector);
            current_sector = current_sector
                .checked_add(data.len().div_ceil(SECTOR_LEN).max(1))
                .ok_or(VddError::SizeOverflow {
                    field: "전체 섹터 수",
                    value: current_sector,
                })?;
            files.push(data);
        }

        let output_len = current_sector
            .checked_mul(SECTOR_LEN)
            .ok_or(VddError::SizeOverflow {
                field: "전체 크기",
                value: current_sector,
            })?;
        let mut output = vec![0u8; output_len];
        output[..4].copy_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for (index, entry) in self.entries.iter().enumerate() {
            let base = 4 + index * ENTRY_LEN;
            output[base..base + NAME_LEN].copy_from_slice(&entry.raw_name);
            output[base + 16..base + 20].copy_from_slice(&sectors[index].to_le_bytes());
            let size = u32::try_from(files[index].len()).map_err(|_| VddError::SizeOverflow {
                field: "엔트리 크기",
                value: files[index].len(),
            })?;
            output[base + 20..base + 24].copy_from_slice(&size.to_le_bytes());
            let start = sectors[index] as usize * SECTOR_LEN;
            output[start..start + files[index].len()].copy_from_slice(&files[index]);
        }
        Ok(output)
    }
}

#[cfg(test)]
#[path = "vdd_tests.rs"]
mod tests;
