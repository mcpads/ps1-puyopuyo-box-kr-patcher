use crate::coords::LogicalFile;

use super::*;

fn map(name: &'static str, text_len: usize) -> &'static ExeMap {
    Box::leak(Box::new(ExeMap {
        name,
        load_addr: 0x8001_0000,
        text_len: text_len as u32,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, (text_len + 0x800) as u32),
    }))
}

#[test]
fn 기존_가나_휴리스틱의_길이와_경계를_그대로_지킨다() {
    let raw = [
        0x00, 0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0x00, // あいう
        0x83, 0x41, 0x83, 0x43, 0x00, // アイ: 2자는 제외
        0x88, 0xEA, 0x8A, 0xBF, 0x8E, 0x9A, 0x00, // 한자만: 제외
    ];
    let candidates = scan_kana_runs(&raw, MIN_CODE_UNITS);

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].offset, 1);
    assert_eq!(candidates[0].raw, &raw[1..7]);
    assert_eq!(candidates[0].hiragana, 3);
    assert_eq!(candidates[0].katakana, 0);
}

#[test]
fn heuristic_pair지만_mapping이_없으면_엄격한_decode와_분리한다() {
    let mut text = vec![0u8; 24];
    text[16..22].copy_from_slice(&[0x81, 0xAD, 0x82, 0xA0, 0x82, 0xA2]);
    let analysis = audit_exe_string_candidates([(map("INVALID.EXE", 24), text.as_slice())])
        .expect("휴리스틱 후보 자체는 보존해야 한다");

    assert_eq!(analysis.candidates.len(), 1);
    assert!(analysis.candidates[0].decoded.is_none());
    assert_eq!(analysis.summaries[0].heuristic_only_candidates, 1);
}

#[test]
fn 정적_참조와_엔트리_추적_포함을_runtime_판정과_분리한다() {
    let words = [
        0x3C08_8001u32, // lui t0, 0x8001
        0x2508_0010u32, // addiu t0, t0, 0x10
        0x03E0_0008u32, // jr ra
        0x0000_0000u32, // nop
    ];
    let mut text = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    text.extend_from_slice(&[0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0, 0]);
    let analysis =
        audit_exe_string_candidates([(map("REF.EXE", text.len()), text.as_slice())]).unwrap();
    let candidate = &analysis.candidates[0];

    assert_eq!(candidate.runtime_addr, 0x8001_0010);
    assert_eq!(candidate.static_ref_sites, [0x8001_0000]);
    assert_eq!(candidate.entry_reached_ref_sites, [0x8001_0000]);
    assert!(!candidate.overlaps_entry_code);
    assert_eq!(candidate.evidence(), "entry_trace_static_ref");
}

#[test]
fn 후보_바이트가_엔트리_코드와_겹치면_별도_경고한다() {
    let mut text = vec![0u8; 16];
    text[0..8].copy_from_slice(&[0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0x08, 0x00]);
    let analysis = audit_exe_string_candidates([(map("CODE.EXE", 16), text.as_slice())]).unwrap();

    assert!(analysis.candidates[0].overlaps_entry_code);
    assert_eq!(analysis.candidates[0].evidence(), "entry_code_overlap");
}

#[test]
fn report는_후보와_runtime_미판정을_결정적으로_노출한다() {
    let mut text = vec![0u8; 24];
    text[16..22].copy_from_slice(&[0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4]);
    let analysis = audit_exe_string_candidates([(map("REPORT.EXE", 24), text.as_slice())]).unwrap();
    let report = render_exe_string_candidates_tsv(&analysis);

    assert!(report.starts_with("candidate_id\texe\ttext_offset\truntime_addr"));
    assert!(report.contains("REPORT.EXE:000010"));
    assert!(report.contains("\tunverified\t"));
    assert!(report.ends_with("\tあいう\n"));
}

#[test]
fn 빈_입력_중복_길이_drift를_거부한다() {
    assert_eq!(
        audit_exe_string_candidates(std::iter::empty()),
        Err(ExeStringError::EmptyInput)
    );

    let duplicate_map = map("DUP.EXE", 4);
    let text = [0u8; 4];
    assert_eq!(
        audit_exe_string_candidates([
            (duplicate_map, text.as_slice()),
            (duplicate_map, text.as_slice())
        ]),
        Err(ExeStringError::DuplicateExecutable("DUP.EXE"))
    );

    let wrong = map("WRONG.EXE", 8);
    assert!(matches!(
        audit_exe_string_candidates([(wrong, text.as_slice())]),
        Err(ExeStringError::TextLengthMismatch { .. })
    ));
}
