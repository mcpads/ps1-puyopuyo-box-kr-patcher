//! QUEST glyph cache body의 셀 decoder 기계어 조립.
//!
//! 각 decoder는 명령 열과 내부 control-flow patch를 함께 소유한다. 상위 body
//! 조립기는 decoder 배치 순서와 row decoder entry만 소비한다.

use crate::isa::{InstructionKind as K, Reg};

use super::super::super::quest_glyph_cache_plan::{
    QUEST_GLYPH_CACHE_ROWS_PER_CELL, QUEST_GLYPH_CACHE_ROW_INDEX_BITS,
    QUEST_GLYPH_CACHE_ROW_INDEX_LIMIT, QUEST_GLYPH_CACHE_SCRATCH_BYTES,
};
use super::super::QuestGlyphCacheHookPlanError;
use super::{patch_branch, patch_jump};

pub(super) fn append_row_dictionary_decoder(
    kinds: &mut Vec<K>,
    row_dictionary_limit_exclusive: i16,
    body_runtime: u32,
) -> Result<usize, QuestGlyphCacheHookPlanError> {
    // A0/A3는 bitmap record의 시작/끝, A1은 dictionary, A2는 96-byte scratch다.
    let row_decoder = kinds.len();
    kinds.extend([
        K::Addu {
            rd: Reg::T0,
            rs: Reg::A2,
            rt: Reg::ZERO,
        },
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::A2,
            imm: QUEST_GLYPH_CACHE_SCRATCH_BYTES as i16,
        },
        K::Addu {
            rd: Reg::T2,
            rs: Reg::A3,
            rt: Reg::ZERO,
        },
        K::Addiu {
            rt: Reg::T6,
            rs: Reg::A0,
            imm: 2,
        },
        K::Sltu {
            rd: Reg::T7,
            rs: Reg::T2,
            rt: Reg::T6,
        },
        K::Bne {
            rs: Reg::T7,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0,
        },
        K::Lbu {
            rt: Reg::T6,
            base: Reg::A0,
            offset: 1,
        },
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 2,
        },
        K::Sll {
            rd: Reg::T6,
            rt: Reg::T6,
            sa: 8,
        },
        K::Or {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::T6,
        },
        K::Addu {
            rd: Reg::T3,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Addu {
            rd: Reg::T4,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Addiu {
            rt: Reg::T5,
            rs: Reg::ZERO,
            imm: QUEST_GLYPH_CACHE_ROWS_PER_CELL as i16,
        },
    ]);
    let header_error = row_decoder + 5;
    let row_loop = kinds.len();
    kinds.extend([
        K::Andi {
            rt: Reg::T6,
            rs: Reg::V1,
            imm: 1,
        },
        K::Beq {
            rs: Reg::T6,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Srl {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 1,
        },
    ]);
    let blank_branch = row_loop + 1;
    let fill_loop = kinds.len();
    kinds.extend([
        K::Sltiu {
            rt: Reg::T6,
            rs: Reg::T4,
            imm: QUEST_GLYPH_CACHE_ROW_INDEX_BITS as i16,
        },
        K::Beq {
            rs: Reg::T6,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::A0,
            rt: Reg::T2,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T6,
            base: Reg::A0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 1,
        },
        K::Sllv {
            rd: Reg::T6,
            rt: Reg::T6,
            rs: Reg::T4,
        },
        K::Or {
            rd: Reg::T3,
            rs: Reg::T3,
            rt: Reg::T6,
        },
        K::Addiu {
            rt: Reg::T4,
            rs: Reg::T4,
            imm: 8,
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let row_ready = kinds.len();
    kinds.extend([
        K::Andi {
            rt: Reg::T6,
            rs: Reg::T3,
            imm: (QUEST_GLYPH_CACHE_ROW_INDEX_LIMIT - 1) as u16,
        },
        K::Sltiu {
            rt: Reg::T7,
            rs: Reg::T6,
            imm: row_dictionary_limit_exclusive,
        },
        K::Beq {
            rs: Reg::T7,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Srl {
            rd: Reg::T3,
            rt: Reg::T3,
            sa: QUEST_GLYPH_CACHE_ROW_INDEX_BITS as u8,
        },
        K::Beq {
            rs: Reg::T6,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::T4,
            rs: Reg::T4,
            imm: -(QUEST_GLYPH_CACHE_ROW_INDEX_BITS as i16),
        },
        K::Sll {
            rd: Reg::T7,
            rt: Reg::T6,
            sa: 2,
        },
        K::Sll {
            rd: Reg::T8,
            rt: Reg::T6,
            sa: 1,
        },
        K::Addu {
            rd: Reg::T6,
            rs: Reg::A1,
            rt: Reg::T7,
        },
        K::Addu {
            rd: Reg::T6,
            rs: Reg::T6,
            rt: Reg::T8,
        },
        K::Lhu {
            rt: Reg::T7,
            base: Reg::T6,
            offset: 0,
        },
        K::Lhu {
            rt: Reg::T8,
            base: Reg::T6,
            offset: 2,
        },
        K::Lhu {
            rt: Reg::T9,
            base: Reg::T6,
            offset: 4,
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let blank = kinds.len();
    kinds.extend([
        K::Addu {
            rd: Reg::T7,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Addu {
            rd: Reg::T8,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Addu {
            rd: Reg::T9,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ]);
    let write_row = kinds.len();
    kinds.extend([
        K::Sh {
            rt: Reg::T7,
            base: Reg::T0,
            offset: 0,
        },
        K::Sh {
            rt: Reg::T8,
            base: Reg::T0,
            offset: 2,
        },
        K::Sh {
            rt: Reg::T9,
            base: Reg::T0,
            offset: 4,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 6,
        },
        K::Addiu {
            rt: Reg::T5,
            rs: Reg::T5,
            imm: -1,
        },
        K::Bgtz {
            rs: Reg::T5,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let finish = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::A0,
            rt: Reg::T2,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Bne {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Bne {
            rs: Reg::T3,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sltiu {
            rt: Reg::T6,
            rs: Reg::T4,
            imm: 8,
        },
        K::Beq {
            rs: Reg::T6,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let failure = kinds.len();
    kinds.extend([
        K::Addu {
            rd: Reg::V0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    for (source, target) in [
        (header_error, failure),
        (blank_branch, blank),
        (fill_loop + 1, row_ready),
        (fill_loop + 3, failure),
        (row_ready + 2, failure),
        (row_ready + 4, failure),
        (write_row + 5, row_loop),
        (finish, failure),
        (finish + 2, failure),
        (finish + 4, failure),
        (finish + 6, failure),
    ] {
        patch_branch(kinds, source, target)?;
    }
    patch_jump(kinds, fill_loop + 10, fill_loop, body_runtime)?;
    patch_jump(kinds, row_ready + 13, write_row, body_runtime)?;
    Ok(row_decoder)
}

pub(super) fn append_bounded_packbits_decoder(
    kinds: &mut Vec<K>,
    body_runtime: u32,
) -> Result<(), QuestGlyphCacheHookPlanError> {
    let decoder = kinds.len();
    kinds.extend([
        K::Addu {
            rd: Reg::T0,
            rs: Reg::A2,
            rt: Reg::ZERO,
        },
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::A2,
            imm: QUEST_GLYPH_CACHE_SCRATCH_BYTES as i16,
        },
        K::Beq {
            rs: Reg::A0,
            rt: Reg::A1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let empty_decoder_branch = decoder + 2;
    let decoder_loop = kinds.len();
    kinds.extend([
        K::Lbu {
            rt: Reg::T2,
            base: Reg::A0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 1,
        },
        K::Sltiu {
            rt: Reg::T3,
            rs: Reg::T2,
            imm: 128,
        },
        K::Beq {
            rs: Reg::T3,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::T2,
            imm: 1,
        },
        K::Addu {
            rd: Reg::T3,
            rs: Reg::A0,
            rt: Reg::T2,
        },
        K::Sltu {
            rd: Reg::T4,
            rs: Reg::A1,
            rt: Reg::T3,
        },
        K::Bne {
            rs: Reg::T4,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Addu {
            rd: Reg::T3,
            rs: Reg::T0,
            rt: Reg::T2,
        },
        K::Sltu {
            rd: Reg::T4,
            rs: Reg::T1,
            rt: Reg::T3,
        },
        K::Bne {
            rs: Reg::T4,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let repeat_branch = decoder_loop + 3;
    let literal_source_bound_branch = decoder_loop + 8;
    let literal_output_bound_branch = decoder_loop + 12;
    let literal_loop = kinds.len();
    kinds.extend([
        K::Lbu {
            rt: Reg::T4,
            base: Reg::A0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 1,
        },
        K::Sb {
            rt: Reg::T4,
            base: Reg::T0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 1,
        },
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::T2,
            imm: -1,
        },
        K::Bgtz {
            rs: Reg::T2,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::A0,
            rt: Reg::A1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let literal_loop_branch = literal_loop + 5;
    let literal_finish_branch = literal_loop + 7;
    let literal_next_jump = literal_loop + 9;

    let repeat = kinds.len();
    kinds.extend([
        K::Addiu {
            rt: Reg::T3,
            rs: Reg::ZERO,
            imm: 128,
        },
        K::Beq {
            rs: Reg::T2,
            rt: Reg::T3,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::A0,
            rt: Reg::A1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Lbu {
            rt: Reg::T4,
            base: Reg::A0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 1,
        },
        K::Addiu {
            rt: Reg::T3,
            rs: Reg::ZERO,
            imm: 257,
        },
        K::Subu {
            rd: Reg::T2,
            rs: Reg::T3,
            rt: Reg::T2,
        },
        K::Addu {
            rd: Reg::T3,
            rs: Reg::T0,
            rt: Reg::T2,
        },
        K::Sltu {
            rd: Reg::T5,
            rs: Reg::T1,
            rt: Reg::T3,
        },
        K::Bne {
            rs: Reg::T5,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let reserved_control_branch = repeat + 1;
    let repeat_value_bound_branch = repeat + 3;
    let repeat_output_bound_branch = repeat + 11;
    let repeat_loop = kinds.len();
    kinds.extend([
        K::Sb {
            rt: Reg::T4,
            base: Reg::T0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::T0,
            imm: 1,
        },
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::T2,
            imm: -1,
        },
        K::Bgtz {
            rs: Reg::T2,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::Beq {
            rs: Reg::A0,
            rt: Reg::A1,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
        K::J { target: 0 },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let repeat_loop_branch = repeat_loop + 3;
    let repeat_finish_branch = repeat_loop + 5;
    let repeat_next_jump = repeat_loop + 7;

    let decoder_finish = kinds.len();
    kinds.extend([
        K::Bne {
            rs: Reg::T0,
            rt: Reg::T1,
            offset: 0,
        },
        K::Addu {
            rd: Reg::V0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let decoder_length_branch = decoder_finish;
    let decoder_error = kinds.len();
    kinds.extend([
        K::Addu {
            rd: Reg::V0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);

    for (source, target) in [
        (empty_decoder_branch, decoder_finish),
        (repeat_branch, repeat),
        (literal_source_bound_branch, decoder_error),
        (literal_output_bound_branch, decoder_error),
        (literal_loop_branch, literal_loop),
        (literal_finish_branch, decoder_finish),
        (reserved_control_branch, decoder_error),
        (repeat_value_bound_branch, decoder_error),
        (repeat_output_bound_branch, decoder_error),
        (repeat_loop_branch, repeat_loop),
        (repeat_finish_branch, decoder_finish),
        (decoder_length_branch, decoder_error),
    ] {
        patch_branch(kinds, source, target)?;
    }
    for (source, target) in [
        (literal_next_jump, decoder_loop),
        (repeat_next_jump, decoder_loop),
    ] {
        patch_jump(kinds, source, target, body_runtime)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "decoders_tests.rs"]
mod tests;
