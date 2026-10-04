use super::*;

const VALID_SCREEN_SHA256: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const VALID_STATE_SHA256: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const TEST_ASSET_SHA256: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const APPROVED_TRANSLATION: &str = "가\n나\n";
const APPROVED_SPANS: [QuestDialogueSpan; 2] = [
    QuestDialogueSpan {
        offset: 0,
        protected_source: "あ",
    },
    QuestDialogueSpan {
        offset: 2,
        protected_source: "い",
    },
];
const PENDING_SPANS: [QuestDialogueSpan; 1] = [QuestDialogueSpan {
    offset: 4,
    protected_source: "う",
}];
const TEST_ENTRIES: [QuestDialogueTranslationEntry; 2] = [
    QuestDialogueTranslationEntry {
        id: "TEST-APPROVED",
        branch: QuestDialogueBranch::PlayerDefeat,
        protected_display: "あ\nい",
        spans: &APPROVED_SPANS,
        status: QuestDialogueTranslationStatus::PocApproved,
        runtime: QuestDialogueRuntimeEvidence {
            screenshot_sha256: VALID_SCREEN_SHA256,
            state_sha256: VALID_STATE_SHA256,
            saved_stream_offset: 1,
        },
        translation_asset: Some(APPROVED_TRANSLATION),
    },
    QuestDialogueTranslationEntry {
        id: "TEST-PENDING",
        branch: QuestDialogueBranch::PlayerVictory,
        protected_display: "う",
        spans: &PENDING_SPANS,
        status: QuestDialogueTranslationStatus::PendingHumanTranslation,
        runtime: QuestDialogueRuntimeEvidence {
            screenshot_sha256: VALID_SCREEN_SHA256,
            state_sha256: VALID_STATE_SHA256,
            saved_stream_offset: 5,
        },
        translation_asset: None,
    },
];

fn ledger_fixture(entries: &[QuestDialogueTranslationEntry]) -> Vec<u8> {
    let required_len = entries
        .iter()
        .flat_map(|entry| entry.spans)
        .map(|span| span.offset + encode_sjis(span.protected_source).unwrap().len())
        .max()
        .unwrap_or_default();
    let mut raw = vec![0u8; required_len + 1];
    for entry in entries {
        for span in entry.spans {
            let encoded = encode_sjis(span.protected_source).unwrap();
            raw[span.offset..span.offset + encoded.len()].copy_from_slice(&encoded);
        }
    }
    raw
}

#[test]
fn 합성_표본의_승인과_대기상태를_입력에서_집계한다() {
    assert_eq!(
        verify_entries(&ledger_fixture(&TEST_ENTRIES), &TEST_ENTRIES),
        Ok(QuestDialogueTranslationReport {
            observed_japanese_samples: 2,
            poc_approved: 1,
            pending_human_translation: 1,
        })
    );
    assert_eq!(normalized_translation(TEST_ENTRIES[0]), Ok(Some("가\n나")));
    assert_eq!(normalized_translation(TEST_ENTRIES[1]), Ok(None));
}

#[test]
fn 합성_보호원문_drift를_거부한다() {
    let mut raw = ledger_fixture(&TEST_ENTRIES);
    raw[APPROVED_SPANS[1].offset] ^= 1;
    assert!(verify_entries(&raw, &TEST_ENTRIES)
        .unwrap_err()
        .to_string()
        .contains("보호 원문 바이트 불일치"));
}

#[test]
fn 번역상태와_asset의_불일치를_거부한다() {
    let mut entries = TEST_ENTRIES;
    entries[1].translation_asset = Some("다\n");
    assert!(verify_entries(&ledger_fixture(&entries), &entries)
        .unwrap_err()
        .to_string()
        .contains("상태와 translation asset"));
}

#[test]
fn 보호_span은_겹칠_수_없다() {
    const OVERLAPPING: [QuestDialogueSpan; 2] = [
        QuestDialogueSpan {
            offset: 0,
            protected_source: "あ",
        },
        QuestDialogueSpan {
            offset: 0,
            protected_source: "あ",
        },
    ];
    let mut entries = TEST_ENTRIES;
    entries[0].spans = &OVERLAPPING;
    entries[0].protected_display = "ああ";
    entries[0].status = QuestDialogueTranslationStatus::PendingHumanTranslation;
    entries[0].translation_asset = None;
    assert!(verify_entries(&ledger_fixture(&entries), &entries)
        .unwrap_err()
        .to_string()
        .contains("보호 span 겹침"));
}

#[test]
fn review_tsv는_호출자가_준_현재자산식별자와_입력상태를_쓴다() {
    let raw = ledger_fixture(&TEST_ENTRIES);
    let output =
        render_review_tsv_from(&raw, &TEST_ENTRIES, "TEST.SEQ", TEST_ASSET_SHA256).unwrap();
    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    assert!(lines[1].contains("TEST.SEQ\tcccccccc"));
    assert!(lines[1].contains("TEST-APPROVED\tplayer-defeat\tpoc-approved"));
    assert!(lines[1].contains("\t가\\n나\t"));
    assert!(lines[2].contains("TEST-PENDING\tplayer-victory\tpending-human-translation"));
}
