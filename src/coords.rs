//! 좌표계 변환.
//!
//! PS1 디스크에는 **서로 다른 세 좌표계**가 있고 섞으면 쓰기가 엉뚱한 곳에
//! 간다.
//!
//! | 좌표 | 단위 | 예 |
//! |------|------|-----|
//! | 런타임 주소 | CPU 주소 공간 | `0x800AA6E8` (QUEST 엔트리) |
//! | 논리 파일 오프셋 | 파일 선두 기준 바이트 | PSX-EXE 텍스트는 `0x800`부터 |
//! | 이미지 바이트 오프셋 | BIN 파일 선두 기준 | Mode2/2352 섹터 안 form1 위치 |
//!
//! 논리 → 이미지 변환은 섹터마다 304바이트(sync·header·subheader·EDC·ECC)를
//! 건너뛴다. 논리적으로 연속인 2049바이트가 이미지에서는 연속이 아니다.

use crate::source::{FORM1_DATA_LEN, FORM1_DATA_OFFSET, SECTOR_LEN};

/// RAM에 연속 적재된 자산 하나의 주소 사상.
///
/// EXE 적재 사상과 달리 디스크 파일 header를 포함하지 않는다. SEQ처럼
/// 파일 offset 0이 그대로 RAM `base`에 놓이는 자산의 runtime 주소와
/// 논리 파일 offset을 여기서만 서로 바꾼다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeAssetMap {
    pub base: u32,
    pub len: u32,
}

impl RuntimeAssetMap {
    /// 끝 주소가 `u32`를 넘지 않는 사상만 만든다.
    pub const fn new(base: u32, len: u32) -> Option<Self> {
        if base.checked_add(len).is_none() {
            return None;
        }
        Some(Self { base, len })
    }

    /// runtime 주소 → 자산 선두 기준 논리 파일 offset.
    pub fn file_offset(&self, runtime_addr: u32) -> Option<usize> {
        let offset = runtime_addr.checked_sub(self.base)?;
        if offset >= self.len {
            return None;
        }
        usize::try_from(offset).ok()
    }

    /// 자산 선두 기준 논리 파일 offset → runtime 주소.
    pub fn runtime_addr(&self, file_offset: usize) -> Option<u32> {
        let offset = u32::try_from(file_offset).ok()?;
        if offset >= self.len {
            return None;
        }
        self.base.checked_add(offset)
    }

    /// 주소가 이 자산의 적재 범위 안인가.
    pub fn contains(&self, runtime_addr: u32) -> bool {
        self.file_offset(runtime_addr).is_some()
    }
}

/// ISO 9660 논리 파일 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalFile {
    /// 파일시스템 LBA. raw 트랙 섹터 번호와 같은 값이지만 의미가 다르다.
    pub lba: u32,
    /// 논리 크기 (바이트).
    pub len: u32,
}

impl LogicalFile {
    pub const fn new(lba: u32, len: u32) -> Self {
        Self { lba, len }
    }

    /// 논리 파일 오프셋 → 이미지 바이트 오프셋.
    ///
    /// 파일 범위 밖이면 `None`. 범위를 넘겨 쓰는 계획이 조용히 통과하지
    /// 않도록 경계를 여기서 막는다.
    pub fn image_offset(&self, file_offset: u64) -> Option<u64> {
        if file_offset >= self.len as u64 {
            return None;
        }
        let sector = file_offset / FORM1_DATA_LEN as u64;
        let within = file_offset % FORM1_DATA_LEN as u64;
        Some((self.lba as u64 + sector) * SECTOR_LEN as u64 + FORM1_DATA_OFFSET as u64 + within)
    }

    /// 이 파일이 차지하는 섹터 수.
    pub const fn sector_count(&self) -> u32 {
        self.len.div_ceil(FORM1_DATA_LEN as u32)
    }

    /// `file_offset`부터 `len`바이트가 걸치는 섹터 LBA 범위.
    ///
    /// 수정된 섹터만 EDC/ECC를 다시 계산해야 하므로 필요하다.
    pub fn touched_sectors(&self, file_offset: u64, len: u64) -> Option<(u32, u32)> {
        if len == 0 || file_offset.checked_add(len)? > self.len as u64 {
            return None;
        }
        let first = file_offset / FORM1_DATA_LEN as u64;
        let last = (file_offset + len - 1) / FORM1_DATA_LEN as u64;
        Some((self.lba + first as u32, self.lba + last as u32))
    }
}

