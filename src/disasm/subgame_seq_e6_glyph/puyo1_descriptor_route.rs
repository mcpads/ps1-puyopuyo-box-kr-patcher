//! PUYO1 E6 glyph descriptor의 독립 census 결과 조립.

mod alias_census;
mod literal_sources;
mod object_routes;
mod static_initializer;

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{InstructionKind as K, Reg};

use super::super::{linear, recursive, xref, BtxNameAnalysis};
use super::binary_checks::{read_u32, runtime_slice};
use super::glyph_assets::cell;
use super::{
    error, hex, DescriptorCellCountWriter, P1StaticRoute, SubgameSeqE6GlyphError,
    SubgameSeqE6GlyphSpec, CELL_HEADER_BYTES, DESCRIPTOR_BYTES, P1_LITERAL_HALFWORD_20_STORES,
    P1_LITERAL_RESOURCE_006D_LOADS, P1_STATIC_INITIALIZER_CALLBACK_RUNTIME,
    P1_STATIC_INITIALIZER_CALL_SITES, P1_STATIC_INITIALIZER_ENTRY_REACHED_CALL_SITES,
    P1_STATIC_INITIALIZER_RESOURCE, P1_STATIC_INITIALIZER_RUNTIME,
    P1_STATIC_INITIALIZER_TABLE_PREFIX_SHA256, P1_STATIC_INITIALIZER_TABLE_RUNTIME,
};
use alias_census::verify_p1_descriptor_alias_census;
use literal_sources::{
    literal_non_stack_halfword_20_stores, literal_resource_006d_loads,
    paired_descriptor_cell_count_writers, verify_gpu_primitive_halfword_20_stores,
    verify_p1_cache_record_halfword_20_store, verify_p1_descriptor_root_census,
};
use object_routes::{
    verify_p1_allocator_census, verify_p1_direct_chr_object_routes,
    verify_p1_non_object_literal_006d_load,
};
use static_initializer::{verify_p1_static_initializer, verify_p1_static_initializer_calls};

#[cfg(test)]
pub(super) use alias_census::{p1_async_dispatch_incoming_a0_uses, p1_memory_accesses_with_base};
#[cfg(test)]
pub(super) use literal_sources::has_nearby_halfword_store;
#[cfg(test)]
pub(super) use literal_sources::{
    literal_non_stack_halfword_20_stores as test_literal_non_stack_halfword_20_stores,
    literal_resource_006d_loads as test_literal_resource_006d_loads,
    paired_descriptor_cell_count_writers as test_paired_descriptor_cell_count_writers,
};

