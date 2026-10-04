use super::*;
use crate::isa::{
    encode_all, verify_bytes, verify_typed_source, InstructionKind as K, IsaProfile, Reg,
};
use crate::patch::region::{ImageRegion, RegionKind, UnchangedBoundary};
use crate::source::{FORM1_DATA_LEN, FORM1_DATA_OFFSET, SECTOR_LEN};

fn baseline() -> Vec<u8> {
    (0..512u32).map(|i| (i % 251) as u8).collect()
}

fn data_region() -> ImageRegion {
    ImageRegion::new(
        "data",
        0..256,
        RegionKind::Data,
        "조사로 확인한 텍스트 구간",
    )
}

fn code_region() -> ImageRegion {
    ImageRegion::new(
        "code",
        256..384,
        RegionKind::MachineCode,
        "QUEST.EXE 텍스트 세그먼트",
    )
}

fn protected_region() -> ImageRegion {
    ImageRegion::new(
        "protected",
        384..512,
        RegionKind::Protected,
        "무변경 선언 구간",
    )
}

fn data_write(id: &str, offset: u64, len: usize, base: &[u8]) -> ExpectedWrite {
    ExpectedWrite {
        id: id.into(),
        actor: "test".into(),
        purpose: "번역문 교체".into(),
        offset,
        expected_original: base[offset as usize..offset as usize + len].to_vec(),
        replacement: vec![0xAA; len],
        intent: WriteIntent::Data,
    }
}

// ── 통과 경로 ──────────────────────────────────────────────────

#[test]
fn accepts_a_well_formed_data_write() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(data_region())
        .write(data_write("w1", 16, 8, &b));
    let out = plan.apply(&b).expect("통과해야 함");
    assert_eq!(&out[16..24], &[0xAA; 8]);
    assert_eq!(&out[..16], &b[..16]);
    assert_eq!(&out[24..], &b[24..]);
}

