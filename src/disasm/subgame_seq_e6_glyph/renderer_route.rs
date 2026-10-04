//! E6 glyph object의 생성, 생명주기와 short renderer 도달 검증.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::{linear, recursive};
use super::binary_checks::{high_low, require};
use super::{
    error, E6ObjectRendererRoute, SubgameSeqE6GlyphError, SubgameSeqE6GlyphSpec,
    E6_OBJECT_RENDERER_ROUTE_SPECS, OBJECT_POOL_COUNT, OBJECT_POOL_ZERO_WORDS, OBJECT_WORDS,
};

pub(super) fn verify_zero_words_function(
    instructions: &[Instruction],
    map: &ExeMap,
    runtime: u32,
) -> Result<(), SubgameSeqE6GlyphError> {
    for (delta, expected) in [
        (
            0,
            K::Beq {
                rs: Reg::A2,
                rt: Reg::ZERO,
                offset: 6,
            },
        ),
        (
            4,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            8,
            K::Sw {
                rt: Reg::A1,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            12,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 1,
            },
        ),
        (
            16,
            K::Sltu {
                rd: Reg::V0,
                rs: Reg::V1,
                rt: Reg::A2,
            },
        ),
        (
            20,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -4,
            },
        ),
        (
            24,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 4,
            },
        ),
        (28, K::Jr { rs: Reg::RA }),
    ] {
        require(instructions, map, runtime + delta, expected)?;
    }
    Ok(())
}

