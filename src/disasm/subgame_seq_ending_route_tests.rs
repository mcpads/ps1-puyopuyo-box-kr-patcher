use super::*;

fn route_fixture() -> (Vec<u8>, Vec<u8>) {
    let mut pp2_all = vec![0; PP2_ALL_LEN];
    for (index, &offset) in PP2_TTL_SELECTION_OFFSETS.iter().enumerate() {
        let flags = if index < 2 { 0x97 } else { 0x17 };
        pp2_all[offset..offset + 8].copy_from_slice(&selection_record(flags));
    }
    for &(offset, index) in &PP2_TTL_CALLS {
        pp2_all[offset..offset + 8].copy_from_slice(&call_record(index));
    }

    let mut pp2_ttl = vec![0; PP2_TTL_LEN];
    for (index, (&record_offset, &stream_offset)) in PP2_TTL_EXPORT_RECORDS
        .iter()
        .zip(PP2_TTL_EXPORT_STREAMS.iter())
        .enumerate()
    {
        let mut export = [0x03, 0, 0, 0, 0, 0, 0, 0];
        export[4..8].copy_from_slice(&runtime_pointer(record_offset).unwrap());
        pp2_ttl[index * 8..index * 8 + 8].copy_from_slice(&export);

        let mut target = [0x1C, 0x02, 0, 0, 0, 0, 0, 0];
        target[4..8].copy_from_slice(&runtime_pointer(stream_offset).unwrap());
        pp2_ttl[record_offset..record_offset + 8].copy_from_slice(&target);
    }
    pp2_ttl[P2_END10_SETUP_OFFSET..P2_END10_SETUP_OFFSET + 48].copy_from_slice(
        &setup_record(
            P2_END10_RESOURCE,
            P2_END10_TABLE_OFFSET,
            P2_END10_UPLOAD_PARAMETER,
        )
        .unwrap(),
    );
    pp2_ttl[P2_END15_SETUP_OFFSET..P2_END15_SETUP_OFFSET + 48].copy_from_slice(
        &setup_record(
            P2_END15_RESOURCE,
            P2_END15_TABLE_OFFSET,
            P2_END15_UPLOAD_PARAMETER,
        )
        .unwrap(),
    );
    pp2_ttl[P2_END16_SETUP_OFFSET..P2_END16_SETUP_OFFSET + 48].copy_from_slice(
        &setup_record(
            P2_END16_RESOURCE,
            P2_END16_TABLE_OFFSET,
            P2_END16_UPLOAD_PARAMETER,
        )
        .unwrap(),
    );
    (pp2_all, pp2_ttl)
}

#[test]
fn pp2_all에서_credits와_final_ending_btx_setup까지_연결한다() {
    let (pp2_all, pp2_ttl) = route_fixture();
    let analysis = verify_route_sources(&pp2_all, &pp2_ttl).unwrap();

    assert_eq!(analysis.pp2_ttl_selection_records, 5);
    assert_eq!(analysis.pp2_ttl_call_records, 4);
    assert_eq!(analysis.credits_export_index, 3);
    assert_eq!(analysis.credits_export_offset, 0x331C);
    assert_eq!(analysis.final_ending_export_index, 4);
    assert_eq!(analysis.final_ending_export_offset, 0x4520);
    assert_eq!(analysis.end10_resource, 0x47);
    assert_eq!(analysis.end15_resource, 0x4C);
    assert_eq!(analysis.end16_resource, 0x4D);
}

#[test]
fn final_ending_export_index가_바뀌면_거부한다() {
    let (mut pp2_all, pp2_ttl) = route_fixture();
    pp2_all[0x038C + 4..0x038C + 8].copy_from_slice(&3u32.to_le_bytes());

    let result = verify_route_sources(&pp2_all, &pp2_ttl);
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("PP2_ALL.SEQ ending route 불일치"));
}

#[test]
fn credits_export_index가_바뀌면_거부한다() {
    let (mut pp2_all, pp2_ttl) = route_fixture();
    pp2_all[0x03B4 + 4..0x03B4 + 8].copy_from_slice(&2u32.to_le_bytes());

    let result = verify_route_sources(&pp2_all, &pp2_ttl);
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("PP2_ALL.SEQ ending route 불일치"));
}

#[test]
fn end10_table_pointer가_바뀌면_거부한다() {
    let (pp2_all, mut pp2_ttl) = route_fixture();
    pp2_ttl[P2_END10_SETUP_OFFSET + 12..P2_END10_SETUP_OFFSET + 16]
        .copy_from_slice(&0u32.to_le_bytes());

    let result = verify_route_sources(&pp2_all, &pp2_ttl);
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("PP2_TTL.SEQ ending route 불일치"));
}

#[test]
fn end15_table_pointer가_바뀌면_거부한다() {
    let (pp2_all, mut pp2_ttl) = route_fixture();
    pp2_ttl[P2_END15_SETUP_OFFSET + 12..P2_END15_SETUP_OFFSET + 16]
        .copy_from_slice(&0u32.to_le_bytes());

    let result = verify_route_sources(&pp2_all, &pp2_ttl);
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("PP2_TTL.SEQ ending route 불일치"));
}

#[test]
fn 없는_export_2_call을_추가하면_call_분모가_거부한다() {
    let (mut pp2_all, pp2_ttl) = route_fixture();
    pp2_all[0x0400..0x0408].copy_from_slice(&call_record(2));

    let result = verify_route_sources(&pp2_all, &pp2_ttl);
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("PP2_ALL PP2_TTL call route 불일치"));
}
