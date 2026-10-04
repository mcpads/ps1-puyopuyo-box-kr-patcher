//! 지원 원본의 EXE 분석 결과와 script·asset model을 결속하는 정적 분석.
//!
//! [`crate::disasm`]은 기계어 해석과 일반 제어 흐름을, [`crate::script`]는
//! 저장 형식·token·원장을 소유한다. 이 module은 두 경계의 typed 결과를
//! 함께 소비하되 둘 중 어느 곳에서도 역참조하지 않는다.

pub mod quest_battle_name_table;
pub mod quest_item_acquisition_records;
pub mod quest_item_acquisition_text_units;
pub mod quest_resident_encounter_path;
pub mod quest_resident_encounter_text;
pub mod quest_resident_escape_text;
pub mod quest_resident_seq_child_graph;
pub mod quest_resident_seq_frontiers;
pub mod quest_resident_seq_paths;
pub mod quest_resident_seq_spawn_path;
pub mod quest_resident_seq_transition;
pub mod quest_resident_text_units;
pub mod quest_seq_flag_branches;
pub mod quest_seq_resource_transitions;
pub mod quest_seq_text_frontiers;
pub mod quest_seq_text_paths;
pub mod quest_ui_literal_units;
pub mod subgame_seq_overlay_call_arguments;
pub mod subgame_seq_scene_glyph;
pub mod subgame_seq_scene_messages;

pub use quest_battle_name_table::*;
pub use quest_item_acquisition_records::*;
pub use quest_item_acquisition_text_units::*;
pub use quest_resident_encounter_path::*;
pub use quest_resident_encounter_text::*;
pub use quest_resident_escape_text::*;
pub use quest_resident_seq_child_graph::*;
pub use quest_resident_seq_frontiers::*;
pub use quest_resident_seq_paths::*;
pub use quest_resident_seq_spawn_path::*;
pub use quest_resident_seq_transition::*;
pub use quest_resident_text_units::*;
pub use quest_seq_flag_branches::*;
pub use quest_seq_resource_transitions::*;
pub use quest_seq_text_frontiers::*;
pub use quest_seq_text_paths::*;
pub use quest_ui_literal_units::*;
pub use subgame_seq_overlay_call_arguments::*;
pub use subgame_seq_scene_glyph::*;
pub use subgame_seq_scene_messages::*;