pub(super) fn verify_p1_static_route(
    map: &ExeMap,
    text: &[u8],
    names: &BtxNameAnalysis,
    spec: SubgameSeqE6GlyphSpec,
) -> Result<P1StaticRoute, SubgameSeqE6GlyphError> {
    let instructions =
        linear::sweep(map, text).ok_or_else(|| error("PUYO1 descriptor writer EXE 정렬 불일치"))?;
    let writers = paired_descriptor_cell_count_writers(&instructions);
    let expected_writers = vec![
        DescriptorCellCountWriter {
            cell_store_runtime: 0x8001_B170,
            count_store_runtime: 0x8001_B184,
            base: Reg::S1,
            cell_source: Reg::V0,
            count_source: Reg::V0,
        },
        DescriptorCellCountWriter {
            cell_store_runtime: 0x8001_B1A0,
            count_store_runtime: 0x8001_B1AC,
            base: Reg::V0,
            cell_source: Reg::V1,
            count_source: Reg::V1,
        },
        DescriptorCellCountWriter {
            cell_store_runtime: 0x8002_A2F4,
            count_store_runtime: 0x8002_A2F8,
            base: Reg::V1,
            cell_source: Reg::V0,
            count_source: Reg::ZERO,
        },
    ];
    if writers != expected_writers {
        return Err(error(format!(
            "PUYO1 paired descriptor cell/count writer 분모 불일치: {writers:?}"
        )));
    }
    let literal_halfword_20_stores = literal_non_stack_halfword_20_stores(&instructions);
    if literal_halfword_20_stores != P1_LITERAL_HALFWORD_20_STORES {
        return Err(error(format!(
            "PUYO1 non-stack literal sh +0x20 분모 불일치: {literal_halfword_20_stores:?}"
        )));
    }
    let descriptor_literal_halfword_20_stores = writers
        .iter()
        .map(|writer| writer.count_store_runtime)
        .collect::<Vec<_>>();
    let gpu_primitive_halfword_20_stores = verify_gpu_primitive_halfword_20_stores(&instructions)?;
    let cache_record_halfword_20_stores =
        verify_p1_cache_record_halfword_20_store(&instructions, map, text)?;
    let classified = descriptor_literal_halfword_20_stores
        .iter()
        .chain(&gpu_primitive_halfword_20_stores)
        .chain(&cache_record_halfword_20_stores)
        .copied()
        .collect::<BTreeSet<_>>();
    let literal = literal_halfword_20_stores
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if classified != literal {
        return Err(error(format!(
            "PUYO1 non-stack literal sh +0x20 분류 불완전: literal={literal:?}, classified={classified:?}"
        )));
    }
    let literal_resource_006d_loads = literal_resource_006d_loads(&instructions);
    if literal_resource_006d_loads != P1_LITERAL_RESOURCE_006D_LOADS {
        return Err(error(format!(
            "PUYO1 literal resource 0x006D 분모 불일치: {literal_resource_006d_loads:?}"
        )));
    }
    let (
        direct_chr_object_resource_stores,
        fixed_cell_chr_object_routes,
        variable_cell_chr_object_routes,
    ) = verify_p1_direct_chr_object_routes(&instructions, map)?;
    let non_object_literal_006d_loads = verify_p1_non_object_literal_006d_load(&instructions, map)?;
    let classified_resource_loads = direct_chr_object_resource_stores
        .iter()
        .map(|store| store - 4)
        .chain(non_object_literal_006d_loads.iter().copied())
        .collect::<BTreeSet<_>>();
    let literal_resource_load_set = literal_resource_006d_loads
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if classified_resource_loads != literal_resource_load_set {
        return Err(error(format!(
            "PUYO1 literal resource 0x006D 분류 불완전: literal={literal_resource_load_set:?}, classified={classified_resource_loads:?}"
        )));
    }
    let allocator_census = verify_p1_allocator_census(&instructions, map)?;
    let descriptor_root_census = verify_p1_descriptor_root_census(&instructions, map, text, spec)?;
    let descriptor_alias_census = verify_p1_descriptor_alias_census(&instructions, map, text)?;
    verify_p1_static_initializer(&instructions, map)?;
    verify_p1_static_initializer_calls(&instructions, map, spec)?;

    let direct_call_sites = instructions
        .iter()
        .filter_map(|instruction| {
            (instruction.kind
                == (K::Jal {
                    target: P1_STATIC_INITIALIZER_RUNTIME,
                }))
            .then_some(instruction.addr)
        })
        .collect::<Vec<_>>();
    if direct_call_sites != P1_STATIC_INITIALIZER_CALL_SITES {
        return Err(error(format!(
            "PUYO1 static initializer 직접 호출 분모 불일치: {direct_call_sites:?}"
        )));
    }

    let entry_trace = recursive::trace(map, text, &[]);
    let entry_reached_call_sites = direct_call_sites
        .iter()
        .copied()
        .filter(|runtime| entry_trace.instructions.contains_key(runtime))
        .collect::<Vec<_>>();
    if entry_reached_call_sites != P1_STATIC_INITIALIZER_ENTRY_REACHED_CALL_SITES
        || !entry_trace
            .instructions
            .contains_key(&P1_STATIC_INITIALIZER_RUNTIME)
    {
        return Err(error(format!(
            "PUYO1 static initializer entry trace 불일치: {entry_reached_call_sites:?}"
        )));
    }

    let address_loads = xref::find_address_loads(&instructions, xref::DEFAULT_WINDOW);
    let data_refs = address_loads
        .iter()
        .filter(|load| load.value == P1_STATIC_INITIALIZER_RUNTIME)
        .count();
    if data_refs != 0 {
        return Err(error(format!(
            "PUYO1 static initializer data address 참조가 생김: {data_refs}"
        )));
    }
    let table_refs = address_loads
        .iter()
        .filter(|load| load.value == P1_STATIC_INITIALIZER_TABLE_RUNTIME)
        .collect::<Vec<_>>();
    if table_refs.len() != 1
        || table_refs[0].lui_addr != 0x8002_A2EC
        || table_refs[0].combine_addr != 0x8002_A2F0
    {
        return Err(error(format!(
            "PUYO1 static initializer table 참조 불일치: {table_refs:?}"
        )));
    }
    let raw_pointer_words = text
        .chunks_exact(4)
        .filter(|word| {
            u32::from_le_bytes((*word).try_into().expect("4바이트 word"))
                == P1_STATIC_INITIALIZER_RUNTIME
        })
        .count();
    if raw_pointer_words != 0 {
        return Err(error(format!(
            "PUYO1 static initializer raw pointer word가 생김: {raw_pointer_words}"
        )));
    }

    if runtime_slice(map, text, P1_STATIC_INITIALIZER_CALLBACK_RUNTIME, 4)? != [0; 4] {
        return Err(error("PUYO1 static initializer callback 원문이 0이 아님"));
    }
    let table_prefix = runtime_slice(map, text, P1_STATIC_INITIALIZER_TABLE_RUNTIME, 24)?;
    if cell(table_prefix, 0)? != (0, 0, 128, 24, 0, 0)
        || cell(table_prefix, CELL_HEADER_BYTES)? != (0, 0, 128, 24, 0, 0)
        || hex(&Sha256::digest(table_prefix)) != P1_STATIC_INITIALIZER_TABLE_PREFIX_SHA256
    {
        return Err(error("PUYO1 static initializer table prefix 불일치"));
    }

    let descriptor_runtime = spec
        .descriptor_base_runtime
        .checked_add(u32::from(P1_STATIC_INITIALIZER_RESOURCE) * DESCRIPTOR_BYTES)
        .ok_or_else(|| error("PUYO1 static initializer descriptor overflow"))?;
    let descriptor_offset = map
        .text_offset(descriptor_runtime)
        .ok_or_else(|| error("PUYO1 static initializer descriptor가 EXE 밖임"))?;
    let expected_descriptor = [0x0001_0000, 0, 0, 0, 0, 0, 0, 0, 0];
    for (index, expected) in expected_descriptor.into_iter().enumerate() {
        if read_u32(text, descriptor_offset + index * 4)? != expected {
            return Err(error(format!(
                "PUYO1 static initializer descriptor word {index} 불일치"
            )));
        }
    }
    let registered_assets = names
        .links
        .iter()
        .filter(|link| {
            link.exe == spec.exe && link.descriptor_pointer_runtime == descriptor_runtime + 4
        })
        .count();
    if registered_assets != 0 {
        return Err(error(format!(
            "PUYO1 static initializer resource 0xB4에 BTX 이름이 연결됨: {registered_assets}"
        )));
    }

    Ok(P1StaticRoute {
        paired_writers: writers.len(),
        literal_halfword_20_stores,
        descriptor_literal_halfword_20_stores,
        gpu_primitive_halfword_20_stores,
        cache_record_halfword_20_stores,
        literal_resource_006d_loads,
        direct_chr_object_resource_stores,
        fixed_cell_chr_object_routes,
        variable_cell_chr_object_routes,
        non_object_literal_006d_loads,
        allocator_direct_calls: allocator_census.direct_calls,
        allocator_immediate_type_calls: allocator_census.immediate_type_calls,
        allocator_dynamic_type_calls: allocator_census.dynamic_type_calls,
        immediate_type9_calls: allocator_census.immediate_type9_calls,
        direct_chr_object_allocator_calls: allocator_census.direct_chr_object_calls,
        inactive_type9_calls: allocator_census.inactive_type9_calls,
        descriptor_root_literal_combines: descriptor_root_census.literal_combines,
        descriptor_root_resource_x36_selectors: descriptor_root_census.resource_x36_selectors,
        descriptor_root_alternate_branch_combines: descriptor_root_census.alternate_branch_combines,
        descriptor_root_registration_stores: descriptor_root_census.registration_stores,
        chr_descriptor_direct_address_loads: descriptor_root_census
            .chr_descriptor_direct_address_loads,
        chr_descriptor_raw_pointer_words: descriptor_root_census.chr_descriptor_raw_pointer_words,
        descriptor_table_records: descriptor_root_census.descriptor_table_records,
        descriptor_table_sha256: descriptor_root_census.descriptor_table_sha256,
        descriptor_table_address_loads: descriptor_root_census.descriptor_table_address_loads,
        descriptor_table_preadjusted_address_loads: descriptor_root_census
            .descriptor_table_preadjusted_address_loads,
        descriptor_table_raw_pointer_locations: descriptor_root_census
            .descriptor_table_raw_pointer_locations,
        descriptor_table_preadjusted_raw_pointers: descriptor_root_census
            .descriptor_table_preadjusted_raw_pointers,
        descriptor_selector_direct_callee_calls: descriptor_alias_census.direct_callee_calls,
        descriptor_selector_local_consumers: descriptor_alias_census.local_consumers,
        descriptor_selector_unique_callees: descriptor_alias_census.unique_callees,
        descriptor_forwarding_calls: descriptor_alias_census.forwarding_calls,
        descriptor_forwarding_unique_callees: descriptor_alias_census.forwarding_unique_callees,
        descriptor_synchronous_field_reads: descriptor_alias_census.synchronous_field_reads,
        descriptor_synchronous_field_writes: descriptor_alias_census.synchronous_field_writes,
        descriptor_pointer_escape_stores: descriptor_alias_census.pointer_escape_stores,
        descriptor_cache_link_stores: descriptor_alias_census.cache_link_stores,
        descriptor_fixed_cache_callbacks: descriptor_alias_census.fixed_cache_callbacks,
        descriptor_fixed_callback_writes: descriptor_alias_census.fixed_callback_writes,
        descriptor_fixed_callback_cell_reads: descriptor_alias_census.fixed_callback_cell_reads,
        descriptor_async_dispatch_targets: descriptor_alias_census.async_dispatch_targets,
        descriptor_async_dispatch_incoming_uses: descriptor_alias_census
            .async_dispatch_incoming_uses,
        descriptor_singleton_callback_writes: descriptor_alias_census.singleton_callback_writes,
        descriptor_cache_callbacks: descriptor_alias_census.cache_callbacks,
        descriptor_callback_copy_loops: descriptor_alias_census.callback_copy_loops,
        descriptor_completion_callback_field_writes: descriptor_alias_census
            .completion_callback_field_writes,
        descriptor_completion_descriptor_writes: descriptor_alias_census
            .completion_descriptor_writes,
        descriptor_type_cache_root_combines: descriptor_alias_census.type_cache_root_combines,
        descriptor_singleton_snapshot_root_combines: descriptor_alias_census
            .singleton_snapshot_root_combines,
        descriptor_type_snapshot_root_combines: descriptor_alias_census.type_snapshot_root_combines,
        descriptor_payload_copy_loops: descriptor_alias_census.payload_copy_loops,
        descriptor_snapshot_field_writes: descriptor_alias_census.snapshot_field_writes,
        descriptor_type_snapshot_link_stores: descriptor_alias_census.type_snapshot_link_stores,
        descriptor_snapshot_raw_pointer_words: descriptor_alias_census.snapshot_raw_pointer_words,
        setup_source_cell_writer_runtime: writers[0].cell_store_runtime,
        cache_mirror_cell_writer_runtime: writers[1].cell_store_runtime,
        initializer_cell_writer_runtime: writers[2].cell_store_runtime,
        direct_call_sites,
        entry_reached_call_sites,
        data_refs,
        raw_pointer_words,
        descriptor_runtime,
        registered_assets,
        cell_count: 0,
        cell_width: 128,
        cell_height: 24,
    })
}
