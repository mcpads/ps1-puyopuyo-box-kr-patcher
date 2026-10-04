use super::*;
use crate::pipeline::btx_pending_static_cells::{
    BtxPendingStaticCellPrefix, BtxPendingStaticCellRecord,
};

fn record(index: usize, x: u16, y: u16, width: u16, height: u16) -> BtxPendingStaticCellRecord {
    BtxPendingStaticCellRecord {
        index,
        x,
        y,
        width,
        height,
        anchor_x: 0,
        anchor_y: 0,
        empty: false,
        outside_atlas: false,
    }
}

fn valid_prefix() -> BtxPendingStaticCellPrefix {
    BtxPendingStaticCellPrefix {
        valid_cells: vec![
            record(0, 0, 0, 64, 48),
            record(1, 0, 48, 64, 48),
            record(2, 0, 144, 80, 48),
            record(3, 64, 0, 64, 48),
            record(4, 64, 48, 64, 48),
            record(5, 0, 96, 64, 48),
        ],
        boundary_record: BtxPendingStaticCellRecord {
            index: 6,
            x: 0,
            y: 0,
            width: 0,
            height: 0x8000,
            anchor_x: -32768,
            anchor_y: -32736,
            empty: true,
            outside_atlas: false,
        },
        protected_tail_bytes: 508,
    }
}

#[test]
fn 여섯_cell과_non_cell_boundary를_함께_요구한다() {
    assert_eq!(verify_pp2_prefix(&valid_prefix()), Ok(()));

    let mut drift = valid_prefix();
    drift.protected_tail_bytes -= 1;
    assert!(verify_pp2_prefix(&drift).is_err());

    let mut drift = valid_prefix();
    drift.valid_cells[2].width -= 1;
    assert!(verify_pp2_prefix(&drift).is_err());
}
