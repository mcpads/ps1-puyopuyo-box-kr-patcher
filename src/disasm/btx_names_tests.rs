use crate::coords::LogicalFile;

use super::*;

const SECTOR_LEN: usize = 2048;

fn map(name: &'static str, text_len: usize) -> &'static ExeMap {
    Box::leak(Box::new(ExeMap {
        name,
        load_addr: 0x8001_0000,
        text_len: text_len as u32,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, (text_len + 0x800) as u32),
    }))
}

fn archive(names: &[&str]) -> VddArchive {
    let mut raw = vec![0u8; (names.len() + 1) * SECTOR_LEN];
    raw[..4].copy_from_slice(&(names.len() as u32).to_le_bytes());
    for (index, name) in names.iter().enumerate() {
        let table = 4 + index * 24;
        raw[table..table + name.len()].copy_from_slice(name.as_bytes());
        raw[table + 16..table + 20].copy_from_slice(&((index + 1) as u32).to_le_bytes());
        raw[table + 20..table + 24].copy_from_slice(&1u32.to_le_bytes());
        raw[(index + 1) * SECTOR_LEN] = index as u8;
    }
    VddArchive::parse(&raw).unwrap()
}

fn spec(expected_btx: usize) -> BtxNameConsumerSpec {
    BtxNameConsumerSpec {
        exe: "TEST.EXE",
        archive: "/TEST_GRP.VDD;1",
        expected_btx,
    }
}

