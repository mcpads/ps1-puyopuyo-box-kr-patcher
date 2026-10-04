//! QUEST exact state의 OP8E item container 접촉 경로를 파일로 남긴다.

use std::ffi::OsString;
use std::fmt::Write;
use std::path::PathBuf;

use ps1_puyopuyo_box::disasm::{
    verify_box_quest_item_container, verify_box_quest_player_interaction_contact,
};
use ps1_puyopuyo_box::runtime::{inspect_quest_item_container_contact_state, MednafenPsxState};

use super::quest_map_collision_runtime_input::{
    load_quest_map_collision_runtime_input, parse_quest_pixel,
};
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn write_quest_item_container_contact_candidate(
    args: impl Iterator<Item = OsString>,
) -> Result<(), String> {
    let values = args.collect::<Vec<_>>();
    let [source_raw, observed_image_raw, state_raw, target_x_raw, target_y_raw, output_raw] =
        values.as_slice()
    else {
        return Err(usage().to_string());
    };
    let source_path = PathBuf::from(source_raw);
    let observed_image_path = PathBuf::from(observed_image_raw);
    let state_path = PathBuf::from(state_raw);
    let target_anchor_pixel = [
        parse_quest_pixel(target_x_raw, "item container anchor x")?,
        parse_quest_pixel(target_y_raw, "item container anchor y")?,
    ];
    let output_path = PathBuf::from(output_raw);

    let input =
        load_quest_map_collision_runtime_input(&source_path, &observed_image_path, &state_path)?;
    verify_box_quest_player_interaction_contact(input.quest_map, &input.quest_text)
        .map_err(|error| error.to_string())?;
    verify_box_quest_item_container(input.quest_map, &input.quest_text)
        .map_err(|error| error.to_string())?;
    let source_match = input.report.unique_source_match().ok_or_else(|| {
        format!(
            "QUEST active QMP 원본 후보가 유일하지 않음: dimensions={}, compatible={}",
            input.report.dimension_candidate_count,
            input.report.source_matches.len(),
        )
    })?;
    let state = MednafenPsxState::parse(&input.state_raw).map_err(|error| error.to_string())?;
    let report =
        inspect_quest_item_container_contact_state(&state, &input.report, target_anchor_pixel)
            .map_err(|error| error.to_string())?;

    let columns = [
        "source_image_sha1",
        "source_image_sha256",
        "observed_image_sha256",
        "state_sha256",
        "mednafen_state_version",
        "active_slot",
        "qmp_resource_id",
        "qmp_name",
        "primary_actor_runtime",
        "player_callback_runtime",
        "player_layer",
        "container_object_count",
        "target_requested_x_pixel",
        "target_requested_y_pixel",
        "target_pool_index",
        "target_object_runtime",
        "target_callback_runtime",
        "target_layer",
        "target_collision_member_flags",
        "target_contact_flags",
        "target_state",
        "target_persistence_selector",
        "target_parameter",
        "target_item_id",
        "target_anchor_x_pixel",
        "target_anchor_y_pixel",
        "target_rect_x",
        "target_rect_y",
        "target_rect_horizontal_extent",
        "target_rect_vertical_extent",
        "interaction_input_runtime",
        "interaction_input_mask",
        "interaction_direction",
        "interaction_contact_mask",
        "centered_contact_anchor_x_pixel",
        "centered_contact_anchor_y_pixel",
        "contact_anchor_candidate_count",
        "contact_anchor_x_pixel",
        "contact_anchor_y_pixel",
        "player_rect_x",
        "player_rect_y",
        "player_rect_horizontal_extent",
        "player_rect_vertical_extent",
        "probe_rect_x",
        "probe_rect_y",
        "probe_rect_horizontal_extent",
        "probe_rect_vertical_extent",
        "overlap_x",
        "overlap_y",
        "overlap_width_pixels",
        "overlap_height_pixels",
        "route_start_x_pixel",
        "route_start_y_pixel",
        "route_start_subpixel_fixed_x",
        "route_start_subpixel_fixed_y",
        "route_goal_x_pixel",
        "route_goal_y_pixel",
        "route_normal_movement_state",
        "route_movement_input_runtime",
        "route_direction_input_masks",
        "route_direction_deltas_fixed",
        "route_position_step_pixels",
        "route_movement_steps",
        "route_pixel_steps",
        "route_waypoint_count",
        "route_evaluated_positions",
        "route_clear_positions",
        "waypoint_index",
        "waypoint_x_pixel",
        "waypoint_y_pixel",
        "delta_x",
        "delta_y",
        "segment_pixels",
    ];
    let mut rendered = format!("{}\n", columns.join("\t"));
    let route = &report.route.route;
    for (index, waypoint) in route.waypoints.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .and_then(|previous_index| route.waypoints.get(previous_index));
        let delta = previous
            .map(|previous| [waypoint[0] - previous[0], waypoint[1] - previous[1]])
            .unwrap_or([0, 0]);
        let segment_pixels = delta[0].unsigned_abs() + delta[1].unsigned_abs();
        let fields = vec![
            input.source_image_sha1.to_string(),
            input.source_image_sha256.clone(),
            input.observed_image_sha256.clone(),
            input.state_sha256.clone(),
            format!("0x{:X}", input.state_version),
            input.report.active_slot.to_string(),
            source_match.resource_id.to_string(),
            source_match.name.clone(),
            format!("0x{:08X}", report.primary_actor_runtime),
            format!("0x{:08X}", report.player_callback_runtime),
            report.player_layer.to_string(),
            report.container_object_count.to_string(),
            report.target_anchor_pixel[0].to_string(),
            report.target_anchor_pixel[1].to_string(),
            report.target.pool_index.to_string(),
            format!("0x{:08X}", report.target.object_runtime),
            format!("0x{:08X}", report.target.callback_runtime),
            report.target.layer.to_string(),
            format!("0x{:02X}", report.target.collision_member_flags),
            format!("0x{:04X}", report.target.contact_flags),
            report.target.state.to_string(),
            format!("0x{:02X}", report.target.persistence_selector),
            report.target.parameter.to_string(),
            report.target.item_id.to_string(),
            report.target_anchor_pixel[0].to_string(),
            report.target_anchor_pixel[1].to_string(),
            report.target.world_rectangle.x.to_string(),
            report.target.world_rectangle.y.to_string(),
            report.target.world_rectangle.horizontal_extent.to_string(),
            report.target.world_rectangle.vertical_extent.to_string(),
            format!("0x{:08X}", report.interaction_input_runtime),
            format!("0x{:04X}", report.interaction_input_mask),
            report.interaction_direction.to_string(),
            format!("0x{:04X}", report.interaction_contact_mask),
            report.centered_contact_anchor_pixel[0].to_string(),
            report.centered_contact_anchor_pixel[1].to_string(),
            report.contact_anchor_candidate_count.to_string(),
            report.contact_anchor_pixel[0].to_string(),
            report.contact_anchor_pixel[1].to_string(),
            report.player_rectangle_at_contact.x.to_string(),
            report.player_rectangle_at_contact.y.to_string(),
            report
                .player_rectangle_at_contact
                .horizontal_extent
                .to_string(),
            report
                .player_rectangle_at_contact
                .vertical_extent
                .to_string(),
            report.interaction_probe_at_contact.x.to_string(),
            report.interaction_probe_at_contact.y.to_string(),
            report
                .interaction_probe_at_contact
                .horizontal_extent
                .to_string(),
            report
                .interaction_probe_at_contact
                .vertical_extent
                .to_string(),
            report.interaction_overlap.x.to_string(),
            report.interaction_overlap.y.to_string(),
            report.interaction_overlap.width_pixels.to_string(),
            report.interaction_overlap.height_pixels.to_string(),
            route.start_pixel[0].to_string(),
            route.start_pixel[1].to_string(),
            route.start_subpixel_fixed[0].to_string(),
            route.start_subpixel_fixed[1].to_string(),
            route.goal_pixel[0].to_string(),
            route.goal_pixel[1].to_string(),
            report.route.normal_movement_state.to_string(),
            format!("0x{:08X}", report.route.input_runtime),
            format!("{:04X?}", report.route.direction_input_masks),
            format!("{:?}", report.route.direction_deltas_fixed),
            report.route.position_step_pixels.to_string(),
            report.route.movement_steps.to_string(),
            route.pixel_steps.to_string(),
            route.waypoints.len().to_string(),
            route.evaluated_positions.to_string(),
            route.clear_positions.to_string(),
            index.to_string(),
            waypoint[0].to_string(),
            waypoint[1].to_string(),
            delta[0].to_string(),
            delta[1].to_string(),
            segment_pixels.to_string(),
        ];
        writeln!(rendered, "{}", fields.join("\t")).expect("String 쓰기는 실패하지 않음");
    }
    write_report_file(
        &output_path,
        "QUEST item container contact candidate TSV",
        route.waypoints.len(),
        &rendered,
    )?;
    println!(
        "PASS QUEST OP8E item container contact candidate: target slot {}/runtime 0x{:08X}, anchor ({},{}), centered/contact ({},{})/({},{}), candidates {}, direction/input {}/0x{:04X}, walk route {} px/{} inputs/{} waypoints",
        report.target.pool_index,
        report.target.object_runtime,
        report.target_anchor_pixel[0],
        report.target_anchor_pixel[1],
        report.centered_contact_anchor_pixel[0],
        report.centered_contact_anchor_pixel[1],
        report.contact_anchor_pixel[0],
        report.contact_anchor_pixel[1],
        report.contact_anchor_candidate_count,
        report.interaction_direction,
        report.interaction_input_mask,
        route.pixel_steps,
        report.route.movement_steps,
        route.waypoints.len(),
    );
    println!(
        "  exact source·관측 BIN·state·active QMP·OP8E callback·접촉 geometry와 state 0의 2px delta를 결속함. 경로는 모든 live OP8E container를 피하고 map collision이 전혀 없는 전이만 쓰며, 다른 종류의 객체·적·polling timing·자연 실행·HITL을 대신하지 않음"
    );
    Ok(())
}
