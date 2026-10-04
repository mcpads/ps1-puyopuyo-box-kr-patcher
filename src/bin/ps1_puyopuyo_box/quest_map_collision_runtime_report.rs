//! QUEST exact save-state의 활성 QMP collision plane 보고서.

use std::ffi::OsString;
use std::path::PathBuf;

use super::quest_map_collision_runtime_input::load_quest_map_collision_runtime_input;
use super::usage;

pub(super) fn report_quest_map_collision_state(
    args: impl Iterator<Item = OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, observed_image_path, state_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };

    let input =
        load_quest_map_collision_runtime_input(source_path, observed_image_path, state_path)?;
    let report = &input.report;
    let source_match = report.unique_source_match().ok_or_else(|| {
        format!(
            "QUEST active QMP 원본 후보가 유일하지 않음: dimensions={}, compatible={}",
            report.dimension_candidate_count,
            report.source_matches.len(),
        )
    })?;

    println!(
        "PASS QUEST map collision state: observed BIN SHA-256 {}, state SHA-256 {}, \
         Mednafen version 0x{:X}, active slot {}, map {}x{}, primary actor 0x{:08X}",
        input.observed_image_sha256,
        input.state_sha256,
        input.state_version,
        report.active_slot,
        report.width,
        report.height,
        report.primary_actor_runtime,
    );
    println!(
        "  context primary {}, layer {}, direction {}, state {}, input 0x{:04X}, \
         OP92 [{},{}]",
        report.classifier_context.actor_is_primary,
        report.classifier_context.actor_layer,
        report.classifier_context.actor_direction,
        report.classifier_context.actor_state,
        report.classifier_context.input_state,
        report.classifier_context.op92_conditions[0],
        report.classifier_context.op92_conditions[1],
    );
    println!(
        "  actor fixed ({:#010X},{:#010X}), local rectangle ({},{},{},{}), \
         stationary rectangle ({},{},{},{}), member flags 0x{:02X}, map collision {}, \
         tile bounds ({},{})..({},{})",
        report.actor_geometry.position_fixed[0] as u32,
        report.actor_geometry.position_fixed[1] as u32,
        report.actor_geometry.local_rectangle[0],
        report.actor_geometry.local_rectangle[1],
        report.actor_geometry.local_rectangle[2],
        report.actor_geometry.local_rectangle[3],
        report.stationary_actor_rectangle.x,
        report.stationary_actor_rectangle.y,
        report.stationary_actor_rectangle.horizontal_extent,
        report.stationary_actor_rectangle.vertical_extent,
        report.actor_member_flags,
        report.actor_map_collision_enabled,
        report.tile_bounds.min_x,
        report.tile_bounds.min_y,
        report.tile_bounds.max_x_exclusive,
        report.tile_bounds.max_y_exclusive,
    );
    println!(
        "  classes 0/1/2 = {}/{}/{}, published tail nonzero {}, dimension candidates {}, \
         unique source resource {} {}, dynamic zero-clearing {} cells",
        report.class_counts[0],
        report.class_counts[1],
        report.class_counts[2],
        report.live_tail_nonzero_cells,
        report.dimension_candidate_count,
        source_match.resource_id,
        source_match.name,
        source_match.zero_cleared_cells.len(),
    );
    for cell in &source_match.zero_cleared_cells {
        println!(
            "    ({},{}) index {}: source 0x{:04X} -> runtime 0x{:04X}",
            cell.x, cell.y, cell.index, cell.source_tile, cell.runtime_tile,
        );
    }
    let stationary_overlap_count = report
        .stationary_probes
        .iter()
        .filter(|probe| probe.overlap.is_some())
        .count();
    let stationary_collision_count = report
        .stationary_probes
        .iter()
        .filter(|probe| probe.overlap.is_some() && probe.class.code() != 0)
        .count();
    println!(
        "  zero-delta probes {}, in-bounds {}, overlaps {}, collision overlaps {}",
        report.stationary_probes.len(),
        report
            .stationary_probes
            .iter()
            .filter(|probe| probe.in_bounds)
            .count(),
        stationary_overlap_count,
        stationary_collision_count,
    );
    for probe in report
        .stationary_probes
        .iter()
        .filter(|probe| probe.overlap.is_some())
    {
        let overlap = probe.overlap.expect("overlap으로 거른 probe");
        println!(
            "    tile ({},{}) in-bounds {} value 0x{:04X} class {}, overlap ({},{}) {}x{}px",
            probe.tile_x,
            probe.tile_y,
            probe.in_bounds,
            probe.tile,
            probe.class.code(),
            overlap.x,
            overlap.y,
            overlap.width_pixels,
            overlap.height_pixels,
        );
    }
    println!(
        "  source 식별은 이 state의 active plane과 zero-clearing 양립성만 뜻하며, \
         zero-delta geometry는 exact movement delta·movement adjust 결과가 아님. \
         관측 BIN-state 결속·자연 route·통행 가능성·사람 승인을 대신하지 않음"
    );
    Ok(())
}
