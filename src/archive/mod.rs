//! 게임 컨테이너 경계.

pub mod sound_inventory;
pub mod vab;
pub mod vdd;

pub use sound_inventory::{
    audit_sound_archives, verify_box_sound_inventory, SoundInventoryError, SoundInventorySummary,
    BOX_EXPECTED_SOUND_INVENTORY, BOX_SOUND_ARCHIVE_PATHS,
};
pub use vab::{VabBank, VabError, VabProgram, VabSample, VabTone};
pub use vdd::{VddArchive, VddEntry, VddError};
