//! QUEST sparse glyph cache control의 RAM 적재 검증.
//!
//! full-disc 작성과 실행 관측을 분리한다. 정확한 control build를 다시 받은 뒤
//! package, OP21 site, carrier body와 page 0 상태만 frozen RAM에서 대조한다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::QUEST_PAGE_CARRIER_START_RUNTIME;

use super::quest_glyph_cache_disc::QuestGlyphCacheDiscBuild;
use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;

pub const QUEST_GLYPH_CACHE_RUNTIME_SCHEMA_VERSION: &str = "quest-glyph-cache-disc-runtime-v1";

/// 실행 중인 재배치 QUEST.EXE에서 control 설치 바이트를 대조한 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub ram_sha256: String,
    pub package_runtime: u32,
    pub package_bytes: usize,
    pub package_sha256: String,
    pub site_runtime: u32,
    pub site_bytes: usize,
    pub site_sha256: String,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub body_sha256: String,
    pub page_state_runtime: u32,
    pub page_state: u8,
    pub package_exact: bool,
    pub site_exact: bool,
    pub body_exact: bool,
    pub page_state_zero: bool,
}

/// sparse cache control의 runtime 설치 대조를 한 행 TSV로 만든다.
pub fn render_quest_glyph_cache_runtime_tsv(report: &QuestGlyphCacheRuntimeReport) -> String {
    format!(
        "schema_version\timage_sha256\tram_sha256\tpackage_runtime\tpackage_bytes\t\
         package_sha256\tsite_runtime\tsite_bytes\tsite_sha256\tbody_runtime\t\
         body_bytes\tbody_sha256\tpage_state_runtime\tpage_state\tpackage_exact\t\
         site_exact\tbody_exact\tpage_state_zero\n\
         {}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t\
         0x{:08X}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.image_sha256,
        report.ram_sha256,
        report.package_runtime,
        report.package_bytes,
        report.package_sha256,
        report.site_runtime,
        report.site_bytes,
        report.site_sha256,
        report.body_runtime,
        report.body_bytes,
        report.body_sha256,
        report.page_state_runtime,
        report.page_state,
        report.package_exact,
        report.site_exact,
        report.body_exact,
        report.page_state_zero,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheRuntimeError(String);

impl fmt::Display for QuestGlyphCacheRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheRuntimeError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheRuntimeError {
    QuestGlyphCacheRuntimeError(message.into())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], QuestGlyphCacheRuntimeError> {
    let offset = psx_main_ram_offset(runtime, len)
        .ok_or_else(|| error(format!("{label} runtime 범위가 PS1 main RAM 밖임")))?;
    let end = offset
        .checked_add(len)
        .ok_or_else(|| error(format!("{label} RAM 범위 overflow")))?;
    ram.get(offset..end)
        .ok_or_else(|| error(format!("{label} RAM dump가 잘림")))
}

/// exact control의 package/site/body 적재와 page 0 상태를 RAM dump에서 검증한다.
pub fn verify_quest_glyph_cache_control_ram(
    build: &QuestGlyphCacheDiscBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    ram: &[u8],
) -> Result<QuestGlyphCacheRuntimeReport, QuestGlyphCacheRuntimeError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "PS1 main RAM dump 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            ram.len()
        )));
    }
    if !is_sha256_hex(&build.report.image_sha256)
        || !is_sha256_hex(&package.sha256)
        || build.report.package_sha256 != package.sha256
        || build.report.package_bytes != package.bytes.len()
        || build.report.site_runtime != hook.site.base_addr()
        || build.report.body_runtime != hook.body_runtime
        || build.report.body_bytes != hook.body.bytes().len()
        || !hook.package_materialized
        || hook.runtime_verified
        || hook.build_eligible
    {
        return Err(error("QUEST sparse cache runtime 검증 입력 계약 drift"));
    }

    let loaded_package = runtime_slice(
        ram,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        package.bytes.len(),
        "QUEST sparse cache package",
    )?;
    let loaded_site = runtime_slice(
        ram,
        hook.site.base_addr(),
        hook.site.bytes().len(),
        "QUEST sparse cache site",
    )?;
    let loaded_body = runtime_slice(
        ram,
        hook.body_runtime,
        hook.body.bytes().len(),
        "QUEST sparse cache body",
    )?;
    let page_state = runtime_slice(
        ram,
        QUEST_PAGE_HOOK_STATE_RUNTIME,
        1,
        "QUEST sparse cache page state",
    )?[0];
    let package_exact = loaded_package == package.bytes.as_slice();
    let site_exact = loaded_site == hook.site.bytes();
    let body_exact = loaded_body == hook.body.bytes();
    let page_state_zero = page_state == 0;
    if !package_exact || !site_exact || !body_exact || !page_state_zero {
        return Err(error(format!(
            "QUEST sparse cache runtime readback 불일치: package/site/body/page0={package_exact}/{site_exact}/{body_exact}/{page_state_zero}"
        )));
    }

    Ok(QuestGlyphCacheRuntimeReport {
        schema_version: QUEST_GLYPH_CACHE_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.image_sha256.clone(),
        ram_sha256: format!("{:x}", Sha256::digest(ram)),
        package_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
        package_bytes: loaded_package.len(),
        package_sha256: format!("{:x}", Sha256::digest(loaded_package)),
        site_runtime: hook.site.base_addr(),
        site_bytes: loaded_site.len(),
        site_sha256: format!("{:x}", Sha256::digest(loaded_site)),
        body_runtime: hook.body_runtime,
        body_bytes: loaded_body.len(),
        body_sha256: format!("{:x}", Sha256::digest(loaded_body)),
        page_state_runtime: QUEST_PAGE_HOOK_STATE_RUNTIME,
        page_state,
        package_exact,
        site_exact,
        body_exact,
        page_state_zero,
    })
}

#[cfg(test)]
#[path = "quest_glyph_cache_runtime_tests.rs"]
mod tests;
