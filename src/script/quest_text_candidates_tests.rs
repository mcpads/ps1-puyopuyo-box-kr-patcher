use super::*;
use crate::text::encode_sjis;

fn code_table_with(text: &str) -> QuestCodeTable {
    let required = text
        .chars()
        .flat_map(|character| {
            let encoded = encode_sjis(&character.to_string()).unwrap();
            [[encoded[0], encoded[1]]]
        })
        .collect::<BTreeSet<_>>();
    let mut codes = required.into_iter().collect::<Vec<_>>();
    for high in 0u8..=u8::MAX {
        for low in 0u8..=u8::MAX {
            let pair = [high, low];
            if !codes.contains(&pair) {
                codes.push(pair);
            }
            if codes.len() == crate::font::QUEST_CODE_TABLE_COUNT {
                let raw = codes.into_iter().flatten().collect::<Vec<_>>();
                return QuestCodeTable::parse(&raw).unwrap();
            }
        }
    }
    unreachable!("345개 고유 code를 만들 수 있음")
}

#[test]
fn 코드표에_있는_엄격한_sjis_run만_후보로_만든다() {
    let first = "アルル「";
    let second = "やった～！";
    let table = code_table_with(&format!("{first}{second}"));
    let mut raw = encode_sjis(first).unwrap();
    raw.extend_from_slice(&[0x0B, 0xFF, 0x00, 0x00]);
    let second_offset = raw.len();
    raw.extend_from_slice(&encode_sjis(second).unwrap());
    raw.extend_from_slice(&[0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE]);

    let candidates = scan_quest_code_table_text_runs(&raw, &table);

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].source, first);
    assert_eq!(candidates[1].offset, second_offset);
    assert_eq!(candidates[1].source, second);
}

#[test]
fn 세_code_unit_미만과_일본어가_없는_run은_제외한다() {
    let table = code_table_with("あいＡＢＣ");
    let mut raw = encode_sjis("あい").unwrap();
    raw.extend_from_slice(&[0x00, 0x00]);
    raw.extend_from_slice(&encode_sjis("ＡＢＣ").unwrap());

    assert!(scan_quest_code_table_text_runs(&raw, &table).is_empty());
}

#[test]
fn tsv는_offset_id와_증거를_결정적으로_보존한다() {
    let analysis = Town00TextCandidateAnalysis {
        candidates: vec![Town00TextCandidate {
            offset: 0x1B78,
            byte_len: 10,
            source: "やった～！".to_string(),
            japanese_code_units: 4,
            evidence: Town00TextCandidateEvidence::RuntimeObservedSpan,
        }],
        runtime_observed_spans: 1,
        static_only_candidates: 0,
        source_code_units: 5,
        unique_source_characters: 5,
    };
    let tsv = render_town00_text_candidates_tsv(&analysis);
    assert_eq!(tsv.lines().count(), 2);
    assert!(tsv.contains(
        "TOWN00-TEXT-0x1B78\t0x1B78\t0x8019B5E8\t10\t5\t4\truntime-observed-span\tやった～！"
    ));
    assert!(!tsv.contains('\0'));
}
