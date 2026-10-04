use std::collections::BTreeSet;

use super::*;

fn summary(empty_slots: Vec<usize>) -> DemoFontSummary {
    DemoFontSummary {
        slots: 512,
        nonempty_slots: 512 - empty_slots.len(),
        empty_slots,
        unique_cell_pixels: 0,
        palette_indices: BTreeSet::new(),
    }
}

#[test]
fn resource_index를_descriptor주소로_환산한다() {
    assert_eq!(
        DEMO_FONT_RESOURCE_TABLE_RUNTIME
            + u32::from(DEMO_FONT_RESOURCE_INDEX) * RESOURCE_RECORD_BYTES,
        DEMO_FONT_DESCRIPTOR_RUNTIME
    );
}

#[test]
fn 코드표는_중복을_허용하되_각_pair가_한_문자여야_한다() {
    let mut table = Vec::with_capacity(CODE_TABLE_BYTES);
    for _ in 0..DEMO_FONT_CODE_TABLE_ENTRIES {
        table.extend_from_slice(&[0x82, 0xA0]);
    }
    assert_eq!(verify_code_table_shape(&table), Ok(()));

    table[CODE_TABLE_BYTES - 1] = 0;
    assert_eq!(
        verify_code_table_shape(&table),
        Err(DemoFontConsumerError::InvalidCodeTableEntry(
            DEMO_FONT_CODE_TABLE_ENTRIES - 1
        ))
    );
}

#[test]
fn 등록범위_안팎의_빈_cell을_구분한다() {
    assert_eq!(verify_physical_cells(&summary(vec![206, 398, 511])), Ok(1));
    assert_eq!(
        verify_physical_cells(&DemoFontSummary {
            slots: 511,
            ..summary(vec![])
        }),
        Err(DemoFontConsumerError::PhysicalSlotCount {
            expected: 512,
            actual: 511
        })
    );
}