#[test]
fn accepts_verified_machine_code_in_a_code_region() {
    let mut b = baseline();
    let code = encode_all(&[
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 0,
        },
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    b[256..256 + code.len()].copy_from_slice(&code);

    let new_source = [
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ];
    let verified = verify_typed_source(
        IsaProfile::DECLARED,
        0x8001_0000,
        "asm/font_hook.typed",
        &new_source,
    )
    .unwrap();
    let new_code = verified.bytes().to_vec();

    let plan = WritePlan::new().region(code_region()).write(ExpectedWrite {
        id: "hook".into(),
        actor: "font-hook".into(),
        purpose: "한글 글리프 경로 진입".into(),
        offset: 256,
        expected_original: code.clone(),
        replacement: new_code.clone(),
        intent: WriteIntent::MachineCode {
            verified: Box::new(verified),
        },
    });

    let out = plan.apply(&b).expect("검산된 기계어는 통과해야 함");
    assert_eq!(&out[256..256 + new_code.len()], new_code.as_slice());
}

#[test]
fn logical_file_relocation은_불변_원본을_sector별로_복사한다() {
    let mut baseline = vec![0u8; SECTOR_LEN * 4];
    for sector in 0..2usize {
        let start = sector * SECTOR_LEN + FORM1_DATA_OFFSET;
        for index in 0..FORM1_DATA_LEN {
            baseline[start + index] = (sector * 17 + index % 251) as u8;
        }
    }
    let source = LogicalFile::new(0, (FORM1_DATA_LEN * 2) as u32);
    let destination = LogicalFile::new(2, (FORM1_DATA_LEN * 2) as u32);
    let writes = plan_logical_copy(
        &baseline,
        source,
        0,
        destination,
        0,
        (FORM1_DATA_LEN * 2) as u64,
        "relocate",
        "test",
        "같은 논리 파일 extent 이동",
        "/TEST.EXE;1",
    )
    .unwrap();
    assert_eq!(writes.len(), 2);

    let mut plan = WritePlan::new();
    for write in writes {
        plan.regions.push(ImageRegion::new(
            format!("region:{}", write.id),
            write.range(),
            RegionKind::FileRelocation,
            "불변 원본 논리 파일 byte copy",
        ));
        plan.writes.push(write);
    }
    let output = plan.apply(&baseline).unwrap();
    for sector in 0..2usize {
        let source_start = sector * SECTOR_LEN + FORM1_DATA_OFFSET;
        let destination_start = (sector + 2) * SECTOR_LEN + FORM1_DATA_OFFSET;
        assert_eq!(
            &output[destination_start..destination_start + FORM1_DATA_LEN],
            &baseline[source_start..source_start + FORM1_DATA_LEN]
        );
    }
}

#[test]
fn file_relocation은_source와_다른_replacement를_거부한다() {
    let baseline = baseline();
    let plan = WritePlan::new()
        .region(ImageRegion::new(
            "relocation",
            128..136,
            RegionKind::FileRelocation,
            "불변 원본 byte copy",
        ))
        .write(ExpectedWrite {
            id: "tampered-copy".into(),
            actor: "test".into(),
            purpose: "변조 검출".into(),
            offset: 128,
            expected_original: baseline[128..136].to_vec(),
            replacement: vec![0xAA; 8],
            intent: WriteIntent::FileRelocation(FileRelocationProof {
                source_offset: 16,
                source_id: "/TEST.EXE;1".into(),
            }),
        });
    assert_eq!(
        plan.validate(&baseline),
        Err(PlanError::RelocationSourceMismatch {
            id: "tampered-copy".into(),
            source_offset: 16,
        })
    );
}

#[test]
fn file_relocation은_빈_source_id와_범위밖_source를_거부한다() {
    let baseline = baseline();
    let relocation_region = || {
        ImageRegion::new(
            "relocation",
            128..136,
            RegionKind::FileRelocation,
            "불변 원본 byte copy",
        )
    };
    let relocation_write = |source_offset, source_id: &str, replacement: Vec<u8>| ExpectedWrite {
        id: "copy".into(),
        actor: "test".into(),
        purpose: "원본 증명 검증".into(),
        offset: 128,
        expected_original: baseline[128..136].to_vec(),
        replacement,
        intent: WriteIntent::FileRelocation(FileRelocationProof {
            source_offset,
            source_id: source_id.into(),
        }),
    };

    let empty_id = WritePlan::new()
        .region(relocation_region())
        .write(relocation_write(16, " ", baseline[16..24].to_vec()));
    assert_eq!(
        empty_id.validate(&baseline),
        Err(PlanError::RelocationSourceIdEmpty { id: "copy".into() })
    );

    let outside = WritePlan::new()
        .region(relocation_region())
        .write(relocation_write(508, "/TEST.EXE;1", vec![0; 8]));
    assert_eq!(
        outside.validate(&baseline),
        Err(PlanError::RelocationSourceOutOfBounds {
            id: "copy".into(),
            end: 516,
            image_len: 512,
        })
    );
}

// ── 거부 경로 ──────────────────────────────────────────────────

#[test]
fn rejects_wrong_original_bytes() {
    let b = baseline();
    let mut w = data_write("w1", 16, 8, &b);
    w.expected_original[0] ^= 0xFF;
    let plan = WritePlan::new().region(data_region()).write(w);
    assert_eq!(
        plan.validate(&b),
        Err(PlanError::OriginalBytesMismatch {
            id: "w1".into(),
            offset: 16
        })
    );
}

#[test]
fn rejects_overlapping_writes_even_from_the_same_actor() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(data_region())
        .write(data_write("w1", 16, 8, &b))
        .write(data_write("w2", 20, 8, &b));
    assert!(matches!(
        plan.validate(&b),
        Err(PlanError::WritesOverlap { .. })
    ));
}

#[test]
fn rejects_writes_into_a_protected_region() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(protected_region())
        .write(data_write("w1", 400, 8, &b));
    assert_eq!(
        plan.validate(&b),
        Err(PlanError::ProtectedRegionWrite {
            id: "w1".into(),
            region: "protected".into()
        })
    );
}

#[test]
fn rejects_data_intent_targeting_a_machine_code_region() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(code_region())
        .write(data_write("w1", 260, 8, &b));
    assert!(matches!(
        plan.validate(&b),
        Err(PlanError::IntentRegionMismatch { .. })
    ));
}

