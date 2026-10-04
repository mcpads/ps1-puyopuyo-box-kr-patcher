//! SPRT 직전에 같은 OT 연결 안에 DR_TPAGE를 넣어 texture page를 명시한다.
//! 원본 SPRT의 UV/크기를 유지하므로 atlas 마지막 행의 256→0 UV 보간을 피한다.

use super::super::{
    QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE, QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE,
};
use super::{patch_branch, QuestGlyphCacheHookPlanError};
use crate::isa::{InstructionKind as K, Reg};

pub(super) fn append_sprite_texture_page(
    k: &mut Vec<K>,
) -> Result<usize, QuestGlyphCacheHookPlanError> {
    let entry = k.len();
    // V0는 OT bucket, A1은 OT base, S0는 완성된 20-byte SPRT다.
    k.extend([
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
        K::Addu {
            rd: Reg::T0,
            rs: Reg::V0,
            rt: Reg::A1,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: -4,
        },
        K::Lw {
            rt: Reg::V1,
            base: Reg::S0,
            offset: 0,
        },
        K::And {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::T3,
        },
        K::And {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::T4,
        },
        K::Or {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::V0,
        },
        K::Sw {
            rt: Reg::V1,
            base: Reg::S0,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::T1,
            rs: Reg::S0,
            imm: 20,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::T1,
            imm: 8,
        },
        K::Lui {
            rt: Reg::AT,
            imm: 0x800C,
        },
        K::Sw {
            rt: Reg::V0,
            base: Reg::AT,
            offset: 0xE4D0u16 as i16,
        },
        K::And {
            rd: Reg::V1,
            rs: Reg::S0,
            rt: Reg::T3,
        },
        K::Lui {
            rt: Reg::V0,
            imm: 0x0100,
        },
        K::Or {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::V0,
        },
        K::Sw {
            rt: Reg::V1,
            base: Reg::T1,
            offset: 0,
        },
        K::Lbu {
            rt: Reg::V0,
            base: Reg::S1,
            offset: 0xDD,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let native = k.len();
    k.extend([
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x80,
        },
    ]);
    let primary = k.len();
    k.extend([
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE as i16,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE as i16,
        },
    ]);
    let secondary = k.len();
    k.extend([
        K::Beq {
            rs: Reg::ZERO,
            rt: Reg::ZERO,
            offset: 0,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let native_target = k.len();
    k.extend([
        K::Andi {
            rt: Reg::V0,
            rs: Reg::T2,
            imm: 0xFF,
        },
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 1,
        },
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::SP,
        },
        K::Lhu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0x38,
        },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    let texture_ready = k.len();
    // 원본 SetDrawTPage(a1=0,a2=0)과 같은 E1 mask다.
    k.extend([
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x09FF,
        },
        K::Lui {
            rt: Reg::V1,
            imm: 0xE100,
        },
        K::Or {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
        K::Sw {
            rt: Reg::V0,
            base: Reg::T1,
            offset: 4,
        },
        K::Lw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: -4,
        },
        K::And {
            rd: Reg::V1,
            rs: Reg::T1,
            rt: Reg::T3,
        },
        K::And {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::T4,
        },
        K::Or {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
        K::J {
            target: 0x8002_B910,
        },
        K::Sw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: -4,
        },
    ]);
    patch_branch(k, native, native_target)?;
    patch_branch(k, primary, texture_ready)?;
    patch_branch(k, secondary, texture_ready)?;
    Ok(entry)
}