pub(super) fn verify_e6_object_renderer_route(
    map: &ExeMap,
    text: &[u8],
    glyph: SubgameSeqE6GlyphSpec,
) -> Result<E6ObjectRendererRoute, SubgameSeqE6GlyphError> {
    let route = E6_OBJECT_RENDERER_ROUTE_SPECS
        .iter()
        .copied()
        .find(|route| route.scope == glyph.scope)
        .ok_or_else(|| {
            error(format!(
                "{} E6 object renderer route 사양 누락",
                glyph.scope
            ))
        })?;
    let instructions =
        linear::sweep(map, text).ok_or_else(|| error("E6 object route EXE 정렬 불일치"))?;
    let (callback_high, callback_low) = high_low(route.callback_runtime);
    let (pool_high, pool_low) = high_low(route.pool_runtime);
    let (flags_high, flags_low) = high_low(route.type_flags_runtime);
    let (list_high, list_low) = high_low(route.object_list_runtime);

    for (runtime, expected) in [
        (
            route.callback_high_runtime,
            K::Lui {
                rt: Reg::A0,
                imm: callback_high,
            },
        ),
        (
            route.callback_low_runtime,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: callback_low,
            },
        ),
        (
            route.allocator_call_runtime,
            K::Jal {
                target: route.allocator_runtime,
            },
        ),
        (
            route.allocator_call_runtime + 4,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: route.allocator_type,
            },
        ),
        (
            route.active_store_runtime - 4,
            K::Addiu {
                rt: route.active_value_reg,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            route.active_store_runtime,
            K::Sh {
                rt: route.active_value_reg,
                base: Reg::V0,
                offset: 0x70,
            },
        ),
        (
            route.resource_value_runtime,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: glyph.glyph_resource as i16,
            },
        ),
        (
            route.resource_store_runtime,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x6C,
            },
        ),
        (
            route.cell_store_runtime,
            K::Sh {
                rt: Reg::S1,
                base: Reg::V0,
                offset: 0x6A,
            },
        ),
        (
            route.pool_init_runtime + 8,
            K::Lui {
                rt: Reg::S0,
                imm: pool_high,
            },
        ),
        (
            route.pool_init_runtime + 12,
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S0,
                imm: pool_low,
            },
        ),
        (
            route.pool_init_runtime + 28,
            K::Jal {
                target: route.zero_words_runtime,
            },
        ),
        (
            route.pool_init_runtime + 32,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: OBJECT_POOL_ZERO_WORDS,
            },
        ),
        (
            route.pool_init_runtime + 36,
            K::Lui {
                rt: Reg::A0,
                imm: flags_high,
            },
        ),
        (
            route.pool_init_runtime + 40,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: flags_low,
            },
        ),
        (
            route.pool_init_runtime + 48,
            K::Jal {
                target: route.zero_words_runtime,
            },
        ),
        (
            route.pool_init_runtime + 52,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 16,
            },
        ),
        (
            route.pool_init_runtime + 76,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::A0,
                imm: OBJECT_POOL_COUNT,
            },
        ),
        (
            route.pool_init_runtime + 84,
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S0,
                imm: 0x00EC,
            },
        ),
        (
            route.pool_init_runtime + 88,
            K::Lui {
                rt: Reg::V0,
                imm: pool_high,
            },
        ),
        (
            route.pool_init_runtime + 92,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: pool_low,
            },
        ),
        (
            route.pool_init_runtime + 96,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: route.free_head_gp_offset,
            },
        ),
        (
            route.pool_init_runtime + 104,
            K::Lui {
                rt: Reg::V0,
                imm: list_high,
            },
        ),
        (
            route.pool_init_runtime + 108,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: list_low,
            },
        ),
        (
            route.allocator_runtime,
            K::Lw {
                rt: Reg::A2,
                base: Reg::GP,
                offset: route.free_head_gp_offset,
            },
        ),
        (
            route.allocator_runtime + 28,
            K::Sw {
                rt: Reg::A0,
                base: Reg::A2,
                offset: 0x18,
            },
        ),
        (
            route.allocator_runtime + 32,
            K::Lui {
                rt: Reg::A0,
                imm: list_high,
            },
        ),
        (
            route.allocator_runtime + 36,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: list_low,
            },
        ),
        (
            route.allocator_runtime + 44,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x00FF,
            },
        ),
        (
            route.allocator_runtime + 52,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V1,
                sa: 1,
            },
        ),
        (
            route.allocator_runtime + 56,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            route.allocator_runtime + 60,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 3,
            },
        ),
        (
            route.allocator_runtime + 72,
            K::Sb {
                rt: Reg::A1,
                base: Reg::A2,
                offset: 0x41,
            },
        ),
        (
            route.allocator_runtime + 140,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0x78,
            },
        ),
        (
            route.callback_release_call_runtime,
            K::Jal {
                target: route.release_runtime,
            },
        ),
        (
            route.release_runtime + 32,
            K::Addu {
                rd: Reg::A1,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            route.release_runtime + 48,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: OBJECT_WORDS,
            },
        ),
        (
            route.release_runtime + 52,
            K::Jal {
                target: route.zero_words_runtime,
            },
        ),
        (
            route.render_dispatcher_runtime + 48,
            K::Addu {
                rd: Reg::S6,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            route.short_renderer_call_runtime - 240,
            K::Lui {
                rt: Reg::V0,
                imm: list_high,
            },
        ),
        (
            route.short_renderer_call_runtime - 236,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: list_low,
            },
        ),
        (
            route.short_renderer_call_runtime - 224,
            K::Lui {
                rt: Reg::V0,
                imm: flags_high,
            },
        ),
        (
            route.short_renderer_call_runtime - 220,
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::V0,
                imm: flags_low,
            },
        ),
        (
            route.short_renderer_call_runtime - 216,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0,
            },
        ),
        (
            route.short_renderer_call_runtime - 208,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            route.short_renderer_call_runtime - 196,
            K::Lw {
                rt: Reg::S0,
                base: Reg::S3,
                offset: 8,
            },
        ),
        (
            route.short_renderer_call_runtime - 172,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x70,
            },
        ),
        (
            route.short_renderer_call_runtime - 164,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 1,
            },
        ),
        (
            route.short_renderer_call_runtime - 152,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x78,
            },
        ),
        (
            route.short_renderer_call_runtime - 40,
            K::Lh {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x68,
            },
        ),
        (
            route.short_renderer_call_runtime - 28,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 0x0400,
            },
        ),
        (
            route.short_renderer_call_runtime,
            K::Jal {
                target: glyph.renderer_runtime,
            },
        ),
        (
            route.short_renderer_call_runtime + 40,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::S6,
                imm: 16,
            },
        ),
    ] {
        require(&instructions, map, runtime, expected)?;
    }
    verify_zero_words_function(&instructions, map, route.zero_words_runtime)?;

    let object_bytes = i32::from(OBJECT_WORDS) * 4;
    let initialized_pool_bytes = i32::from(OBJECT_POOL_ZERO_WORDS) * 4;
    let required_pool_bytes = i32::from(OBJECT_POOL_COUNT) * object_bytes;
    if object_bytes != 0xEC || initialized_pool_bytes < required_pool_bytes {
        return Err(error(format!(
            "{} E6 object pool zero 범위가 object pool을 덮지 못함",
            glyph.scope
        )));
    }
    let emitter_overwrites_renderer_selector = instructions.iter().any(|instruction| {
        instruction.addr > route.allocator_call_runtime
            && instruction.addr <= route.cell_store_runtime + 16
            && matches!(
                instruction.kind,
                K::Sb {
                    base: Reg::V0,
                    offset: 0x68,
                    ..
                } | K::Sh {
                    base: Reg::V0,
                    offset: 0x68,
                    ..
                } | K::Sw {
                    base: Reg::V0,
                    offset: 0x68,
                    ..
                }
            )
    });
    if emitter_overwrites_renderer_selector {
        return Err(error(format!(
            "{} E6 emitter가 object +0x68 renderer selector를 덮음",
            glyph.scope
        )));
    }
    let callback_writes_object = instructions.iter().any(|instruction| {
        instruction.addr >= route.callback_runtime
            && instruction.addr <= route.callback_release_call_runtime
            && matches!(
                instruction.kind,
                K::Sb { base: Reg::A0, .. }
                    | K::Sh { base: Reg::A0, .. }
                    | K::Sw { base: Reg::A0, .. }
            )
    });
    if callback_writes_object {
        return Err(error(format!(
            "{} E6 callback이 release 전에 object state를 덮음",
            glyph.scope
        )));
    }

    let entry_trace = recursive::trace(map, text, &[]);
    let dispatcher_entry_reached = entry_trace
        .instructions
        .contains_key(&route.render_dispatcher_runtime)
        && entry_trace
            .instructions
            .contains_key(&route.short_renderer_call_runtime);
    if !dispatcher_entry_reached {
        return Err(error(format!(
            "{} E6 short renderer dispatcher가 entry trace에 없음",
            glyph.scope
        )));
    }
    Ok(E6ObjectRendererRoute {
        spec: route,
        dispatcher_entry_reached,
    })
}
