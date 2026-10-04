use super::*;

fn descriptor_selection(exe: &'static str, archive: &'static str) -> BtxDescriptorSelection {
    BtxDescriptorSelection {
        exe,
        archive,
        resource_multiply_runtime: 0x8001_AEFC,
        descriptor_select_runtime: 0x8001_AF0C,
        cache_check_runtime: 0x8001_AF1C,
        descriptor_copy_runtime: 0x8001_AF30,
        descriptor_name_field_read_runtime: 0x8001_AF34,
        descriptor_copy_end_runtime: 0x8001_AF64,
        descriptor_copy_source_register: crate::isa::Reg::V1,
        working_descriptor_runtime: 0x800B_37C0,
        entry_reached: true,
    }
}

#[test]
fn descriptor는_36바이트_stride의_resource로_환산한다() {
    assert_eq!(
        descriptor_resource_id(0x8006_7E48, 0x8006_96E4).unwrap(),
        175
    );
    assert_eq!(
        descriptor_resource_id(0x8006_7E48, 0x8006_9750).unwrap(),
        178
    );
}

#[test]
fn descriptor_table_앞이나_격자_밖_주소를_거부한다() {
    assert!(descriptor_resource_id(0x8006_7E48, 0x8006_7E44).is_err());
    assert!(descriptor_resource_id(0x8006_7E48, 0x8006_7E49).is_err());
}

#[test]
fn 같은_exe와_grp의_descriptor_선택_경계만_받는다() {
    let selections = BtxDescriptorSelectionAnalysis {
        selections: vec![descriptor_selection(PUYO1_EXE, PP1_GRP_ARCHIVE)],
    };
    let selection = exact_descriptor_selection(&selections).unwrap();
    assert_eq!(selection.resource_multiply_runtime, 0x8001_AEFC);
    assert_eq!(selection.descriptor_copy_runtime, 0x8001_AF30);
}

#[test]
fn descriptor_선택_경계의_누락과_중복을_거부한다() {
    assert!(
        exact_descriptor_selection(&BtxDescriptorSelectionAnalysis { selections: vec![] }).is_err()
    );
    assert!(exact_descriptor_selection(&BtxDescriptorSelectionAnalysis {
        selections: vec![
            descriptor_selection(PUYO1_EXE, PP1_GRP_ARCHIVE),
            descriptor_selection(PUYO1_EXE, PP1_GRP_ARCHIVE),
        ],
    })
    .is_err());
    assert!(exact_descriptor_selection(&BtxDescriptorSelectionAnalysis {
        selections: vec![descriptor_selection(PUYO1_EXE, "/PUYO1/OTHER_GRP.VDD;1")],
    })
    .is_err());
}
