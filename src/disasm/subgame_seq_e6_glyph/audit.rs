//! E6 glyph의 독립 검증 결과를 scope별 최종 행으로 조립.

use std::collections::{BTreeMap, BTreeSet};

use crate::archive::VddArchive;
use crate::coords::ExeMap;
use crate::font::verify_box_demo_font;
use crate::script::{SubgameSeqLayoutAnalysis, SubgameSeqLayoutKind};

use super::super::{BtxCellAnalysis, BtxNameAnalysis, SubgameSeqE6TextAnalysis};
use super::glyph_assets::{
    cell, code_table_indices, setup_records, verify_asset, verify_p1_candidate,
    verify_p1_candidate_coverage, verify_p2_table,
};
use super::glyph_consumer::{verify_cell_setup_contract, verify_renderer};
use super::puyo1_descriptor_route::verify_p1_static_route;
use super::renderer_route::verify_e6_object_renderer_route;
use super::sequence_storage::{
    verify_e6_sequence_selection_census, verify_e6_sequence_storage_census,
};
use super::{
    error, GlyphAssetContract, SubgameSeqE6GlyphAnalysis, SubgameSeqE6GlyphError,
    SubgameSeqE6GlyphRow, BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_CELLS, BOX_SUBGAME_SEQ_E6_GLYPH_SPECS,
    DESCRIPTOR_BYTES, P1_STATIC_INITIALIZER_RESOURCE, P1_STATIC_INITIALIZER_RUNTIME,
    P1_STATIC_INITIALIZER_TABLE_RUNTIME,
};

