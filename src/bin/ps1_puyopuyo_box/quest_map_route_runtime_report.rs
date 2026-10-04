//! QUEST exact state의 live collision plane에서 waypoint 후보를 만든다.

use std::ffi::OsString;
use std::fmt::Write;
use std::path::PathBuf;

use ps1_puyopuyo_box::runtime::find_direction_robust_quest_map_route;

use super::quest_map_collision_runtime_input::{
    load_quest_map_collision_runtime_input, parse_quest_pixel,
};
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn write_quest_map_route_candidate(
    args: impl Iterator<Item = OsString>,
) -> Result<(), String> {
    let values = args.collect::<Vec<_>>();
    let [source_raw, observed_image_raw, state_raw, goal_x_raw, goal_y_raw, output_raw] =
        values.as_slice()
    else {
        return Err(usage().to_string());
    };
    let source_path = PathBuf::from(source_raw);
    let observed_image_path = PathBuf::from(observed_image_raw);
    let state_path = PathBuf::from(state_raw);
    let goal_pixel = [
        parse_quest_pixel(goal_x_raw, "목표 x")?,
        parse_quest_pixel(goal_y_raw, "목표 y")?,
    ];
    let output_path = PathBuf::from(output_raw);

    let input =
        load_quest_map_collision_runtime_input(&source_path, &observed_image_path, &state_path)?;
    let source_match = input.report.unique_source_match().ok_or_else(|| {
        format!(
            "QUEST active QMP 원본 후보가 유일하지 않음: dimensions={}, compatible={}",
            input.report.dimension_candidate_count,
            input.report.source_matches.len(),
        )
    })?;
    let route = find_direction_robust_quest_map_route(&input.report, goal_pixel)
        .map_err(|error| error.to_string())?;

    let mut rendered = String::from(
        "observed_image_sha256\tstate_sha256\tmednafen_state_version\tactive_slot\tqmp_resource_id\tqmp_name\twaypoint_index\tx_pixel\ty_pixel\tdelta_x\tdelta_y\tsegment_pixels\n",
    );
    for (index, waypoint) in route.waypoints.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .and_then(|previous_index| route.waypoints.get(previous_index));
        let delta = previous
            .map(|previous| [waypoint[0] - previous[0], waypoint[1] - previous[1]])
            .unwrap_or([0, 0]);
        let segment_pixels = delta[0].unsigned_abs() + delta[1].unsigned_abs();
        writeln!(
            rendered,
            "{}\t{}\t0x{:X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            input.observed_image_sha256,
            input.state_sha256,
            input.state_version,
            input.report.active_slot,
            source_match.resource_id,
            source_match.name,
            index,
            waypoint[0],
            waypoint[1],
            delta[0],
            delta[1],
            segment_pixels,
        )
        .expect("String 쓰기는 실패하지 않음");
    }
    write_report_file(
        &output_path,
        "QUEST map route candidate TSV",
        route.waypoints.len(),
        &rendered,
    )?;
    println!(
        "  route start ({},{}), subpixel fixed ({},{}), goal ({},{}), pixel steps {}, \
         waypoints {}, evaluated/clear {}/{}, direction branches {:?}",
        route.start_pixel[0],
        route.start_pixel[1],
        route.start_subpixel_fixed[0],
        route.start_subpixel_fixed[1],
        route.goal_pixel[0],
        route.goal_pixel[1],
        route.pixel_steps,
        route.waypoints.len(),
        route.evaluated_positions,
        route.clear_positions,
        route.direction_branch_representatives,
    );
    println!(
        "  이 후보는 exact state의 live plane과 zero-delta actor 사각형만 사용함. \
         관측 BIN-state 실행 결속·frame delta·movement adjust·object collision·encounter·\
         자연 실행·사람 승인을 대신하지 않음"
    );
    Ok(())
}