fn single_asset_text() -> Vec<u8> {
    let words = [
        0x3C08_8001u32, // lui t0, 0x8001
        0x2508_007Cu32, // addiu t0, t0, 0x7C
        0x03E0_0008u32, // jr ra
        0x0000_0000u32, // nop
    ];
    let mut text = vec![0u8; 132];
    for (index, word) in words.iter().enumerate() {
        text[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    // block 0x20, VDD descriptor 0x24..0x48, BTX descriptor 0x48..0x6C.
    text[0x24..0x28].copy_from_slice(&0x8001_006Cu32.to_le_bytes());
    text[0x48..0x4C].copy_from_slice(&0x8001_007Cu32.to_le_bytes());
    text[0x6C..0x79].copy_from_slice(b"TEST_GRP.VDD\0");
    text[0x7C..0x82].copy_from_slice(b"A.BTX\0");
    text
}

fn two_asset_text(second_pointer_offset: usize, reverse_names: bool) -> Vec<u8> {
    let mut text = vec![0u8; 256];
    text[0x24..0x28].copy_from_slice(&0x8001_00C0u32.to_le_bytes());
    let (first_name, second_name) = if reverse_names {
        (0x8001_00D8u32, 0x8001_00D0u32)
    } else {
        (0x8001_00D0u32, 0x8001_00D8u32)
    };
    text[0x48..0x4C].copy_from_slice(&first_name.to_le_bytes());
    text[second_pointer_offset..second_pointer_offset + 4]
        .copy_from_slice(&second_name.to_le_bytes());
    text[0xC0..0xCD].copy_from_slice(b"TEST_GRP.VDD\0");
    text[0xD0..0xD6].copy_from_slice(b"A.BTX\0");
    text[0xD8..0xDE].copy_from_slice(b"B.BTX\0");
    text
}

#[test]
fn 물리_btx와_정확한_nul_이름표를_일대일로_연결한다() {
    let text = single_asset_text();
    let grp = archive(&["A.BTX"]);
    let analysis = audit_btx_name_consumers(
        &[spec(1)],
        [(map("TEST.EXE", text.len()), text.as_slice())],
        [("/TEST_GRP.VDD;1", &grp)],
    )
    .unwrap();
    let link = &analysis.links[0];

    assert_eq!(link.exe_text_offset, 0x7C);
    assert_eq!(link.runtime_addr, 0x8001_007C);
    assert_eq!(link.static_ref_sites, [0x8001_0000]);
    assert_eq!(link.entry_reached_ref_sites, [0x8001_0000]);
    assert_eq!(link.table_runtime_start, 0x8001_007C);
    assert_eq!(link.table_runtime_end, 0x8001_0082);
    assert_eq!(link.table_static_ref_sites, [0x8001_0000]);
    assert_eq!(link.descriptor_pointer_offset, 0x48);
    assert_eq!(link.descriptor_pointer_runtime, 0x8001_0048);
    assert_eq!(link.descriptor_runtime_start, 0x8001_0020);
    assert_eq!(link.descriptor_runtime_end, 0x8001_006C);
    assert_eq!(link.static_evidence(), "entry_trace_static_ref");
}

#[test]
fn 긴_이름의_suffix를_별도_이름표로_세지_않는다() {
    let rows = scan_btx_literals(b"COM_FONT.BTX\0FONT.BTX\0");

    assert_eq!(
        rows,
        [
            ("COM_FONT.BTX".to_string(), 0),
            ("FONT.BTX".to_string(), 13)
        ]
    );
}

#[test]
fn 누락_중복_여분_이름표를_거부한다() {
    let grp = archive(&["A.BTX"]);
    let missing = vec![0u8; 8];
    assert!(matches!(
        audit_btx_name_consumers(
            &[spec(1)],
            [(map("TEST.EXE", 8), missing.as_slice())],
            [("/TEST_GRP.VDD;1", &grp)]
        ),
        Err(BtxNameError::MissingLiteral { .. })
    ));

    let mut duplicate = b"A.BTX\0A.BTX\0".to_vec();
    duplicate.resize(16, 0);
    assert!(matches!(
        audit_btx_name_consumers(
            &[spec(1)],
            [(map("TEST.EXE", 16), duplicate.as_slice())],
            [("/TEST_GRP.VDD;1", &grp)]
        ),
        Err(BtxNameError::DuplicateLiteral { .. })
    ));

    let mut extra = b"A.BTX\0B.BTX\0".to_vec();
    extra.resize(16, 0);
    assert!(matches!(
        audit_btx_name_consumers(
            &[spec(1)],
            [(map("TEST.EXE", 16), extra.as_slice())],
            [("/TEST_GRP.VDD;1", &grp)]
        ),
        Err(BtxNameError::ExtraLiteral { .. })
    ));
}

#[test]
fn 이름표_descriptor_포인터의_누락과_중복을_거부한다() {
    let grp = archive(&["A.BTX"]);
    let mut missing = b"A.BTX\0".to_vec();
    missing.resize(8, 0);
    assert!(matches!(
        audit_btx_name_consumers(
            &[spec(1)],
            [(map("TEST.EXE", 8), missing.as_slice())],
            [("/TEST_GRP.VDD;1", &grp)]
        ),
        Err(BtxNameError::MissingDescriptorPointer { .. })
    ));

    let mut duplicate = b"A.BTX\0".to_vec();
    duplicate.resize(8, 0);
    duplicate.extend_from_slice(&0x8001_0000u32.to_le_bytes());
    duplicate.extend_from_slice(&0x8001_0000u32.to_le_bytes());
    assert!(matches!(
        audit_btx_name_consumers(
            &[spec(1)],
            [(map("TEST.EXE", 16), duplicate.as_slice())],
            [("/TEST_GRP.VDD;1", &grp)]
        ),
        Err(BtxNameError::DuplicateDescriptorPointer { .. })
    ));
}

#[test]
fn descriptor는_36바이트_격자를_요구한다() {
    let grp = archive(&["A.BTX", "B.BTX"]);
    let text = two_asset_text(0x70, false);

    assert!(matches!(
        audit_btx_name_consumers(
            &[spec(2)],
            [(map("TEST.EXE", text.len()), text.as_slice())],
            [("/TEST_GRP.VDD;1", &grp)]
        ),
        Err(BtxNameError::DescriptorLayout { .. })
    ));
}

#[test]
fn descriptor_순서는_grp_엔트리_순서와_다를_수_있다() {
    let grp = archive(&["A.BTX", "B.BTX"]);
    let text = two_asset_text(0x6C, true);
    let analysis = audit_btx_name_consumers(
        &[spec(2)],
        [(map("TEST.EXE", text.len()), text.as_slice())],
        [("/TEST_GRP.VDD;1", &grp)],
    )
    .unwrap();

    assert_eq!(analysis.links[0].descriptor_pointer_offset, 0x6C);
    assert_eq!(analysis.links[1].descriptor_pointer_offset, 0x48);
}

#[test]
fn descriptor_격자의_중간_slot은_btx_포인터가_아닐_수_있다() {
    let grp = archive(&["A.BTX", "B.BTX"]);
    let text = two_asset_text(0x90, false);
    let analysis = audit_btx_name_consumers(
        &[spec(2)],
        [(map("TEST.EXE", text.len()), text.as_slice())],
        [("/TEST_GRP.VDD;1", &grp)],
    )
    .unwrap();

    assert_eq!(analysis.links[1].descriptor_pointer_offset, 0x90);
}

#[test]
fn spec와_입력의_중복과_누락을_거부한다() {
    let grp = archive(&["A.BTX"]);
    let text = [0u8; 4];
    assert_eq!(
        audit_btx_name_consumers(
            &[],
            std::iter::empty(),
            std::iter::empty::<(&str, &VddArchive)>()
        ),
        Err(BtxNameError::EmptySpecs)
    );
    assert_eq!(
        audit_btx_name_consumers(
            &[spec(1), spec(1)],
            [(map("TEST.EXE", 4), text.as_slice())],
            [("/TEST_GRP.VDD;1", &grp)]
        ),
        Err(BtxNameError::DuplicateSpecExecutable("TEST.EXE"))
    );
    assert_eq!(
        audit_btx_name_consumers(&[spec(1)], std::iter::empty(), [("/TEST_GRP.VDD;1", &grp)]),
        Err(BtxNameError::MissingExecutable("TEST.EXE"))
    );

    assert_eq!(
        audit_btx_name_consumers(
            &[spec(1)],
            [
                (map("TEST.EXE", 4), text.as_slice()),
                (map("EXTRA.EXE", 4), text.as_slice())
            ],
            [("/TEST_GRP.VDD;1", &grp)]
        ),
        Err(BtxNameError::UnexpectedExecutable("EXTRA.EXE"))
    );
    assert_eq!(
        audit_btx_name_consumers(
            &[spec(1)],
            [(map("TEST.EXE", 4), text.as_slice())],
            [("/TEST_GRP.VDD;1", &grp), ("/EXTRA_GRP.VDD;1", &grp)]
        ),
        Err(BtxNameError::UnexpectedArchive(
            "/EXTRA_GRP.VDD;1".to_string()
        ))
    );
}

#[test]
fn report는_runtime_미판정을_명시한다() {
    let text = single_asset_text();
    let grp = archive(&["A.BTX"]);
    let analysis = audit_btx_name_consumers(
        &[spec(1)],
        [(map("TEST.EXE", text.len()), text.as_slice())],
        [("/TEST_GRP.VDD;1", &grp)],
    )
    .unwrap();
    let report = render_btx_name_consumers_tsv(&analysis);

    assert!(report.starts_with("exe\tarchive\tasset_name"));
    assert!(report.contains("\tunverified\t"));
    assert_eq!(report.lines().count(), 2);
}