pub fn audit_subgame_seq_e6_glyphs<'a>(
    executables: impl IntoIterator<Item = (&'a ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
    layouts: &SubgameSeqLayoutAnalysis,
    e6: &SubgameSeqE6TextAnalysis,
    names: &BtxNameAnalysis,
    cells: &BtxCellAnalysis,
) -> Result<SubgameSeqE6GlyphAnalysis, SubgameSeqE6GlyphError> {
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    let source_map = sources
        .into_iter()
        .map(|(archive, sequence, raw)| ((archive, sequence), raw))
        .collect::<BTreeMap<_, _>>();
    let archive_map = archives.into_iter().collect::<BTreeMap<_, _>>();
    if executable_map.len() != BOX_SUBGAME_SEQ_E6_GLYPH_SPECS.len()
        || source_map.len() != layouts.rows.len()
        || archive_map.len() != BOX_SUBGAME_SEQ_E6_GLYPH_SPECS.len() * 2
    {
        return Err(error("E6 glyph 입력 분모 불일치"));
    }

    let mut rows = Vec::new();
    let mut common_indices = None::<BTreeSet<usize>>;
    for spec in BOX_SUBGAME_SEQ_E6_GLYPH_SPECS {
        let &(map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} E6 glyph EXE 누락", spec.exe)))?;
        if text.len() != map.text_len as usize {
            return Err(error(format!("{} E6 glyph EXE 길이 불일치", spec.exe)));
        }
        verify_cell_setup_contract(map, text, spec)?;
        let object_renderer_route = verify_e6_object_renderer_route(map, text, spec)?;
        verify_renderer(cells, spec)?;
        let p1_static_route = (spec.scope == "PUYO1")
            .then(|| verify_p1_static_route(map, text, names, spec))
            .transpose()?;
        let archive = archive_map
            .get(spec.grp_archive)
            .ok_or_else(|| error(format!("{} E6 glyph GRP 누락", spec.grp_archive)))?;
        let (glyph_digest, glyph_plain) = verify_asset(
            map,
            text,
            archive,
            names,
            spec,
            GlyphAssetContract {
                resource: spec.glyph_resource,
                name: spec.glyph_asset,
                vdd_index: spec.glyph_vdd_index,
                stored_bytes: spec.glyph_stored_bytes,
                plain_bytes: spec.glyph_plain_bytes,
                plain_sha256: spec.glyph_plain_sha256,
            },
        )?;
        if let (
            Some(resource),
            Some(asset),
            Some(vdd_index),
            Some(stored_bytes),
            Some(plain_sha256),
        ) = (
            spec.rejected_resource,
            spec.rejected_asset,
            spec.rejected_vdd_index,
            spec.rejected_stored_bytes,
            spec.rejected_plain_sha256,
        ) {
            let _ = verify_asset(
                map,
                text,
                archive,
                names,
                spec,
                GlyphAssetContract {
                    resource,
                    name: asset,
                    vdd_index,
                    stored_bytes,
                    plain_bytes: 65_536,
                    plain_sha256,
                },
            )?;
        }

        let (physical_glyph_cells, verified_e6_nonempty_cells) = if spec.scope == "PUYO2" {
            let summary = verify_box_demo_font(&glyph_plain)
                .map_err(|source| error(format!("PUYO2 E6 physical DEMOFONT 실패: {source}")))?;
            let mapped_empty = summary
                .empty_slots
                .iter()
                .filter(|index| **index < BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_CELLS)
                .count();
            (
                summary.slots,
                BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_CELLS - mapped_empty,
            )
        } else {
            (0, 0)
        };

        let scope_rows = e6
            .rows
            .iter()
            .filter(|row| row.scope == spec.scope)
            .collect::<Vec<_>>();
        let e6_sequences = scope_rows
            .iter()
            .map(|row| row.sequence.as_str())
            .collect::<BTreeSet<_>>();
        let e6_selection =
            verify_e6_sequence_selection_census(map, text, &source_map, &e6_sequences, spec)?;
        let seq_archive = archive_map
            .get(spec.seq_archive)
            .ok_or_else(|| error(format!("{} E6 sequence VDD 누락", spec.seq_archive)))?;
        let e6_storage = verify_e6_sequence_storage_census(
            map,
            text,
            seq_archive,
            &source_map,
            layouts,
            &e6_selection.resources,
            spec,
        )?;
        let (glyph_tokens, used_indices) =
            code_table_indices(map, text, scope_rows.iter().map(|row| row.raw.as_slice()))?;
        if let Some(common) = &common_indices {
            if common != &used_indices {
                return Err(error("PUYO1/PUYO2 E6 used cell index 집합 불일치"));
            }
        } else {
            common_indices = Some(used_indices.clone());
        }

        let mut all_glyph_setups = 0usize;
        let mut e6_glyph_setups = 0usize;
        let mut rejected_setups = 0usize;
        let mut e6_table_digests = BTreeSet::new();
        let mut rejected_table_digests = BTreeSet::new();
        let mut candidate_record = None;
        let mut candidate_raw = None;
        for layout in layouts.rows.iter().filter(|layout| {
            layout.archive == spec.seq_archive && layout.kind == SubgameSeqLayoutKind::Overlay
        }) {
            let raw = source_map
                .get(&(layout.archive.as_str(), layout.sequence.as_str()))
                .ok_or_else(|| error("E6 glyph layout 원본 누락"))?;
            let footer = layout
                .footer
                .ok_or_else(|| error("E6 glyph overlay footer 누락"))?;
            let runtime_map = footer
                .runtime_map()
                .ok_or_else(|| error("E6 glyph overlay runtime 사상 overflow"))?;
            let initialized_len = usize::try_from(footer.initialized_len)
                .map_err(|_| error("E6 glyph initialized 길이 변환 실패"))?;
            let records = setup_records(raw, initialized_len);
            for record in records
                .iter()
                .filter(|record| record.resource == spec.glyph_resource)
            {
                if record.count != 0 || record.reserved != 0 {
                    return Err(error("E6 glyph setup reserved field가 0이 아님"));
                }
                all_glyph_setups += 1;
                if e6_sequences.contains(layout.sequence.as_str()) {
                    e6_glyph_setups += 1;
                    let offset = runtime_map
                        .file_offset(record.pointer)
                        .ok_or_else(|| error("E6 glyph setup pointer가 overlay 밖임"))?;
                    if Some(offset) != spec.expected_table_offset {
                        return Err(error(format!(
                            "{} E6 glyph table offset 불일치: +0x{offset:04X}",
                            spec.scope
                        )));
                    }
                    e6_table_digests.insert(verify_p2_table(raw, offset)?);
                }
                if spec.candidate_sequence == Some(layout.sequence.as_str()) {
                    candidate_record = Some(*record);
                    candidate_raw = Some(*raw);
                }
            }
            if e6_sequences.contains(layout.sequence.as_str()) {
                if let Some(resource) = spec.rejected_resource {
                    let matches = records
                        .iter()
                        .filter(|record| record.resource == resource)
                        .collect::<Vec<_>>();
                    if matches.len() != 1 {
                        return Err(error(format!(
                            "{} E6 rejected table record 수 불일치: {}:{}",
                            spec.scope,
                            layout.sequence,
                            matches.len()
                        )));
                    }
                    let record = matches[0];
                    if record.count != 0 || record.reserved != 0 {
                        return Err(error("E6 rejected setup reserved field가 0이 아님"));
                    }
                    let offset = runtime_map
                        .file_offset(record.pointer)
                        .ok_or_else(|| error("E6 rejected setup pointer가 overlay 밖임"))?;
                    if Some(offset) != spec.rejected_table_offset {
                        return Err(error("E6 rejected table offset 불일치"));
                    }
                    rejected_table_digests.insert(verify_p2_table(raw, offset)?);
                    rejected_setups += 1;
                }
            }
        }
        if all_glyph_setups != spec.expected_all_glyph_setups
            || e6_glyph_setups != spec.expected_e6_glyph_setups
        {
            return Err(error(format!(
                "{} E6 glyph setup 분모 불일치: all/e6={all_glyph_setups}/{e6_glyph_setups}",
                spec.scope
            )));
        }
        let expected_e6_table_variants = usize::from(e6_glyph_setups != 0);
        if e6_table_digests.len() != expected_e6_table_variants {
            return Err(error(format!(
                "{} E6 glyph table variant 수 불일치: {}",
                spec.scope,
                e6_table_digests.len()
            )));
        }
        let expected_rejected_table_variants = usize::from(rejected_setups != 0);
        if rejected_table_digests.len() != expected_rejected_table_variants {
            return Err(error(format!(
                "{} E6 rejected table variant 수 불일치: {}",
                spec.scope,
                rejected_table_digests.len()
            )));
        }

        let mut candidate_valid_prefix = 0usize;
        let mut candidate_coverage = None;
        if let (Some(sequence), Some(setup_offset), Some(table_offset)) = (
            spec.candidate_sequence,
            spec.candidate_setup_offset,
            spec.candidate_table_offset,
        ) {
            let record =
                candidate_record.ok_or_else(|| error("PUYO1 CHR candidate record 누락"))?;
            let raw = candidate_raw.ok_or_else(|| error("PUYO1 CHR candidate 원본 누락"))?;
            if record.offset != setup_offset || record.resource != spec.glyph_resource {
                return Err(error(format!(
                    "PUYO1 CHR candidate setup 불일치: {sequence}:+0x{:04X}",
                    record.offset
                )));
            }
            let layout = layouts
                .rows
                .iter()
                .find(|layout| layout.archive == spec.seq_archive && layout.sequence == sequence)
                .ok_or_else(|| error("PUYO1 CHR candidate layout 누락"))?;
            let actual_table_offset = layout
                .footer
                .and_then(|footer| footer.runtime_map())
                .and_then(|runtime_map| runtime_map.file_offset(record.pointer))
                .ok_or_else(|| error("PUYO1 CHR candidate pointer가 overlay 밖임"))?;
            if actual_table_offset != table_offset {
                return Err(error("PUYO1 CHR candidate table offset 불일치"));
            }
            candidate_valid_prefix = verify_p1_candidate(raw, table_offset)?.0;
            let header = cell(raw, table_offset)?;
            let declared_bytes = usize::from(header.2) * usize::from(header.3) / 2;
            if declared_bytes != spec.glyph_plain_bytes {
                return Err(error("PUYO1 CHR candidate header/BTX 길이 불일치"));
            }
            candidate_coverage = Some(verify_p1_candidate_coverage(
                raw,
                table_offset,
                &used_indices,
                &glyph_plain,
            )?);
        }

        let min_cell_index = used_indices.iter().copied().min().unwrap_or(0);
        let max_cell_index = used_indices.iter().copied().max().unwrap_or(0);
        let used_indices_above_candidate = used_indices
            .iter()
            .filter(|index| **index >= candidate_valid_prefix && candidate_valid_prefix != 0)
            .count();
        let (verified_e6_cells, verdict) = if spec.scope == "PUYO2" {
            if spec.glyph_plain_bytes != 512 * 256 / 2 {
                return Err(error("PUYO2 DEMOFONT header/BTX 길이 불일치"));
            }
            (
                BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_CELLS,
                "static-cell-contract-complete-runtime-display-unverified",
            )
        } else {
            (0, "no-write-static-cell-route-unresolved")
        };
        rows.push(SubgameSeqE6GlyphRow {
            scope: spec.scope,
            exe: spec.exe,
            archive: spec.seq_archive,
            glyph_resource: spec.glyph_resource,
            glyph_asset: spec.glyph_asset,
            descriptor_runtime: spec.descriptor_base_runtime
                + u32::from(spec.glyph_resource) * DESCRIPTOR_BYTES,
            glyph_plain_bytes: spec.glyph_plain_bytes,
            glyph_plain_sha256: glyph_digest,
            physical_glyph_cells,
            e6_sequences: e6_sequences.len(),
            e6_sequence_resource_ids: e6_selection.resources,
            e6_sequence_selection_records: e6_selection.records.len(),
            e6_sequence_selected_resources: e6_selection.selected_resources.len(),
            e6_sequence_selection_sources: e6_selection.sources,
            e6_sequence_selection_flags: e6_selection.flags,
            e6_sequence_selection_sites: e6_selection
                .records
                .iter()
                .map(|record| format!("{}:+0x{:04X}", record.sequence, record.offset))
                .collect(),
            e6_sequence_vdd_entries: e6_storage.vdd_entries,
            e6_sequence_descriptor_rows: e6_storage.descriptor_rows,
            e6_sequence_cnx_payloads: e6_storage.cnx_payloads,
            e6_sequence_overlay_rows: e6_storage.overlay_rows,
            e6_sequence_stored_bytes: e6_storage.stored_bytes,
            e6_sequence_plain_bytes: e6_storage.plain_bytes,
            e6_sequence_load_bases: e6_storage.load_bases,
            e6_sequence_storage_links: e6_storage.links,
            e6_sequence_storage_verdict: if spec.scope == "PUYO2" {
                "storage-discovery-load-target-complete-runtime-execution-unverified"
            } else {
                "storage-identity-complete-canonical-selection-absent-runtime-indirect-unresolved"
            },
            glyph_tokens,
            used_cell_indices: used_indices.len(),
            min_cell_index,
            max_cell_index,
            all_glyph_setup_records: all_glyph_setups,
            e6_glyph_setup_records: e6_glyph_setups,
            e6_table_copies: e6_glyph_setups,
            verified_e6_cells,
            verified_e6_nonempty_cells,
            e6_table_offset: spec.expected_table_offset,
            e6_table_sha256: e6_table_digests.into_iter().next(),
            non_e6_glyph_setup_records: all_glyph_setups - e6_glyph_setups,
            rejected_resource: spec.rejected_resource,
            rejected_asset: spec.rejected_asset,
            rejected_setup_records: rejected_setups,
            rejected_table_offset: spec.rejected_table_offset,
            rejected_table_sha256: rejected_table_digests.into_iter().next(),
            candidate_sequence: spec.candidate_sequence,
            candidate_setup_offset: spec.candidate_setup_offset,
            candidate_table_offset: spec.candidate_table_offset,
            candidate_valid_prefix,
            used_indices_above_candidate,
            candidate_used_valid_cells: candidate_coverage
                .as_ref()
                .map_or(0, |coverage| coverage.valid_cells),
            candidate_used_invalid_cells: candidate_coverage
                .as_ref()
                .map_or(0, |coverage| coverage.invalid_indices.len()),
            candidate_used_nonempty_cells: candidate_coverage
                .as_ref()
                .map_or(0, |coverage| coverage.nonempty_cells),
            candidate_used_empty_cells: candidate_coverage
                .as_ref()
                .map_or(0, |coverage| coverage.empty_indices.len()),
            candidate_invalid_used_indices: candidate_coverage
                .as_ref()
                .map_or_else(Vec::new, |coverage| coverage.invalid_indices.clone()),
            candidate_empty_used_indices: candidate_coverage
                .as_ref()
                .map_or_else(Vec::new, |coverage| coverage.empty_indices.clone()),
            paired_cell_count_writers: p1_static_route
                .as_ref()
                .map_or(0, |route| route.paired_writers),
            literal_halfword_20_stores: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| route.literal_halfword_20_stores.clone()),
            descriptor_literal_halfword_20_stores: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_literal_halfword_20_stores.clone()
                }),
            gpu_primitive_halfword_20_stores: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.gpu_primitive_halfword_20_stores.clone()
                }),
            cache_record_halfword_20_stores: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.cache_record_halfword_20_stores.clone()
                }),
            unclassified_literal_halfword_20_stores: 0,
            literal_halfword_20_verdict: if p1_static_route.is_some() {
                "literal-sh-20-complete-computed-alias-writes-unresolved"
            } else {
                "not-audited-puyo1-only"
            },
            literal_resource_006d_loads: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| route.literal_resource_006d_loads.clone()),
            direct_chr_object_resource_stores: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.direct_chr_object_resource_stores.clone()
                }),
            fixed_cell_chr_object_routes: p1_static_route
                .as_ref()
                .map_or(0, |route| route.fixed_cell_chr_object_routes),
            variable_cell_chr_object_routes: p1_static_route
                .as_ref()
                .map_or(0, |route| route.variable_cell_chr_object_routes),
            non_object_literal_006d_loads: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.non_object_literal_006d_loads.clone()
                }),
            literal_resource_006d_verdict: if p1_static_route.is_some() {
                "direct-literal-complete-computed-and-table-sources-unresolved"
            } else {
                "not-audited-puyo1-only"
            },
            allocator_direct_calls: p1_static_route
                .as_ref()
                .map_or(0, |route| route.allocator_direct_calls),
            allocator_immediate_type_calls: p1_static_route
                .as_ref()
                .map_or(0, |route| route.allocator_immediate_type_calls),
            allocator_dynamic_type_calls: p1_static_route
                .as_ref()
                .map_or(0, |route| route.allocator_dynamic_type_calls),
            immediate_type9_calls: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| route.immediate_type9_calls.clone()),
            direct_chr_object_allocator_calls: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.direct_chr_object_allocator_calls.clone()
                }),
            inactive_type9_calls: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| route.inactive_type9_calls.clone()),
            immediate_type9_verdict: if p1_static_route.is_some() {
                "immediate-type9-complete-dynamic-type-calls-unresolved"
            } else {
                "not-audited-puyo1-only"
            },
            descriptor_root_literal_combines: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_root_literal_combines.clone()
                }),
            descriptor_root_resource_x36_selectors: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_root_resource_x36_selectors.clone()
                }),
            descriptor_root_alternate_branch_combines: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_root_alternate_branch_combines.clone()
                }),
            descriptor_root_registration_stores: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_root_registration_stores.clone()
                }),
            unclassified_descriptor_root_combines: 0,
            chr_descriptor_direct_address_loads: p1_static_route
                .as_ref()
                .map_or(0, |route| route.chr_descriptor_direct_address_loads),
            chr_descriptor_raw_pointer_words: p1_static_route
                .as_ref()
                .map_or(0, |route| route.chr_descriptor_raw_pointer_words),
            descriptor_table_records: p1_static_route
                .as_ref()
                .map_or(0, |route| route.descriptor_table_records),
            descriptor_table_sha256: p1_static_route
                .as_ref()
                .map(|route| route.descriptor_table_sha256.clone()),
            descriptor_table_address_loads: p1_static_route.as_ref().map_or_else(
                Vec::new,
                |route| route.descriptor_table_address_loads.clone(),
            ),
            descriptor_table_preadjusted_address_loads: p1_static_route
                .as_ref()
                .map_or(0, |route| route.descriptor_table_preadjusted_address_loads),
            descriptor_table_raw_pointer_locations: p1_static_route.as_ref().map_or_else(
                Vec::new,
                |route| route.descriptor_table_raw_pointer_locations.clone(),
            ),
            descriptor_table_preadjusted_raw_pointers: p1_static_route
                .as_ref()
                .map_or(0, |route| route.descriptor_table_preadjusted_raw_pointers),
            descriptor_root_census_verdict: if p1_static_route.is_some() {
                "literal-root-and-descriptor-span-complete-resource-x36-selectors"
            } else {
                "not-audited-puyo1-only"
            },
            descriptor_selector_direct_callee_calls: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_selector_direct_callee_calls.clone()
                }),
            descriptor_selector_local_consumers: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_selector_local_consumers.clone()
                }),
            descriptor_selector_unique_callees: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_selector_unique_callees.clone()
                }),
            descriptor_forwarding_calls: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| route.descriptor_forwarding_calls.clone()),
            descriptor_forwarding_unique_callees: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_forwarding_unique_callees.clone()
                }),
            descriptor_synchronous_field_reads: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_synchronous_field_reads.clone()
                }),
            descriptor_synchronous_field_writes: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_synchronous_field_writes.clone()
                }),
            descriptor_pointer_escape_stores: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_pointer_escape_stores.clone()
                }),
            descriptor_cache_link_stores: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| route.descriptor_cache_link_stores.clone()),
            descriptor_fixed_cache_callbacks: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_fixed_cache_callbacks.clone()
                }),
            descriptor_fixed_callback_writes: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_fixed_callback_writes.clone()
                }),
            descriptor_fixed_callback_cell_reads: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_fixed_callback_cell_reads.clone()
                }),
            descriptor_fixed_callback_cell_writes: 0,
            descriptor_async_dispatch_targets: p1_static_route
                .as_ref()
                .map_or(0, |route| route.descriptor_async_dispatch_targets),
            descriptor_async_dispatch_incoming_uses: p1_static_route
                .as_ref()
                .map_or(0, |route| route.descriptor_async_dispatch_incoming_uses),
            descriptor_singleton_callback_writes: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_singleton_callback_writes.clone()
                }),
            descriptor_cache_callbacks: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| route.descriptor_cache_callbacks.clone()),
            descriptor_callback_copy_loops: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_callback_copy_loops.clone()
                }),
            descriptor_completion_callback_field_writes: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_completion_callback_field_writes.clone()
                }),
            descriptor_completion_descriptor_writes: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_completion_descriptor_writes.clone()
                }),
            descriptor_completion_callback_cell_reads: 0,
            descriptor_completion_callback_cell_writes: 0,
            descriptor_type_cache_root_combines: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_type_cache_root_combines.clone()
                }),
            descriptor_singleton_snapshot_root_combines: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_singleton_snapshot_root_combines.clone()
                }),
            descriptor_type_snapshot_root_combines: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_type_snapshot_root_combines.clone()
                }),
            descriptor_payload_copy_loops: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_payload_copy_loops.clone()
                }),
            descriptor_snapshot_field_writes: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_snapshot_field_writes.clone()
                }),
            descriptor_type_snapshot_link_stores: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| {
                    route.descriptor_type_snapshot_link_stores.clone()
                }),
            descriptor_snapshot_raw_pointer_words: p1_static_route
                .as_ref()
                .map_or(0, |route| route.descriptor_snapshot_raw_pointer_words),
            descriptor_alias_census_verdict: if p1_static_route.is_some() {
                "selector-forwarding-field-access-cache-callback-and-36-byte-clone-lineage-complete-no-additional-cell-write-other-computed-aliases-unresolved"
            } else {
                "not-audited-puyo1-only"
            },
            setup_source_cell_writer_runtime: p1_static_route
                .as_ref()
                .map(|route| route.setup_source_cell_writer_runtime),
            cache_mirror_cell_writer_runtime: p1_static_route
                .as_ref()
                .map(|route| route.cache_mirror_cell_writer_runtime),
            static_initializer_runtime: p1_static_route
                .as_ref()
                .map(|_| P1_STATIC_INITIALIZER_RUNTIME),
            static_initializer_cell_writer_runtime: p1_static_route
                .as_ref()
                .map(|route| route.initializer_cell_writer_runtime),
            static_initializer_direct_call_sites: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| route.direct_call_sites.clone()),
            static_initializer_entry_reached_call_sites: p1_static_route
                .as_ref()
                .map_or_else(Vec::new, |route| route.entry_reached_call_sites.clone()),
            static_initializer_data_refs: p1_static_route
                .as_ref()
                .map_or(0, |route| route.data_refs),
            static_initializer_raw_pointer_words: p1_static_route
                .as_ref()
                .map_or(0, |route| route.raw_pointer_words),
            static_initializer_resource: p1_static_route
                .as_ref()
                .map(|_| P1_STATIC_INITIALIZER_RESOURCE),
            static_initializer_descriptor_runtime: p1_static_route
                .as_ref()
                .map(|route| route.descriptor_runtime),
            static_initializer_registered_assets: p1_static_route
                .as_ref()
                .map_or(0, |route| route.registered_assets),
            static_initializer_cell_table_runtime: p1_static_route
                .as_ref()
                .map(|_| P1_STATIC_INITIALIZER_TABLE_RUNTIME),
            static_initializer_cell_count: p1_static_route.as_ref().map(|route| route.cell_count),
            static_initializer_cell_width: p1_static_route.as_ref().map(|route| route.cell_width),
            static_initializer_cell_height: p1_static_route.as_ref().map(|route| route.cell_height),
            alternate_static_route_verdict: if p1_static_route.is_some() {
                "direct-static-route-rejected-resource-00B4-count-zero-indirect-call-unresolved"
            } else {
                "not-audited-puyo1-only"
            },
            glyph_object_allocator_runtime: object_renderer_route.spec.allocator_runtime,
            glyph_object_allocator_type: object_renderer_route.spec.allocator_type,
            glyph_object_callback_runtime: object_renderer_route.spec.callback_runtime,
            object_pool_init_runtime: object_renderer_route.spec.pool_init_runtime,
            object_release_runtime: object_renderer_route.spec.release_runtime,
            object_type_flags_runtime: object_renderer_route.spec.type_flags_runtime,
            object_list_runtime: object_renderer_route.spec.object_list_runtime,
            render_dispatcher_runtime: object_renderer_route.spec.render_dispatcher_runtime,
            render_dispatcher_short_call_runtime: object_renderer_route
                .spec
                .short_renderer_call_runtime,
            render_dispatcher_entry_reached: object_renderer_route.dispatcher_entry_reached,
            renderer_route_verdict:
                "created-active-state-short-renderer-route-type-enable-runtime-pending",
            static_verdict: verdict,
        });
    }
    rows.sort_by_key(|row| row.scope);
    let common = common_indices.ok_or_else(|| error("E6 glyph common index 집합 누락"))?;
    Ok(SubgameSeqE6GlyphAnalysis {
        common_used_cell_index_values: common.iter().copied().collect(),
        common_used_cell_indices: common.len(),
        common_min_cell_index: common.iter().copied().min().unwrap_or(0),
        common_max_cell_index: common.iter().copied().max().unwrap_or(0),
        p2_static_ready_rows: rows
            .iter()
            .filter(|row| row.static_verdict.starts_with("static-cell-contract"))
            .count(),
        p1_no_write_rows: rows
            .iter()
            .filter(|row| row.static_verdict.starts_with("no-write"))
            .count(),
        renderer_route_rows: rows
            .iter()
            .filter(|row| {
                row.render_dispatcher_entry_reached
                    && row
                        .renderer_route_verdict
                        .starts_with("created-active-state")
            })
            .count(),
        rows,
    })
}