#[test]
fn rejects_a_write_with_no_declared_region() {
    let b = baseline();
    let plan = WritePlan::new().write(data_write("w1", 16, 8, &b));
    assert_eq!(
        plan.validate(&b),
        Err(PlanError::NotInExactlyOneRegion {
            id: "w1".into(),
            matched: 0
        })
    );
}

#[test]
fn rejects_a_write_straddling_two_regions() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(data_region())
        .region(code_region())
        .write(data_write("w1", 252, 8, &b)); // 256 경계를 넘는다
    assert_eq!(
        plan.validate(&b),
        Err(PlanError::NotInExactlyOneRegion {
            id: "w1".into(),
            matched: 0
        })
    );
}

#[test]
fn rejects_machine_code_bytes_that_differ_from_the_verified_ones() {
    let mut b = baseline();
    let code = encode_all(&[
        K::Jr { rs: Reg::RA },
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    ]);
    b[256..256 + code.len()].copy_from_slice(&code);

    let verified = verify_typed_source(
        IsaProfile::DECLARED,
        0x8001_0000,
        "asm/mismatch.typed",
        &[
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 2,
            },
        ],
    )
    .unwrap();

    // 검산은 통과했지만 실제로 쓰는 바이트가 다르다.
    let plan = WritePlan::new().region(code_region()).write(ExpectedWrite {
        id: "hook".into(),
        actor: "font-hook".into(),
        purpose: "우회 시도".into(),
        offset: 256,
        expected_original: code.clone(),
        replacement: vec![0xFF; code.len()],
        intent: WriteIntent::MachineCode {
            verified: Box::new(verified),
        },
    });
    assert_eq!(
        plan.validate(&b),
        Err(PlanError::MachineCodeBytesMismatch { id: "hook".into() })
    );
}

#[test]
fn rejects_machine_code_verified_from_raw_bytes_only() {
    let mut b = baseline();
    let source = [K::Jr { rs: Reg::RA }];
    let code = encode_all(&source);
    b[256..256 + code.len()].copy_from_slice(&code);
    let verified = verify_bytes(IsaProfile::DECLARED, 0x8001_0000, &code).unwrap();

    let plan = WritePlan::new().region(code_region()).write(ExpectedWrite {
        id: "raw-hook".into(),
        actor: "font-hook".into(),
        purpose: "raw byte 우회 거부".into(),
        offset: 256,
        expected_original: code.clone(),
        replacement: code,
        intent: WriteIntent::MachineCode {
            verified: Box::new(verified),
        },
    });

    assert_eq!(
        plan.validate(&b),
        Err(PlanError::MachineCodeSourceMissing {
            id: "raw-hook".into()
        })
    );
}

#[test]
fn rejects_length_changing_replacement() {
    let b = baseline();
    let mut w = data_write("w1", 16, 8, &b);
    w.replacement.push(0);
    let plan = WritePlan::new().region(data_region()).write(w);
    assert!(matches!(
        plan.validate(&b),
        Err(PlanError::LengthMismatch { .. })
    ));
}

#[test]
fn rejects_duplicate_ids() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(data_region())
        .write(data_write("w1", 16, 8, &b))
        .write(data_write("w1", 32, 8, &b));
    assert_eq!(
        plan.validate(&b),
        Err(PlanError::DuplicateWriteId("w1".into()))
    );
}

#[test]
fn rejects_empty_reason_on_a_region() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(ImageRegion::new("data", 0..256, RegionKind::Data, ""))
        .write(data_write("w1", 16, 8, &b));
    assert!(matches!(
        plan.validate(&b),
        Err(PlanError::EmptyField { .. })
    ));
}

#[test]
fn rejects_overlapping_regions() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(ImageRegion::new("a", 0..256, RegionKind::Data, "근거"))
        .region(ImageRegion::new("b", 128..384, RegionKind::Data, "근거"));
    assert!(matches!(
        plan.validate(&b),
        Err(PlanError::RegionsOverlap { .. })
    ));
}

