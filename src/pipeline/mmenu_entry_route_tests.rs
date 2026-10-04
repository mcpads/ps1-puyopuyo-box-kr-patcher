use super::*;

use crate::isa::{encode_all, InstructionKind as K};
use std::collections::BTreeMap;

#[test]
fn 두_wrapper와_네_argument_entry를_같은_참조_분모에_둔다() {
    let actual = ENTRY_CANDIDATES
        .iter()
        .map(|candidate| (candidate.runtime, candidate.role))
        .collect::<BTreeMap<_, _>>();
    let expected = BTreeMap::from([
        (
            RECORD_WRAPPER_WITH_DESTINATION_RUNTIME,
            MmenuEntryCandidateRole::Op12RecordWrapperWithDestination,
        ),
        (
            RECORD_WRAPPER_WITHOUT_DESTINATION_RUNTIME,
            MmenuEntryCandidateRole::Op12RecordWrapperWithoutDestination,
        ),
        (
            RELEASE_SELECTED_DESCRIPTOR_RUNTIME,
            MmenuEntryCandidateRole::ReleaseSelectedDescriptor,
        ),
        (
            QUERY_SELECTED_DESCRIPTOR_RUNTIME,
            MmenuEntryCandidateRole::QuerySelectedDescriptor,
        ),
        (
            PROGRAMMATIC_LOADER_RUNTIME,
            MmenuEntryCandidateRole::ProgrammaticLoader,
        ),
        (
            STAGE_SELECTED_DESCRIPTOR_RUNTIME,
            MmenuEntryCandidateRole::StageSelectedDescriptor,
        ),
    ]);

    assert_eq!(actual, expected);
}

#[test]
fn op12_type1_record는_정렬과_reserved를_숨기지_않는다() {
    let mut raw = vec![0; 24];
    raw[1..9].copy_from_slice(&[0x28, 0x12, 0x97, 1, 62, 0, 0, 0]);
    raw[12..20].copy_from_slice(&[0x28, 0x12, 0x08, 1, 63, 0, 1, 0]);
    assert_eq!(
        alternate_type1_records(&raw),
        vec![
            AlternateType1Record {
                offset: 1,
                resource_id: 62,
                reserved: 0,
            },
            AlternateType1Record {
                offset: 12,
                resource_id: 63,
                reserved: 1,
            },
        ]
    );
}

#[test]
fn direct_jal_pointer_address_load를_서로_분리해_센다() {
    let target = RECORD_WRAPPER_WITH_DESTINATION_RUNTIME;
    let mut raw = encode_all(&[
        K::Jal { target },
        K::Lui {
            rt: Reg::V0,
            imm: 0x8002,
        },
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -0x46EC,
        },
    ]);
    raw.push(0);
    raw.extend_from_slice(&target.to_le_bytes());

    assert_eq!(
        count_static_references(&raw, target),
        MmenuStaticReferenceCounts {
            direct_jal_words: 1,
            raw_pointer_bytes: 1,
            address_loads: 1,
        }
    );
}

#[test]
fn 알려진_selector_record_일곱_형식에서_두_대상만_센다() {
    let raw = [
        0x28, 0x01, 0x01, 0xFF, 62, 0, // OP28/sub01 MMENU
        0x28, 0x03, 0x00, 0xFF, 165, 0, // OP28/sub03 MM_MES
        0x28, 0x04, 0x01, 0xFF, 62, 0, // OP28/sub04 MMENU
        0x28, 0x10, 0x97, 0x00, 165, 0, // OP28/sub10 MM_MES
        0x28, 0x12, 0x97, 0x01, 62, 0, // OP28/sub12 MMENU
        0x39, 0x00, 165, 0, 0xFF, 0xFF, // main OP39 MM_MES
        0xC3, 0xFF, 0xFF, 0x01, 62, 0, // main OPC3 MMENU
        0x28, 0x12, 0x97, 0x00, 164, 0, // 조사 대상이 아닌 resource
    ];

    let records = known_selector_target_records("fixture", &raw);
    assert_eq!(records.len(), KNOWN_SELECTOR_RECORD_FORMS);
    assert_eq!(records[0].form, MmenuKnownSelectorRecordForm::Op28Sub01);
    assert_eq!(records[1].form, MmenuKnownSelectorRecordForm::Op28Sub03);
    assert_eq!(records[2].form, MmenuKnownSelectorRecordForm::Op28Sub04);
    assert_eq!(records[3].form, MmenuKnownSelectorRecordForm::Op28Sub10);
    assert_eq!(records[4].form, MmenuKnownSelectorRecordForm::Op28Sub12);
    assert_eq!(records[5].form, MmenuKnownSelectorRecordForm::MainOp39);
    assert_eq!(records[6].form, MmenuKnownSelectorRecordForm::MainOpC3);
    assert_eq!(records[0].target, MmenuKnownSelectorTarget::MmenuSequence);
    assert_eq!(records[1].target, MmenuKnownSelectorTarget::MmMesGraphics);
}
