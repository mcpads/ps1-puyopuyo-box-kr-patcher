//! PlayStation Mode2/2352 디스크 경계.

pub mod image;
pub mod iso9660;
pub mod psx_exe;
pub mod relocation;
pub mod reserved;
pub mod sector;
pub mod writer;

pub use image::{DiscImage, ImageError, SectorCounts, VerifiedSourceImage};
pub use iso9660::{DirectoryRecordLocation, Extent, Iso9660, IsoEntry, IsoError, VolumeTail};
pub use psx_exe::{PsxExe, PsxExeError};
pub use relocation::{
    add_single_extent_data_file_relocation, add_single_extent_record_update, allocate_volume_tail,
    partition_extent_with_remainder, ExtentPartition, IsoRelocationError, VolumeTailAllocation,
    VolumeTailLayout,
};
pub use reserved::{verify_box_32m_reservation, Box32mError, Box32mSummary, BOX_32M_PATH};
pub use writer::{apply_form1_write_plan, DiscWriteError};
