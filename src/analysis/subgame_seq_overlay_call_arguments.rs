//! 비-QUEST SEQ overlay의 간접 호출 제5인자 설정 경계.
//!
//! overlay initialized 영역의 `jalr $ra, reg`를 선형 해석하고, 활성 표준
//! stack frame에서 `sp+16`을 마지막으로 쓴 명령을 연결한다. 이 분석은
//! 현재 호출을 위해 새로 쓴 값과 앞선 호출이 이미 소비한 값을 구분한다.
//! 후자를 현재 호출의 제5인자로 승인하지 않으며, 실제 callee와 인자 의미는
//! 별도 정적·runtime 증거가 필요하다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::disasm::outgoing_stack_arguments::find_outgoing_stack_argument_flows;
pub use crate::disasm::outgoing_stack_arguments::OutgoingStackArgumentOrigin as SubgameSeqOverlayArgumentOrigin;
use crate::isa::{decode_slice, Reg};
use crate::script::{
    parse_subgame_seq_overlay_footer, BOX_SUBGAME_SEQ_LAYOUT_FILES, BOX_SUBGAME_SEQ_OVERLAYS,
};

pub const SUBGAME_SEQ_OVERLAY_CALL_ARGUMENT_SCHEMA_VERSION: &str =
    "subgame-seq-overlay-call-argument-v1";
pub const BOX_SUBGAME_SEQ_OVERLAY_CALL_ARGUMENT_ROWS: usize = 0;
pub const BOX_SUBGAME_SEQ_OVERLAY_CURRENT_CALL_ARGUMENT_ROWS: usize = 0;
pub const BOX_SUBGAME_SEQ_OVERLAY_PREVIOUSLY_CONSUMED_ROWS: usize = 0;
pub const BOX_SUBGAME_SEQ_OVERLAY_CALL_ARGUMENT_REPORT_SHA256: [u8; 32] = [
    0x25, 0xB0, 0x53, 0xD0, 0x25, 0xE0, 0xDE, 0x6F, 0x44, 0xB2, 0xED, 0xA5, 0x0B, 0x25, 0x90, 0x5B,
    0x39, 0xF5, 0xC5, 0x20, 0xA3, 0x76, 0xB1, 0x14, 0x9E, 0x97, 0x9F, 0x33, 0x12, 0x03, 0x9B, 0xB0,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqOverlayCallArgumentRow {
    pub archive: String,
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub load_base: u32,
    pub call_runtime: u32,
    pub target_reg: Reg,
    pub argument_store_runtime: u32,
    pub argument_reg: Reg,
    pub prior_consuming_calls: Vec<u32>,
    pub origin: SubgameSeqOverlayArgumentOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqOverlayCallArgumentAnalysis {
    pub sequence_files: usize,
    pub overlay_files: usize,
    pub rows: Vec<SubgameSeqOverlayCallArgumentRow>,
}

impl SubgameSeqOverlayCallArgumentAnalysis {
    pub fn current_call_rows(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| row.origin != SubgameSeqOverlayArgumentOrigin::PreviouslyConsumed)
            .count()
    }

    pub fn previously_consumed_rows(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| row.origin == SubgameSeqOverlayArgumentOrigin::PreviouslyConsumed)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqOverlayCallArgumentError(String);

impl fmt::Display for SubgameSeqOverlayCallArgumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqOverlayCallArgumentError {}

fn error(message: impl Into<String>) -> SubgameSeqOverlayCallArgumentError {
    SubgameSeqOverlayCallArgumentError(message.into())
}

pub fn analyze_subgame_seq_overlay_call_arguments<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
) -> Result<SubgameSeqOverlayCallArgumentAnalysis, SubgameSeqOverlayCallArgumentError> {
    let mut input = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if input
            .insert((archive.to_string(), sequence.to_string()), raw)
            .is_some()
        {
            return Err(error(format!(
                "{archive}:{sequence} overlay call argument 입력 중복"
            )));
        }
    }
    let mut overlay_files = 0usize;
    let mut rows = Vec::new();
    for ((archive, sequence), raw) in &input {
        let Some(footer) = parse_subgame_seq_overlay_footer(raw)
            .map_err(|cause| error(format!("{archive}:{sequence} overlay footer: {cause}")))?
        else {
            continue;
        };
        overlay_files += 1;
        let initialized = raw
            .get(..footer.footer_offset)
            .ok_or_else(|| error(format!("{archive}:{sequence} initialized 범위 이탈")))?;
        let instructions = decode_slice(footer.load_base, initialized)
            .ok_or_else(|| error(format!("{archive}:{sequence} initialized 비정렬")))?;
        let sequence_sha256 = Sha256::digest(raw).into();
        rows.extend(
            find_outgoing_stack_argument_flows(&instructions, 16)
                .into_iter()
                .map(|flow| SubgameSeqOverlayCallArgumentRow {
                    archive: archive.clone(),
                    sequence: sequence.clone(),
                    sequence_sha256,
                    load_base: footer.load_base,
                    call_runtime: flow.call_runtime,
                    target_reg: flow.target_reg,
                    argument_store_runtime: flow.argument_store_runtime,
                    argument_reg: flow.argument_reg,
                    prior_consuming_calls: flow.prior_consuming_calls,
                    origin: flow.origin,
                }),
        );
    }
    Ok(SubgameSeqOverlayCallArgumentAnalysis {
        sequence_files: input.len(),
        overlay_files,
        rows,
    })
}

