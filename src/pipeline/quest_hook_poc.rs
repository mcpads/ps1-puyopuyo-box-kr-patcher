//! QUEST OP21 hook 설치 경로의 무동작 control PoC.
//!
//! 선택한 내부 0-run에 원본 첫 명령을 옮기고 원본 tail로 돌아오는 최소
//! 우회를 설치한다. 화면 동작은 바꾸지 않으며, 디스크 적재 전부터 존재하는
//! KSEG0 code가 실제로 적재·실행·복귀하는지만 검증하기 위한 제품 빌드다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::{ExeMap, LogicalFile};
use crate::disasm::{
    verify_box_quest_font_hook_site, verify_box_quest_hook_space_candidates,
    QuestHookCandidateAnalysis, QuestHookCandidateStaticVerdict,
    QUEST_FONT_HOOK_CONTINUATION_RUNTIME, QUEST_FONT_HOOK_SITE_RUNTIME,
};
use crate::disc::{apply_form1_write_plan, DiscImage, Iso9660, PsxExe, VerifiedSourceImage};
use crate::isa::{
    verify_bytes, verify_typed_source, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode,
    VerifyError,
};
use crate::patch::{plan_logical_write, ImageRegion, WriteIntent, WritePlan};

use super::vdd_plan::logical_file;

const QUEST_EXE_PATH: &str = "/QUEST/QUEST.EXE;1";

pub const QUEST_HOOK_BODY_RUNTIME: u32 = 0x800B_2328;
pub const QUEST_HOOK_BODY_RESERVED_END_RUNTIME: u32 = 0x800B_24E8;
pub const QUEST_HOOK_BODY_BYTES: usize = 12;
pub const QUEST_HOOK_RESERVED_BYTES: usize = 448;

const SITE_SOURCE_ID: &str = "pipeline::quest_hook_poc::op21_site_jump";
const BODY_SOURCE_ID: &str = "pipeline::quest_hook_poc::noop_body";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestHookNoopSources {
    pub site: VerifiedMachineCode,
    pub body: VerifiedMachineCode,
    pub reserved_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestHookNoopReport {
    pub site_runtime: u32,
    pub body_runtime: u32,
    pub continuation_runtime: u32,
    pub site_words: usize,
    pub body_words: usize,
    pub reserved_bytes: usize,
    pub machine_code_writes: usize,
    pub touched_sectors: usize,
    pub final_expected_writes: usize,
    pub image_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestHookNoopBuild {
    pub image: Vec<u8>,
    pub report: QuestHookNoopReport,
}

#[derive(Debug)]
pub enum QuestHookNoopError {
    Message(String),
    MachineCode(VerifyError),
}

impl fmt::Display for QuestHookNoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::MachineCode(source) => write!(f, "QUEST 무동작 hook ISA 검산 실패: {source}"),
        }
    }
}

impl Error for QuestHookNoopError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MachineCode(source) => Some(source),
            Self::Message(_) => None,
        }
    }
}

impl From<VerifyError> for QuestHookNoopError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

fn error(message: impl Into<String>) -> QuestHookNoopError {
    QuestHookNoopError::Message(message.into())
}

fn same_jump_segment(site: u32, target: u32) -> bool {
    site.wrapping_add(4) & 0xF000_0000 == target & 0xF000_0000
}

fn assemble_sources() -> Result<(VerifiedMachineCode, VerifiedMachineCode), QuestHookNoopError> {
    if !same_jump_segment(QUEST_FONT_HOOK_SITE_RUNTIME, QUEST_HOOK_BODY_RUNTIME)
        || !same_jump_segment(
            QUEST_HOOK_BODY_RUNTIME + 4,
            QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
        )
    {
        return Err(error(
            "QUEST 무동작 hook의 j target이 같은 jump segment 밖임",
        ));
    }

    let site = verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_FONT_HOOK_SITE_RUNTIME,
        SITE_SOURCE_ID,
        &[
            K::J {
                target: QUEST_HOOK_BODY_RUNTIME,
            },
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ],
    )?;
    let body = verify_typed_source(
        IsaProfile::DECLARED,
        QUEST_HOOK_BODY_RUNTIME,
        BODY_SOURCE_ID,
        &[
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
            K::J {
                target: QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ],
    )?;
    Ok((site, body))
}

