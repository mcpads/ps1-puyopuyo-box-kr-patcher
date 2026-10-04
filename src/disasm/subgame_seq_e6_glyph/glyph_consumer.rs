//! E6 glyph descriptor, cell setup과 short renderer 소비 계약.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::{linear, BtxCellAnalysis, BtxRendererFamily};
use super::binary_checks::{high_low, read_u32, require};
use super::{error, SubgameSeqE6GlyphError, SubgameSeqE6GlyphSpec};

fn verify_descriptor_root(
    instructions: &[Instruction],
    map: &ExeMap,
    spec: SubgameSeqE6GlyphSpec,
) -> Result<(), SubgameSeqE6GlyphError> {
    let (root_high, root_low) = high_low(spec.descriptor_root_runtime);
    let (base_high, base_low) = high_low(spec.descriptor_base_runtime);
    for (runtime, expected) in [
        (
            spec.descriptor_root_lui_runtime,
            K::Lui {
                rt: Reg::V1,
                imm: root_high,
            },
        ),
        (
            spec.descriptor_base_lui_runtime,
            K::Lui {
                rt: Reg::V0,
                imm: base_high,
            },
        ),
        (
            spec.descriptor_base_addiu_runtime,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: base_low,
            },
        ),
        (
            spec.descriptor_root_store_runtime,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: root_low,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }
    Ok(())
}

pub(super) fn verify_cell_setup_contract(
    map: &ExeMap,
    text: &[u8],
    spec: SubgameSeqE6GlyphSpec,
) -> Result<(), SubgameSeqE6GlyphError> {
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error(format!("{} E6 glyph EXE 정렬 불일치", spec.exe)))?;
    verify_descriptor_root(&instructions, map, spec)?;
    let wrapper = read_u32(
        text,
        map.text_offset(spec.opcode_28_slot_runtime)
            .ok_or_else(|| error("E6 glyph OP28 slot이 EXE 밖임"))?,
    )?;
    if wrapper != spec.opcode_28_wrapper_runtime {
        return Err(error(format!("{} E6 glyph OP28 wrapper 불일치", spec.exe)));
    }
    require(
        &instructions,
        map,
        spec.opcode_28_wrapper_runtime + 8,
        K::Jal {
            target: spec.subdispatcher_runtime,
        },
    )?;
    let subslot = spec
        .subdispatch_table_runtime
        .checked_add(0x12 * 4)
        .ok_or_else(|| error("E6 glyph subdispatch slot overflow"))?;
    let handler = read_u32(
        text,
        map.text_offset(subslot)
            .ok_or_else(|| error("E6 glyph subdispatch slot이 EXE 밖임"))?,
    )?;
    if handler != spec.cell_setup_handler_runtime {
        return Err(error(format!(
            "{} E6 glyph OP28/sub12 handler 불일치",
            spec.exe
        )));
    }
    for (runtime, expected) in [
        (
            spec.cell_table_pointer_load_runtime,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            spec.descriptor_cell_table_store_runtime,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            spec.cell_count_load_runtime,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            spec.descriptor_cell_count_store_runtime,
            K::Sh {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x20,
            },
        ),
    ] {
        require(&instructions, map, runtime, expected)?;
    }
    Ok(())
}

pub(super) fn verify_renderer(
    cells: &BtxCellAnalysis,
    spec: SubgameSeqE6GlyphSpec,
) -> Result<(), SubgameSeqE6GlyphError> {
    let matches = cells
        .consumers
        .iter()
        .filter(|consumer| {
            consumer.exe == spec.exe
                && consumer.renderer_family == Some(BtxRendererFamily::ShortRenderer)
                && consumer.function_candidate == Some(spec.renderer_runtime)
                && consumer.resource_field_offset == 0x6C
                && consumer.cell_field_offset == 0x6A
                && consumer.resource_load == spec.renderer_resource_load_runtime
                && consumer.cell_load == spec.renderer_cell_load_runtime
                && consumer.descriptor_table_load == spec.renderer_descriptor_root_load_runtime
                && consumer.cell_table_load == spec.renderer_cell_table_load_runtime
                && consumer.cell_entry == spec.renderer_cell_entry_runtime
                && consumer.core_entry_reached
        })
        .count();
    if matches != 1 {
        return Err(error(format!(
            "{} E6 glyph short renderer 일치 수가 1이 아님: {matches}",
            spec.exe
        )));
    }
    Ok(())
}