pub fn verify_box_subgame_seq_overlay_call_arguments<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
) -> Result<SubgameSeqOverlayCallArgumentAnalysis, SubgameSeqOverlayCallArgumentError> {
    let analysis = analyze_subgame_seq_overlay_call_arguments(sources)?;
    if analysis.sequence_files != BOX_SUBGAME_SEQ_LAYOUT_FILES
        || analysis.overlay_files != BOX_SUBGAME_SEQ_OVERLAYS
        || analysis.rows.len() != BOX_SUBGAME_SEQ_OVERLAY_CALL_ARGUMENT_ROWS
        || analysis.current_call_rows() != BOX_SUBGAME_SEQ_OVERLAY_CURRENT_CALL_ARGUMENT_ROWS
        || analysis.previously_consumed_rows() != BOX_SUBGAME_SEQ_OVERLAY_PREVIOUSLY_CONSUMED_ROWS
    {
        return Err(error(format!(
            "overlay call argument 기준선 불일치: files={}/{}, overlays={}/{}, rows={}/{}, current={}/{}, previously-consumed={}/{}",
            analysis.sequence_files,
            BOX_SUBGAME_SEQ_LAYOUT_FILES,
            analysis.overlay_files,
            BOX_SUBGAME_SEQ_OVERLAYS,
            analysis.rows.len(),
            BOX_SUBGAME_SEQ_OVERLAY_CALL_ARGUMENT_ROWS,
            analysis.current_call_rows(),
            BOX_SUBGAME_SEQ_OVERLAY_CURRENT_CALL_ARGUMENT_ROWS,
            analysis.previously_consumed_rows(),
            BOX_SUBGAME_SEQ_OVERLAY_PREVIOUSLY_CONSUMED_ROWS,
        )));
    }
    let report = render_subgame_seq_overlay_call_arguments_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_OVERLAY_CALL_ARGUMENT_REPORT_SHA256 {
        return Err(error(format!(
            "overlay call argument report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_OVERLAY_CALL_ARGUMENT_REPORT_SHA256),
            hex(&digest),
        )));
    }
    Ok(analysis)
}

pub fn render_subgame_seq_overlay_call_arguments_tsv(
    analysis: &SubgameSeqOverlayCallArgumentAnalysis,
) -> String {
    let mut output = String::from(
        "record_kind\tschema_version\tsequence_files\toverlay_files\tcurrent_call_rows\t\
         previously_consumed_rows\tarchive\tsequence\tsequence_sha256\tload_base\tcall_runtime\t\
         target_register\targument_store_runtime\targument_register\t\
         prior_consuming_calls\tprior_consuming_call_runtime\torigin\tclaim_boundary\n",
    );
    writeln!(
        output,
        "summary\t{schema}\t{files}\t{overlays}\t{current}\t{previous}\t\t\t\t\t\t\t\t\t\t\t\tstack-slot-flow-census-only-callee-and-argument-meaning-unverified",
        schema = SUBGAME_SEQ_OVERLAY_CALL_ARGUMENT_SCHEMA_VERSION,
        files = analysis.sequence_files,
        overlays = analysis.overlay_files,
        current = analysis.current_call_rows(),
        previous = analysis.previously_consumed_rows(),
    )
    .expect("String 쓰기는 실패하지 않는다");
    for row in &analysis.rows {
        writeln!(
            output,
            "flow\t{schema}\t{files}\t{overlays}\t{current}\t{previous}\t\
             {archive}\t{sequence}\t{sha}\t0x{load_base:08X}\t\
             0x{call:08X}\t{target}\t0x{store:08X}\t{argument}\t{prior_count}\t\
             {prior}\t{origin}\tstack-slot-flow-only-callee-and-argument-meaning-unverified",
            schema = SUBGAME_SEQ_OVERLAY_CALL_ARGUMENT_SCHEMA_VERSION,
            files = analysis.sequence_files,
            overlays = analysis.overlay_files,
            current = analysis.current_call_rows(),
            previous = analysis.previously_consumed_rows(),
            archive = row.archive,
            sequence = row.sequence,
            sha = hex(&row.sequence_sha256),
            load_base = row.load_base,
            call = row.call_runtime,
            target = row.target_reg.name(),
            store = row.argument_store_runtime,
            argument = row.argument_reg.name(),
            prior_count = row.prior_consuming_calls.len(),
            prior = row
                .prior_consuming_calls
                .iter()
                .map(|runtime| format!("0x{runtime:08X}"))
                .collect::<Vec<_>>()
                .join(";"),
            origin = origin_name(row.origin),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn origin_name(origin: SubgameSeqOverlayArgumentOrigin) -> &'static str {
    match origin {
        SubgameSeqOverlayArgumentOrigin::CurrentCallSetup => "current-call-setup",
        SubgameSeqOverlayArgumentOrigin::CurrentCallDelaySlot => "current-call-delay-slot",
        SubgameSeqOverlayArgumentOrigin::PreviouslyConsumed => "previously-consumed",
    }
}

#[cfg(test)]
#[path = "subgame_seq_overlay_call_arguments_tests.rs"]
mod subgame_seq_overlay_call_arguments_tests;
