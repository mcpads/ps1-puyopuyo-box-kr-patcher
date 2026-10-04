use super::*;

const EXE_A: &str = "A.EXE";
const EXE_B: &str = "B.EXE";
const CODE_A_RUNTIME: u32 = 0x8001_1000;
const CODE_B_RUNTIME: u32 = 0x8001_2000;
const WORK_A_RUNTIME: u32 = 0x8004_0000;
const WORK_B_RUNTIME: u32 = 0x8004_1000;

fn place(vram: &mut [u8], data: &[u8], row_bytes: usize, x_words: usize, y: usize) {
    for (row, source) in data.chunks_exact(row_bytes).enumerate() {
        let start = (y + row) * 2048 + x_words * 2;
        vram[start..start + row_bytes].copy_from_slice(source);
    }
}

fn identity(seed: u8) -> [u8; BTX_DESCRIPTOR_IDENTITY_BYTES] {
    std::array::from_fn(|index| seed.wrapping_add(index as u8))
}

fn code(seed: u8) -> Vec<u8> {
    (0..32).map(|index| seed.wrapping_add(index)).collect()
}

fn raw(seed: u8) -> Vec<u8> {
    (0..32)
        .map(|index| seed.wrapping_mul(3).wrapping_add(index * 7))
        .collect()
}

fn source<'a>(
    finding_id: &'a str,
    exe: &'static str,
    raw: &'a [u8],
    identity: &'a [u8],
    code: &'a [u8],
    descriptor_entry_runtime: u32,
) -> PendingBtxRuntimeSource<'a> {
    let digest = format!("{:x}", Sha256::digest(raw));
    PendingBtxRuntimeSource {
        finding_id,
        exe,
        archive: if exe == EXE_A { "/A" } else { "/B" },
        asset_name: finding_id,
        format: PixelFormat::Indexed8,
        raw,
        expected_sha256: Box::leak(digest.into_boxed_str()),
        descriptor_entry_runtime,
        original_source_descriptor_identity: identity,
        working_descriptor_runtime: if exe == EXE_A {
            WORK_A_RUNTIME
        } else {
            WORK_B_RUNTIME
        },
        selection_code_runtime: if exe == EXE_A {
            CODE_A_RUNTIME
        } else {
            CODE_B_RUNTIME
        },
        selection_code: code,
    }
}

#[test]
fn active_exe_descriptor와_vram상주를_분리한다() {
    let code_a = code(0x31);
    let code_b = code(0x81);
    let identity_a1 = identity(0x11);
    let identity_a2 = identity(0x51);
    let identity_b = identity(0x91);
    let raw_a1 = raw(3);
    let raw_a2 = raw(9);
    let raw_b = raw(17);
    let sources = [
        source("F1", EXE_A, &raw_a1, &identity_a1, &code_a, 0x8003_0000),
        source("F2", EXE_A, &raw_a2, &identity_a2, &code_a, 0x8003_0024),
        source("F3", EXE_B, &raw_b, &identity_b, &code_b, 0x8003_1000),
    ];
    let mut ram = vec![0; PSX_MAIN_RAM_LEN];
    let code_offset = psx_main_ram_offset(CODE_A_RUNTIME, code_a.len()).unwrap();
    ram[code_offset..code_offset + code_a.len()].copy_from_slice(&code_a);
    for (runtime, value) in [(0x8003_0000, &identity_a1), (0x8003_0024, &identity_a2)] {
        let offset = psx_main_ram_offset(runtime, value.len()).unwrap();
        ram[offset..offset + value.len()].copy_from_slice(value);
    }
    let working_offset = psx_main_ram_offset(WORK_A_RUNTIME, identity_a1.len()).unwrap();
    ram[working_offset..working_offset + identity_a1.len()].copy_from_slice(&identity_a1);
    let mut vram = vec![0; VRAM_BYTE_LEN];
    place(&mut vram, &raw_a1, 8, 7, 13);

    let observation = inspect_pending_btx_runtime(&ram, &vram, sources).unwrap();
    assert_eq!(observation.exe, EXE_A);
    assert_eq!(observation.rows.len(), 2);
    assert!(observation.rows[0].descriptor_selected);
    assert!(observation.rows[0].source_descriptor_populated);
    assert!(observation.rows[0].source_descriptor_original_match);
    assert_eq!(observation.rows[0].vram_layouts, [(8, 4)]);
    assert_eq!(
        observation.rows[0].observation_status(),
        "descriptor-selected-vram-resident-screen-unverified"
    );
    assert_eq!(
        observation.rows[1].observation_status(),
        "not-observed-at-snapshot"
    );
    let report = render_pending_btx_runtime_tsv(&observation);
    assert!(report.contains("raw-ram-vram-snapshot-only-load-hit-primitive-draw-screen-unverified"));
}