// ── 최종 diff 검증 ─────────────────────────────────────────────

#[test]
fn untracked_final_diff_is_rejected() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(data_region())
        .write(data_write("w1", 16, 8, &b));
    let mut tampered = plan.apply(&b).unwrap();
    // 계획 밖에서 한 바이트를 만졌다.
    tampered[100] ^= 0xFF;
    assert_eq!(
        plan.verify_final(&b, &tampered),
        Err(PlanError::UntrackedDiff { offset: 100 })
    );
}

#[test]
fn output_length_change_is_rejected() {
    let b = baseline();
    let plan = WritePlan::new().region(data_region());
    let mut short = b.clone();
    short.pop();
    assert!(matches!(
        plan.verify_final(&b, &short),
        Err(PlanError::OutputLengthChanged { .. })
    ));
}

#[test]
fn unchanged_boundary_overlapping_a_write_is_rejected() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(data_region())
        .write(data_write("w1", 16, 8, &b))
        .unchanged(UnchangedBoundary::new("keep", 0..64, "무변경 선언"));
    assert!(matches!(
        plan.validate(&b),
        Err(PlanError::UnchangedBoundaryViolated { .. })
    ));
}

#[test]
fn unchanged_boundary_far_from_writes_passes() {
    let b = baseline();
    let plan = WritePlan::new()
        .region(data_region())
        .write(data_write("w1", 16, 8, &b))
        .unchanged(UnchangedBoundary::new("keep", 256..512, "무변경 선언"));
    assert!(plan.apply(&b).is_ok());
}

// ── 섹터 인식 ──────────────────────────────────────────────────

#[test]
fn touched_sectors_reports_every_sector_a_write_lands_in() {
    let b = vec![0u8; 3 * crate::source::SECTOR_LEN];
    let plan = WritePlan::new()
        .region(ImageRegion::new(
            "r",
            0..b.len() as u64,
            RegionKind::Data,
            "테스트",
        ))
        .write(ExpectedWrite {
            id: "w".into(),
            actor: "test".into(),
            purpose: "테스트".into(),
            offset: crate::source::SECTOR_LEN as u64 - 2,
            expected_original: vec![0; 4],
            replacement: vec![1; 4],
            intent: WriteIntent::Data,
        });
    let sectors = plan.touched_sectors();
    assert!(sectors.contains(&0));
    assert!(sectors.contains(&1));
    assert_eq!(sectors.len(), 2);
}

#[test]
fn logical_write_within_one_sector_stays_one_expected_write() {
    let file = crate::coords::LogicalFile::new(100, 8192);
    let writes = plan_logical_write(
        file,
        10,
        &[0; 16],
        &[1; 16],
        "seq",
        "text-builder",
        "번역문 삽입",
        |_| WriteIntent::Data,
    )
    .expect("계획 생성");
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].offset, 100 * 2352 + 24 + 10);
}

#[test]
fn logical_write_crossing_a_sector_is_split() {
    let file = crate::coords::LogicalFile::new(100, 8192);
    // 논리 2046부터 8바이트 → 섹터 0에 2바이트, 섹터 1에 6바이트
    let writes = plan_logical_write(
        file,
        2046,
        &[0; 8],
        &[1; 8],
        "seq",
        "text-builder",
        "섹터 경계를 넘는 삽입",
        |_| WriteIntent::Data,
    )
    .expect("계획 생성");

    assert_eq!(writes.len(), 2, "섹터 경계에서 분할되지 않았다");
    assert_eq!(writes[0].replacement.len(), 2);
    assert_eq!(writes[1].replacement.len(), 6);
    assert_eq!(writes[0].offset, 100 * 2352 + 24 + 2046);
    assert_eq!(writes[1].offset, 101 * 2352 + 24);
    // 두 쓰기 사이에는 304바이트의 보호 필드가 있다.
    assert_eq!(writes[1].offset - (writes[0].offset + 2), 304);
}

