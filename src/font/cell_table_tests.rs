use super::*;

fn write_u16(ram: &mut [u8], offset: usize, value: u16) {
    ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn fixture() -> Vec<u8> {
    let mut ram = vec![0; 0x400];
    ram[0x100..0x104].copy_from_slice(&0x0001_0001u32.to_le_bytes());
    ram[0x11C..0x120].copy_from_slice(&0x8000_0200u32.to_le_bytes());
    for (offset, value) in [(0, 0), (2, 0), (4, 32), (6, 16), (8, 0), (10, 0)] {
        write_u16(&mut ram, 0x200 + offset, value);
    }
    for (index, x) in [0u16, 8, 16].into_iter().enumerate() {
        let offset = 0x20C + index * 12;
        write_u16(&mut ram, offset, x);
        write_u16(&mut ram, offset + 2, 0);
        write_u16(&mut ram, offset + 4, 8);
        write_u16(&mut ram, offset + 6, 16);
    }
    ram
}

#[test]
fn kseg_descriptor와_cell분모를_명시해_읽는다() {
    let table = parse_runtime_cell_table(&fixture(), 0x8000_0100, 3).unwrap();
    assert_eq!(table.table_runtime, 0x8000_0200);
    assert_eq!((table.atlas_width, table.atlas_height), (32, 16));
    assert_eq!(table.cells.len(), 3);
    assert_eq!(table.cells[2].x, 16);
    assert_eq!(table.distinct_sizes(), 1);
    assert_eq!(table.large_cells(), 0);
}

#[test]
fn 분모의_빈cell과_atlas이탈을_거부한다() {
    let mut ram = fixture();
    write_u16(&mut ram, 0x20C + 12 + 4, 0);
    assert_eq!(
        parse_runtime_cell_table(&ram, 0x8000_0100, 3),
        Err(RuntimeCellTableError::EmptyCell(1))
    );

    let mut ram = fixture();
    write_u16(&mut ram, 0x20C + 24, 30);
    assert!(matches!(
        parse_runtime_cell_table(&ram, 0x8000_0100, 3),
        Err(RuntimeCellTableError::CellOutsideAtlas { index: 2, .. })
    ));
}

#[test]
fn tsv는_signed_anchor와_주소를_보존한다() {
    let mut ram = fixture();
    write_u16(&mut ram, 0x20C + 10, (-3i16) as u16);
    let table = parse_runtime_cell_table(&ram, 0x8000_0100, 3).unwrap();
    let tsv = render_runtime_cell_table_tsv(&table);
    assert!(tsv.starts_with("descriptor_runtime\ttable_runtime"));
    assert!(tsv.contains("0x80000100\t0x80000200\t32\t16\t0\t0\t0\t8\t16\t0\t-3"));
}

#[test]
fn 통합_sprite_표는_명시한_cell만_검증한다() {
    let mut ram = fixture();
    write_u16(&mut ram, 0x20C + 12 + 4, 0);
    write_u16(&mut ram, 0x20C + 24, 40);

    let table = parse_runtime_cell_selection(&ram, 0x8000_0100, 3, &[0]).unwrap();
    assert_eq!(
        table
            .cells
            .iter()
            .map(|cell| cell.index)
            .collect::<Vec<_>>(),
        [0]
    );

    assert_eq!(
        parse_runtime_cell_selection(&ram, 0x8000_0100, 3, &[]),
        Err(RuntimeCellTableError::EmptySelection)
    );
    assert_eq!(
        parse_runtime_cell_selection(&ram, 0x8000_0100, 3, &[0, 0]),
        Err(RuntimeCellTableError::DuplicateCellIndex(0))
    );
    assert_eq!(
        parse_runtime_cell_selection(&ram, 0x8000_0100, 3, &[3]),
        Err(RuntimeCellTableError::CellIndexOutsideTable {
            index: 3,
            record_count: 3,
        })
    );
}