/// PSX-EXE 적재 사상.
///
/// 값의 근거는 `docs/rom_analysis.md` §2.3이며, 지원 리비전 하나에 대한
/// **사양**이다. 런타임 휴리스틱으로 다시 찾지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExeMap {
    pub name: &'static str,
    /// PSX-EXE 헤더의 적재 주소.
    pub load_addr: u32,
    /// 텍스트 세그먼트 길이.
    pub text_len: u32,
    /// 엔트리 PC.
    pub entry_pc: u32,
    /// 디스크상의 논리 파일.
    pub file: LogicalFile,
}

/// PSX-EXE 헤더 크기. 텍스트는 파일 오프셋 `0x800`부터다.
pub const PSX_EXE_HEADER_LEN: u64 = 0x800;
/// PlayStation main RAM의 물리 크기.
pub const PSX_MAIN_RAM_LEN: usize = 0x20_0000;

/// KSEG0/KSEG1 runtime 주소를 2 MiB PS1 main RAM dump offset으로 바꾼다.
///
/// 범위 끝이 RAM을 넘거나 KSEG mirror 밖이면 `None`이다. runtime 주소를
/// dump index로 직접 마스킹하지 않도록 이 변환을 한 곳에서 소유한다.
pub fn psx_main_ram_offset(runtime_addr: u32, len: usize) -> Option<usize> {
    let normalized = normalize_kseg(runtime_addr);
    let offset = normalized.checked_sub(0x8000_0000)? as usize;
    let end = offset.checked_add(len)?;
    (end <= PSX_MAIN_RAM_LEN).then_some(offset)
}

impl ExeMap {
    /// 확장 후 적재 text 길이를 기준으로 runtime 주소를 text offset으로 바꾼다.
    ///
    /// 원본 사양인 [`Self::text_len`]을 바꾸지 않고 relocation build의
    /// post-BSS payload와 code 좌표를 같은 경계에서 계산할 때 쓴다.
    pub fn loaded_text_offset(&self, runtime_addr: u32, loaded_text_len: u32) -> Option<usize> {
        let delta = runtime_addr.checked_sub(self.load_addr)?;
        if delta >= loaded_text_len {
            return None;
        }
        usize::try_from(delta).ok()
    }

    /// 확장 후 적재 text 안의 runtime 주소를 논리 EXE 파일 offset으로 바꾼다.
    pub fn loaded_file_offset(&self, runtime_addr: u32, loaded_text_len: u32) -> Option<u64> {
        Some(PSX_EXE_HEADER_LEN + self.loaded_text_offset(runtime_addr, loaded_text_len)? as u64)
    }

    /// 런타임 주소 → 적재 text 선두 기준 오프셋.
    ///
    /// `PsxExe::text()`와 같은 헤더 제외 slice를 읽을 때 쓴다. 호출자가
    /// `load_addr`를 직접 빼서 11개 EXE의 좌표계를 섞지 않게 한다.
    pub fn text_offset(&self, runtime_addr: u32) -> Option<usize> {
        self.loaded_text_offset(runtime_addr, self.text_len)
    }

    /// 적재 text 선두 기준 오프셋 → 런타임 주소.
    ///
    /// `PsxExe::text()` 같은 header 제외 slice의 위치를 다시 런타임 주소로
    /// 바꾼다. 논리 파일 오프셋을 받는 [`Self::runtime_addr`]와 좌표계를
    /// 구분한다.
    pub fn text_runtime_addr(&self, text_offset: usize) -> Option<u32> {
        let offset = u32::try_from(text_offset).ok()?;
        if offset >= self.text_len {
            return None;
        }
        self.load_addr.checked_add(offset)
    }

    /// 런타임 주소 → 이 EXE의 논리 파일 오프셋.
    ///
    /// 적재 범위 밖이면 `None`. **다른 EXE의 환산식을 쓰면 안 된다** —
    /// 서브게임 10개가 같은 `0x80010000`에 적재되지만 각각 별도 빌드다.
    pub fn file_offset(&self, runtime_addr: u32) -> Option<u64> {
        self.loaded_file_offset(runtime_addr, self.text_len)
    }

