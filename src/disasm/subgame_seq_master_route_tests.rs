use crate::coords::{ExeMap, LogicalFile};

use super::*;

type RouteFixture = (ExeMap, Vec<u8>, Vec<(&'static str, Vec<u8>)>);

fn map(text_len: usize) -> ExeMap {
    ExeMap {
        name: PUYO2_EXE,
        load_addr: 0x8001_0000,
        text_len: text_len as u32,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, (text_len + 0x800) as u32),
    }
}

fn put_record(raw: &mut [u8], offset: usize, flags: u8, resource: u16) {
    raw[offset..offset + 8].copy_from_slice(&[
        0x28,
        0x10,
        flags,
        0x01,
        resource as u8,
        (resource >> 8) as u8,
        0,
        0,
    ]);
}

fn route_fixture() -> RouteFixture {
    let root_offset = (RESIDENT_ROOT_RUNTIME - 0x8001_0000) as usize;
    let mut text = vec![0; root_offset + 16];
    put_record(&mut text, root_offset, 0x17, PP2_ALL_RESOURCE);
    text[root_offset + 8..root_offset + 12]
        .copy_from_slice(&RESIDENT_ROOT_DESTINATION.to_le_bytes());

    let mut pp1_all = vec![0; 0x0150];
    put_record(&mut pp1_all, 0x0120, 0x97, PP1_HITO_RESOURCE);
    put_record(&mut pp1_all, 0x0148, 0x97, PP1_FUTA_RESOURCE);
    let mut pp2_all = vec![0; 0x03CC];
    put_record(&mut pp2_all, 0x0270, 0x97, PP2_HITO_RESOURCE);
    put_record(&mut pp2_all, 0x02B8, 0x97, PP2_HITO_RESOURCE);
    put_record(&mut pp2_all, 0x03C4, 0x97, PP2_FUTA_RESOURCE);

    (
        map(text.len()),
        text,
        vec![("PP1_ALL.SEQ", pp1_all), ("PP2_ALL.SEQ", pp2_all)],
    )
}

fn source_map<'a>(sources: &'a [(&'static str, Vec<u8>)]) -> BTreeMap<&'a str, &'a [u8]> {
    sources
        .iter()
        .map(|(name, raw)| (*name, raw.as_slice()))
        .collect()
}

#[test]
fn resident_root와_master_child_graph를_함께_받는다() {
    let (map, text, sources) = route_fixture();
    assert_eq!(
        verify_route_records(&map, &text, &source_map(&sources)),
        Ok((2, 3))
    );
}

#[test]
fn pp1_all_incoming이_생기면_dormant_보호_계약을_거부한다() {
    let (map, text, mut sources) = route_fixture();
    let mut unrelated = vec![0; 8];
    put_record(&mut unrelated, 0, 0x97, PP1_ALL_RESOURCE);
    sources.push(("UNRELATED.SEQ", unrelated));

    let result = verify_route_records(&map, &text, &source_map(&sources));
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("resource=0x0060, expected=0, actual=1"));
}

#[test]
fn pp2_master_child_edge가_빠지면_route를_받지_않는다() {
    let (map, text, mut sources) = route_fixture();
    let pp2_all = &mut sources
        .iter_mut()
        .find(|(name, _)| *name == "PP2_ALL.SEQ")
        .expect("PP2_ALL fixture")
        .1;
    pp2_all[0x03C4..0x03CC].fill(0);

    let result = verify_route_records(&map, &text, &source_map(&sources));
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("PP2_ALL.SEQ master record 불일치"));
}

#[test]
fn 비정렬_record_모양은_canonical_incoming으로_세지_않는다() {
    let (map, text, mut sources) = route_fixture();
    let mut unrelated = vec![0; 12];
    unrelated[1..9].copy_from_slice(&[0x28, 0x10, 0x97, 0x01, 0x60, 0, 0, 0]);
    sources.push(("UNALIGNED.SEQ", unrelated));

    assert_eq!(
        verify_route_records(&map, &text, &source_map(&sources)),
        Ok((2, 3))
    );
}

#[test]
fn pp1_all_descriptor_raw_pointer가_생기면_정적_보호_계약을_거부한다() {
    let (map, mut text, _) = route_fixture();
    text[..4].copy_from_slice(&PP1_ALL_DESCRIPTOR_RUNTIME.to_le_bytes());

    let result = verify_no_pp1_all_static_reference(&map, &text);
    assert!(result.unwrap_err().to_string().contains("raw_pointer=1"));
}
