use super::*;

fn cell(index: usize) -> BtxPendingStaticCellRecord {
    BtxPendingStaticCellRecord {
        index,
        x: index as u16 * 8,
        y: 0,
        width: 8,
        height: 8,
        anchor_x: 0,
        anchor_y: 0,
        empty: false,
        outside_atlas: false,
    }
}

fn setup() -> BtxPendingStaticSetup {
    BtxPendingStaticSetup {
        sequence: "TEST.EXE".to_owned(),
        record_offset: 0x10,
        record_runtime: 0x8001_0010,
        table_offset: 0x40,
        table_runtime: 0x8001_0040,
        record_cell_count: 0,
        atlas_width: 32,
        atlas_height: 16,
        review_layout_matches: false,
        descriptor_plain_layout_matches: false,
        next_referenced_table_offset: None,
        next_referenced_overlay_offset: None,
        next_overlay_cell_span: None,
        inferred_cell_records: None,
        table_padding_bytes: None,
        valid_cells: None,
        empty_cells: None,
        outside_cells: None,
        inferred_cells: Vec::new(),
        leading_cell_prefix: None,
    }
}

#[test]
fn 온전한_table_extent는_다음_table경계의_cell을_사용한다() {
    let mut setup = setup();
    setup.inferred_cells = vec![cell(0), cell(1)];
    setup.valid_cells = Some(2);
    setup.empty_cells = Some(0);
    setup.outside_cells = Some(0);

    let extent = static_setup_extent(setup).unwrap();

    assert_eq!(
        extent.extent_basis,
        BtxFamilyStaticCellExtentBasis::NextReferencedTable
    );
    assert_eq!(extent.cells.len(), 2);
}

#[test]
fn full_span이_오염되면_검증된_선두_prefix만_사용한다() {
    let mut setup = setup();
    setup.inferred_cells = vec![cell(0), cell(1), cell(2)];
    setup.valid_cells = Some(2);
    setup.empty_cells = Some(0);
    setup.outside_cells = Some(1);
    setup.leading_cell_prefix = Some(
        super::super::btx_pending_static_cells::BtxPendingStaticCellPrefix {
            valid_cells: vec![cell(0), cell(1)],
            boundary_record: BtxPendingStaticCellRecord {
                empty: true,
                ..cell(2)
            },
            protected_tail_bytes: 16,
        },
    );

    let extent = static_setup_extent(setup).unwrap();

    assert_eq!(
        extent.extent_basis,
        BtxFamilyStaticCellExtentBasis::LeadingValidPrefix
    );
    assert_eq!(extent.cells.len(), 2);
}

#[test]
fn 닫히지_않거나_invalid인_extent는_부분_cell을_반환하지_않는다() {
    assert!(static_setup_extent(setup()).is_err());

    let mut setup = setup();
    setup.next_overlay_cell_span = Some(
        super::super::btx_pending_static_cells::BtxPendingStaticCellSpan {
            records: 1,
            padding_bytes: 0,
            valid_cells: 0,
            empty_cells: 0,
            outside_cells: 1,
            cells: vec![BtxPendingStaticCellRecord {
                outside_atlas: true,
                ..cell(0)
            }],
        },
    );
    assert!(static_setup_extent(setup).is_err());
}

#[test]
fn 물리_주소가_달라도_cell_geometry만_같으면_동일하다() {
    let left = BtxFamilyStaticCellSetup {
        setup_runtime: 0x8001_0010,
        table_runtime: 0x8001_0040,
        coordinate_width: 32,
        coordinate_height: 16,
        extent_basis: BtxFamilyStaticCellExtentBasis::NextReferencedTable,
        cells: vec![cell(0), cell(1)],
    };
    let mut right = left.clone();
    right.setup_runtime = 0x8002_0010;
    right.table_runtime = 0x8002_0040;
    assert!(same_cell_geometry(&left, &right));

    right.cells[1].width += 1;
    assert!(!same_cell_geometry(&left, &right));
}