    /// 런타임 주소 → 이미지 바이트 오프셋.
    pub fn image_offset(&self, runtime_addr: u32) -> Option<u64> {
        self.file.image_offset(self.file_offset(runtime_addr)?)
    }

    /// 논리 파일 오프셋 → 런타임 주소.
    pub fn runtime_addr(&self, file_offset: u64) -> Option<u32> {
        let delta = file_offset.checked_sub(PSX_EXE_HEADER_LEN)?;
        if delta >= self.text_len as u64 {
            return None;
        }
        Some(self.load_addr + delta as u32)
    }

    /// 적재된 런타임 주소 범위.
    pub const fn runtime_range(&self) -> (u32, u32) {
        (self.load_addr, self.load_addr + self.text_len)
    }

    /// 주소가 이 EXE의 적재 범위 안인가.
    pub const fn contains(&self, runtime_addr: u32) -> bool {
        runtime_addr >= self.load_addr && runtime_addr < self.load_addr + self.text_len
    }
}

/// 셸과 서브게임 10개의 적재 사상.
///
/// 근거: `docs/rom_analysis.md` §2.2, §2.3. 지원 리비전
/// (SHA-1 `ca01a98c…`)의 사양이다.
pub const EXE_MAPS: [ExeMap; 11] = [
    ExeMap {
        name: "SLPS_031.14",
        load_addr: 0x8012_0000,
        text_len: 0x0004_D800,
        entry_pc: 0x8015_BE04,
        file: LogicalFile::new(43621, 319_488),
    },
    ExeMap {
        name: "GOCHA1.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0009_0000,
        entry_pc: 0x8008_E870,
        file: LogicalFile::new(24, 591_872),
    },
    ExeMap {
        name: "GOCHA2.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0007_5000,
        entry_pc: 0x8007_5410,
        file: LogicalFile::new(4409, 481_280),
    },
    ExeMap {
        name: "HORI2.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0007_5800,
        entry_pc: 0x8007_5258,
        file: LogicalFile::new(8129, 483_328),
    },
    ExeMap {
        name: "HORI4.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0007_6000,
        entry_pc: 0x8007_550C,
        file: LogicalFile::new(11846, 485_376),
    },
    ExeMap {
        name: "KOUSIKI.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0006_E800,
        entry_pc: 0x8006_B140,
        file: LogicalFile::new(15548, 454_656),
    },
    ExeMap {
        name: "MINPUYO.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0007_5800,
        entry_pc: 0x8007_5A40,
        file: LogicalFile::new(19157, 483_328),
    },
    ExeMap {
        name: "PCOMI.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0004_9800,
        entry_pc: 0x8004_7978,
        file: LogicalFile::new(22875, 303_104),
    },
    ExeMap {
        name: "PUYO1.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0008_6000,
        entry_pc: 0x8001_5274,
        file: LogicalFile::new(23315, 550_912),
    },
    ExeMap {
        name: "PUYO2.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0008_8000,
        entry_pc: 0x8001_4A3C,
        file: LogicalFile::new(28257, 559_104),
    },
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x000A_E800,
        entry_pc: 0x800A_A6E8,
        file: LogicalFile::new(33538, 716_800),
    },
];

/// 이름으로 EXE 사상을 찾는다.
pub fn exe_map(name: &str) -> Option<&'static ExeMap> {
    EXE_MAPS.iter().find(|m| m.name == name)
}

/// KSEG 미러를 KUSEG 기준 주소로 정규화한다.
///
/// `0x80000000`(KSEG0, 캐시됨)과 `0xA0000000`(KSEG1, 캐시 우회)은 같은
/// 물리 메모리를 가리킨다. 훅이 stale I-cache를 피하려고 KSEG1로 실행하는
/// 경우가 있으므로, 주소 비교 전에 정규화한다.
pub const fn normalize_kseg(addr: u32) -> u32 {
    if addr >= 0x8000_0000 && addr < 0xC000_0000 {
        (addr & 0x1FFF_FFFF) | 0x8000_0000
    } else {
        addr
    }
}

#[cfg(test)]
#[path = "coords_tests.rs"]
mod coords_tests;