/// 정적 후보와 원본 ABI를 다시 확인하고 무동작 hook의 typed source를 만든다.
pub fn verify_quest_hook_noop_sources(
    map: &ExeMap,
    text: &[u8],
    candidates: &QuestHookCandidateAnalysis,
) -> Result<QuestHookNoopSources, QuestHookNoopError> {
    verify_box_quest_font_hook_site(map, text).map_err(|source| error(source.to_string()))?;
    let candidate = candidates
        .candidates
        .iter()
        .find(|candidate| {
            candidate.start_runtime == QUEST_HOOK_BODY_RUNTIME
                && candidate.end_runtime == QUEST_HOOK_BODY_RESERVED_END_RUNTIME
        })
        .ok_or_else(|| error("QUEST 무동작 hook의 선택 0-run이 후보 분모에 없음"))?;
    if candidate.bytes != QUEST_HOOK_RESERVED_BYTES
        || candidate.static_verdict != QuestHookCandidateStaticVerdict::UnreferencedNeedsRuntime
    {
        return Err(error(format!(
            "QUEST 무동작 hook 후보 계약 불일치: bytes={}, verdict={}",
            candidate.bytes,
            candidate.static_verdict.as_str()
        )));
    }
    if QUEST_HOOK_BODY_BYTES > candidate.bytes {
        return Err(error("QUEST 무동작 hook body가 선택 후보보다 큼"));
    }
    let body_offset = map
        .text_offset(QUEST_HOOK_BODY_RUNTIME)
        .ok_or_else(|| error("QUEST 무동작 hook body가 EXE text 밖임"))?;
    let body_end = body_offset
        .checked_add(candidate.bytes)
        .ok_or_else(|| error("QUEST 무동작 hook 후보 범위 overflow"))?;
    if !text
        .get(body_offset..body_end)
        .is_some_and(|candidate| candidate.iter().all(|byte| *byte == 0))
    {
        return Err(error("QUEST 무동작 hook 후보 원본이 전부 0이 아님"));
    }

    let (site, body) = assemble_sources()?;
    Ok(QuestHookNoopSources {
        site,
        body,
        reserved_bytes: candidate.bytes,
    })
}

fn add_machine_code_write(
    plan: &mut WritePlan,
    file: LogicalFile,
    file_offset: u64,
    original: &[u8],
    verified: &VerifiedMachineCode,
    id: &str,
    purpose: &str,
) -> Result<(), QuestHookNoopError> {
    let writes = plan_logical_write(
        file,
        file_offset,
        original,
        verified.bytes(),
        id,
        "pipeline::quest_hook_poc",
        purpose,
        |_| WriteIntent::MachineCode {
            verified: Box::new(verified.clone()),
        },
    )
    .ok_or_else(|| error(format!("{id}: 논리 기계어 쓰기 계획 생성 실패")))?;
    if writes.len() != 1 {
        return Err(error(format!(
            "{id}: 기계어 쓰기가 Mode2 섹터 경계를 넘어 {}개로 분할됨",
            writes.len()
        )));
    }
    let write = writes.into_iter().next().expect("길이 1을 확인함");
    plan.regions.push(ImageRegion::new(
        format!("region:{}", write.id),
        write.range(),
        write.intent.region_kind(),
        "지원 QUEST.EXE의 typed MIPS hook control 구간",
    ));
    plan.writes.push(write);
    Ok(())
}