#[test]
fn typed_machine_code는_mode2_sector를_넘어도_전체source로_검산한다() {
    let file = LogicalFile::new(0, (FORM1_DATA_LEN * 2) as u32);
    let source = vec![
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 1,
        };
        32
    ];
    let verified = verify_typed_source(
        IsaProfile::DECLARED,
        0x8001_0000,
        "patch::tests::sector_spanning_typed_source",
        &source,
    )
    .unwrap();
    let writes = plan_logical_machine_code(
        file,
        2_000,
        &[0; 128],
        &verified,
        "spanning-hook",
        "test",
        "두 sector를 넘는 typed hook",
    )
    .unwrap();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].replacement.len(), 48);
    assert_eq!(writes[1].replacement.len(), 80);

    let baseline = vec![0u8; SECTOR_LEN * 2];
    let mut plan = WritePlan::new();
    for write in writes {
        plan.regions.push(ImageRegion::new(
            format!("region:{}", write.id),
            write.range(),
            RegionKind::MachineCode,
            "sector별 typed 기계어",
        ));
        plan.writes.push(write);
    }
    let output = plan.apply(&baseline).unwrap();
    assert_eq!(
        &output[FORM1_DATA_OFFSET + 2_000..FORM1_DATA_OFFSET + FORM1_DATA_LEN],
        &verified.bytes()[..48]
    );
    let second = SECTOR_LEN + FORM1_DATA_OFFSET;
    assert_eq!(&output[second..second + 80], &verified.bytes()[48..]);
}

#[test]
fn sector분할_typed_machine_code는_조각누락을_거부한다() {
    let file = LogicalFile::new(0, (FORM1_DATA_LEN * 2) as u32);
    let source = vec![
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 1,
        };
        32
    ];
    let verified = verify_typed_source(
        IsaProfile::DECLARED,
        0x8001_0000,
        "patch::tests::incomplete_sector_spanning_typed_source",
        &source,
    )
    .unwrap();
    let mut writes = plan_logical_machine_code(
        file,
        2_000,
        &[0; 128],
        &verified,
        "incomplete-hook",
        "test",
        "누락을 거부할 typed hook",
    )
    .unwrap();
    let write = writes.remove(0);
    let plan = WritePlan::new()
        .region(ImageRegion::new(
            "remaining-segment",
            write.range(),
            RegionKind::MachineCode,
            "일부만 남긴 sector 조각",
        ))
        .write(write);
    assert_eq!(
        plan.validate(&vec![0u8; SECTOR_LEN * 2]),
        Err(PlanError::MachineCodeSegmentSetInvalid {
            group_id: "test:incomplete-hook".into(),
        })
    );
}

#[test]
fn logical_write_past_the_end_of_the_file_is_rejected() {
    let file = crate::coords::LogicalFile::new(100, 64);
    assert!(plan_logical_write(
        file,
        60,
        &[0; 8],
        &[1; 8],
        "seq",
        "test",
        "범위 초과",
        |_| WriteIntent::Data,
    )
    .is_none());
}

#[test]
fn split_writes_do_not_overlap_and_validate_together() {
    let image = vec![0u8; 3 * crate::source::SECTOR_LEN];
    let file = crate::coords::LogicalFile::new(0, 4096);
    let writes = plan_logical_write(
        file,
        2040,
        &[0; 16],
        &[7; 16],
        "seq",
        "text-builder",
        "경계 삽입",
        |_| WriteIntent::Data,
    )
    .unwrap();

    let mut plan = WritePlan::new().region(ImageRegion::new(
        "r",
        0..image.len() as u64,
        RegionKind::Data,
        "테스트",
    ));
    for w in writes {
        plan = plan.write(w);
    }
    let out = plan
        .apply(&image)
        .expect("분할된 쓰기가 검증을 통과해야 함");
    // 섹터 0 끝 8바이트
    assert_eq!(&out[24 + 2040..24 + 2048], &[7; 8]);
    // 섹터 1 시작 8바이트
    assert_eq!(&out[2352 + 24..2352 + 24 + 8], &[7; 8]);
    // 보호 필드는 건드리지 않았다.
    assert_eq!(&out[24 + 2048..2352 + 24], &image[24 + 2048..2352 + 24]);
}