#[test]
fn source_descriptor의_runtime변환을_원본불일치로_분리한다() {
    let code_a = code(0x31);
    let original_identity = identity(0x11);
    let runtime_identity = identity(0x71);
    let raw_a = raw(3);
    let sources = [source(
        "F1",
        EXE_A,
        &raw_a,
        &original_identity,
        &code_a,
        0x8003_0000,
    )];
    let mut ram = vec![0; PSX_MAIN_RAM_LEN];
    let code_offset = psx_main_ram_offset(CODE_A_RUNTIME, code_a.len()).unwrap();
    ram[code_offset..code_offset + code_a.len()].copy_from_slice(&code_a);
    for runtime in [0x8003_0000, WORK_A_RUNTIME] {
        let offset = psx_main_ram_offset(runtime, runtime_identity.len()).unwrap();
        ram[offset..offset + runtime_identity.len()].copy_from_slice(&runtime_identity);
    }
    let vram = vec![0; VRAM_BYTE_LEN];
    let observation = inspect_pending_btx_runtime(&ram, &vram, sources).unwrap();
    assert!(observation.rows[0].source_descriptor_populated);
    assert!(!observation.rows[0].source_descriptor_original_match);
    assert!(observation.rows[0].descriptor_selected);
}

#[test]
fn code_window가_없거나_복수면_exe를_승인하지_않는다() {
    let code_a = code(0x31);
    let code_b = code(0x81);
    let identity_a = identity(0x11);
    let identity_b = identity(0x91);
    let raw_a = raw(3);
    let raw_b = raw(17);
    let make_sources = || {
        [
            source("F1", EXE_A, &raw_a, &identity_a, &code_a, 0x8003_0000),
            source("F2", EXE_B, &raw_b, &identity_b, &code_b, 0x8003_1000),
        ]
    };
    let mut ram = vec![0; PSX_MAIN_RAM_LEN];
    let vram = vec![0; VRAM_BYTE_LEN];
    assert!(matches!(
        inspect_pending_btx_runtime(&ram, &vram, make_sources()),
        Err(PendingBtxRuntimeError::NoMatchingExecutable)
    ));
    for (runtime, value) in [(CODE_A_RUNTIME, &code_a), (CODE_B_RUNTIME, &code_b)] {
        let offset = psx_main_ram_offset(runtime, value.len()).unwrap();
        ram[offset..offset + value.len()].copy_from_slice(value);
    }
    assert!(matches!(
        inspect_pending_btx_runtime(&ram, &vram, make_sources()),
        Err(PendingBtxRuntimeError::AmbiguousMatchingExecutables(_))
    ));
}

#[test]
fn ram_vram_digest와물리source분모를_실패폐쇄한다() {
    let code_a = code(0x31);
    let identity_a = identity(0x11);
    let raw_a = raw(3);
    let source_a = source("F1", EXE_A, &raw_a, &identity_a, &code_a, 0x8003_0000);
    let ram = vec![0; PSX_MAIN_RAM_LEN];
    let vram = vec![0; VRAM_BYTE_LEN];
    assert!(matches!(
        inspect_pending_btx_runtime(&ram[..1], &vram, [source_a]),
        Err(PendingBtxRuntimeError::WrongRamLength { .. })
    ));
    assert!(matches!(
        inspect_pending_btx_runtime(&ram, &vram[..1], [source_a]),
        Err(PendingBtxRuntimeError::WrongVramLength { .. })
    ));
    assert!(matches!(
        inspect_pending_btx_runtime(&ram, &vram, [source_a, source_a]),
        Err(PendingBtxRuntimeError::DuplicatePhysicalSource { .. })
    ));
    let mut bad = source_a;
    bad.expected_sha256 = "00";
    assert!(matches!(
        inspect_pending_btx_runtime(&ram, &vram, [bad]),
        Err(PendingBtxRuntimeError::InvalidExpectedDigest { .. })
    ));
    let mut bad = source_a;
    bad.descriptor_entry_runtime = 0x8020_0000;
    assert!(matches!(
        inspect_pending_btx_runtime(&ram, &vram, [bad]),
        Err(PendingBtxRuntimeError::CoordinateOutOfRange(0x8020_0000))
    ));
}