fn source_slice<'a>(
    raw: &'a [u8],
    map: &ExeMap,
    runtime: u32,
    len: usize,
) -> Result<(u64, &'a [u8]), QuestHookNoopError> {
    let offset = map
        .file_offset(runtime)
        .ok_or_else(|| error(format!("QUEST runtime 0x{runtime:08X}가 논리 파일 밖임")))?;
    let start = usize::try_from(offset).map_err(|_| error("QUEST 논리 offset 변환 실패"))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error("QUEST 논리 범위 overflow"))?;
    let slice = raw
        .get(start..end)
        .ok_or_else(|| error("QUEST 논리 범위가 EXE 밖임"))?;
    Ok((offset, slice))
}

/// 원본 디스크에서 무동작 hook control 이미지를 처음부터 빌드한다.
pub fn build_quest_hook_noop(
    source: &VerifiedSourceImage,
) -> Result<QuestHookNoopBuild, QuestHookNoopError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let entry = iso
        .find(QUEST_EXE_PATH)
        .map_err(|source| error(source.to_string()))?;
    let file = logical_file(entry).map_err(error)?;
    let raw = iso
        .read_file(QUEST_EXE_PATH)
        .map_err(|source| error(source.to_string()))?;
    let exe = PsxExe::parse(&raw).map_err(|source| error(source.to_string()))?;
    let map =
        crate::coords::exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    exe.verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    let candidates = verify_box_quest_hook_space_candidates(map, exe.text())
        .map_err(|source| error(source.to_string()))?;
    let sources = verify_quest_hook_noop_sources(map, exe.text(), &candidates)?;

    let (site_offset, site_original) = source_slice(
        &raw,
        map,
        QUEST_FONT_HOOK_SITE_RUNTIME,
        sources.site.bytes().len(),
    )?;
    let (body_offset, body_original) = source_slice(
        &raw,
        map,
        QUEST_HOOK_BODY_RUNTIME,
        sources.body.bytes().len(),
    )?;
    let mut plan = WritePlan::new();
    add_machine_code_write(
        &mut plan,
        file,
        site_offset,
        site_original,
        &sources.site,
        "quest-hook-noop-site",
        "OP21 첫 명령을 선택한 KSEG0 hook body로 우회",
    )?;
    add_machine_code_write(
        &mut plan,
        file,
        body_offset,
        body_original,
        &sources.body,
        "quest-hook-noop-body",
        "밀어낸 lw를 실행하고 원본 OP21 tail로 복귀",
    )?;

    let machine_code_writes = plan.writes.len();
    let touched_sectors = plan.touched_sectors().len();
    let final_expected_writes = machine_code_writes + touched_sectors;
    let output = apply_form1_write_plan(source.image(), &plan)
        .map_err(|source| error(source.to_string()))?;
    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    let final_raw = final_iso
        .read_file(QUEST_EXE_PATH)
        .map_err(|source| error(source.to_string()))?;
    let final_exe = PsxExe::parse(&final_raw).map_err(|source| error(source.to_string()))?;
    final_exe
        .verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    for (runtime, verified) in [
        (QUEST_FONT_HOOK_SITE_RUNTIME, &sources.site),
        (QUEST_HOOK_BODY_RUNTIME, &sources.body),
    ] {
        let (_, actual) = source_slice(&final_raw, map, runtime, verified.bytes().len())?;
        if actual != verified.bytes() {
            return Err(error(format!(
                "QUEST 무동작 hook 최종 readback 불일치: 0x{runtime:08X}"
            )));
        }
        verify_bytes(IsaProfile::DECLARED, runtime, actual)?;
    }

    let image = final_image.into_bytes();
    let image_sha256 = format!("{:x}", Sha256::digest(&image));
    Ok(QuestHookNoopBuild {
        image,
        report: QuestHookNoopReport {
            site_runtime: QUEST_FONT_HOOK_SITE_RUNTIME,
            body_runtime: QUEST_HOOK_BODY_RUNTIME,
            continuation_runtime: QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
            site_words: sources.site.len_words(),
            body_words: sources.body.len_words(),
            reserved_bytes: sources.reserved_bytes,
            machine_code_writes,
            touched_sectors,
            final_expected_writes,
            image_sha256,
        },
    })
}

#[cfg(test)]
#[path = "quest_hook_poc_tests.rs"]
mod tests;
