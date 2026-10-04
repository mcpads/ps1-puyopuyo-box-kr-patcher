use super::*;

fn fixture_link(resource: u16, asset_name: &str) -> BtxNameLink {
    let descriptor_runtime =
        PUYO1_DESCRIPTOR_RUNTIME_START + u32::from(resource) * BTX_DESCRIPTOR_BYTES;
    BtxNameLink {
        exe: PUYO1_EXE,
        archive: PP1_GRP,
        asset_name: asset_name.to_string(),
        exe_text_offset: 0,
        runtime_addr: 0,
        static_ref_sites: Vec::new(),
        entry_reached_ref_sites: Vec::new(),
        table_runtime_start: 0,
        table_runtime_end: 0,
        table_static_ref_sites: Vec::new(),
        table_entry_reached_ref_sites: Vec::new(),
        descriptor_pointer_offset: 0,
        descriptor_pointer_runtime: descriptor_runtime + BTX_NAME_POINTER_FIELD_OFFSET,
        descriptor_runtime_start: PUYO1_DESCRIPTOR_RUNTIME_START,
        descriptor_runtime_end: PUYO1_DESCRIPTOR_RUNTIME_START + 185 * BTX_DESCRIPTOR_BYTES,
        descriptor_static_ref_sites: Vec::new(),
        descriptor_entry_reached_ref_sites: Vec::new(),
    }
}

fn fixture_links() -> Vec<BtxNameLink> {
    RUNTIME_SELECTED_ENDING_ASSETS
        .iter()
        .chain(&PROTECTED_PUYO2_ENDING_COPIES)
        .map(|expected| fixture_link(expected.resource, expected.asset_name))
        .collect()
}

#[test]
fn 관측한_puyo1_ending과_puyo2_보호_사본을_서로_다른_resource로_연결한다() {
    let asset_map = verify_puyo1_ending_asset_map(&fixture_links()).unwrap();

    assert_eq!(asset_map.descriptor_runtime_start, 0x8006_7E48);
    assert_eq!(
        asset_map
            .runtime_selected_assets
            .iter()
            .map(|binding| binding.resource)
            .collect::<Vec<_>>(),
        vec![83, 84, 85, 86, 87, 88, 107, 108, 109]
    );
    assert_eq!(
        asset_map
            .protected_puyo2_copies
            .iter()
            .map(|binding| binding.resource)
            .collect::<Vec<_>>(),
        vec![158, 163, 164]
    );
}

#[test]
fn staff_roll_descriptor가_다른_resource로_이동하면_거부한다() {
    let mut links = fixture_links();
    let link = links
        .iter_mut()
        .find(|link| link.asset_name == "P1_STF2.BTX")
        .unwrap();
    link.descriptor_pointer_runtime += BTX_DESCRIPTOR_BYTES;

    let error = verify_puyo1_ending_asset_map(&links).unwrap_err();
    assert!(error.to_string().contains("P1_STF2.BTX"));
    assert!(error.to_string().contains("resource 불일치"));
}

#[test]
fn puyo2_보호_사본_이름_연결이_사라지면_거부한다() {
    let mut links = fixture_links();
    links.retain(|link| link.asset_name != "P2_END10.BTX");

    let error = verify_puyo1_ending_asset_map(&links).unwrap_err();
    assert!(error.to_string().contains("P2_END10.BTX"));
    assert!(error.to_string().contains("count=0"));
}
