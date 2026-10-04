//! QUEST에서 runtime으로 확인한 일본어 대사 표본의 번역 검토 ledger.
//!
//! 이 ledger는 번역 모집단 전수를 주장하지 않는다. 현재 화면과 원본
//! `TOWN00.SEQ` span을 함께 확인한 패배·승리 대사와 아이템 획득 표본만
//! 보호하고, 사람 승인을 받은 패배 PoC와 미승인 문장을 분리한다. 영문은
//! 넣지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::text::encode_sjis;

use super::{verify_town00_seq_roundtrip, TOWN00_SEQ_SHA256};

pub const QUEST_DEFEAT_DIALOGUE_ID: &str = "TOWN00-DLG-DEFEAT-0001";
pub const QUEST_VICTORY_DIALOGUE_ID: &str = "TOWN00-DLG-VICTORY-0001";
pub const QUEST_DEFEAT_REWARD_ID: &str = "TOWN00-EVENT-DEFEAT-REWARD-0001";
pub const QUEST_DIALOGUE_REVIEW_SCHEMA_VERSION: &str = "quest-dialogue-review-v1";

const DEFEAT_TRANSLATION: &str =
    include_str!("../../assets/translations/poc/quest_first_dialogue.txt");

/// 대사 하나를 구성하는 연속 Shift-JIS glyph span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestDialogueSpan {
    pub offset: usize,
    pub protected_source: &'static str,
}

/// 사용자 진행에서 관측한 분기.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestDialogueBranch {
    PlayerDefeat,
    PlayerVictory,
    PlayerDefeatReward,
}

impl QuestDialogueBranch {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PlayerDefeat => "player-defeat",
            Self::PlayerVictory => "player-victory",
            Self::PlayerDefeatReward => "player-defeat-reward",
        }
    }
}

/// 번역문이 현재 얻은 사람 승인 범위.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestDialogueTranslationStatus {
    PendingHumanTranslation,
    PocApproved,
}

impl QuestDialogueTranslationStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PendingHumanTranslation => "pending-human-translation",
            Self::PocApproved => "poc-approved",
        }
    }
}

/// 화면 증거와 대응 state를 함께 보존하는 runtime 표본.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestDialogueRuntimeEvidence {
    pub screenshot_sha256: &'static str,
    pub state_sha256: &'static str,
    /// 화면을 얻은 뒤 저장한 state에서 관측한 stream offset.
    ///
    /// 대사 그 자체의 시작을 뜻하지 않으며, 재실행 기준점의 실행 상태다.
    pub saved_stream_offset: usize,
}

/// 보호 원문 span과 번역 상태를 묶은 QUEST 대사 항목.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestDialogueTranslationEntry {
    pub id: &'static str,
    pub branch: QuestDialogueBranch,
    pub protected_display: &'static str,
    pub spans: &'static [QuestDialogueSpan],
    pub status: QuestDialogueTranslationStatus,
    pub runtime: QuestDialogueRuntimeEvidence,
    translation_asset: Option<&'static str>,
}

/// manifest 검증을 통과해 PoC 합성에 사용할 수 있는 항목.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestApprovedPocTranslation {
    pub id: &'static str,
    pub text: &'static str,
    pub spans: &'static [QuestDialogueSpan],
}

pub const QUEST_DEFEAT_DIALOGUE_SPANS: [QuestDialogueSpan; 3] = [
    QuestDialogueSpan {
        offset: 0x21E4,
        protected_source: "ドラコ「",
    },
    QuestDialogueSpan {
        offset: 0x21F4,
        protected_source: "やった～！",
    },
    QuestDialogueSpan {
        offset: 0x2208,
        protected_source: "　　　　あたしの勝ちね！！",
    },
];

pub const QUEST_VICTORY_DIALOGUE_SPANS: [QuestDialogueSpan; 3] = [
    QuestDialogueSpan {
        offset: 0x1B68,
        protected_source: "アルル「",
    },
    QuestDialogueSpan {
        offset: 0x1B78,
        protected_source: "やった～！",
    },
    QuestDialogueSpan {
        offset: 0x1B8A,
        protected_source: "ボクの勝ちだ～！！",
    },
];

pub const QUEST_DEFEAT_REWARD_SPANS: [QuestDialogueSpan; 2] = [
    QuestDialogueSpan {
        offset: 0x22E8,
        protected_source: "　　ドラコはさいごの　１　個の",
    },
    QuestDialogueSpan {
        offset: 0x2308,
        protected_source: "　　特製ぷよまん　を手に入れた！",
    },
];

/// runtime과 원본 span을 함께 확인한 현재 표본.
///
/// `PocApproved`는 기술 PoC 입력 자격만 뜻하며 배포 적격이 아니다.
pub const QUEST_DIALOGUE_TRANSLATIONS: [QuestDialogueTranslationEntry; 3] = [
    QuestDialogueTranslationEntry {
        id: QUEST_DEFEAT_DIALOGUE_ID,
        branch: QuestDialogueBranch::PlayerDefeat,
        protected_display: "ドラコ「やった～！\n　　　　あたしの勝ちね！！",
        spans: &QUEST_DEFEAT_DIALOGUE_SPANS,
        status: QuestDialogueTranslationStatus::PocApproved,
        runtime: QuestDialogueRuntimeEvidence {
            screenshot_sha256: "7e5ac735fccd59ad0484bcfb20eb9c42a60746281c1bbbaa263df55d0e9091d2",
            state_sha256: "a1f172ae628b9dce0cd7012d9cae39485982a4e42f71a385378f0629b5f80083",
            saved_stream_offset: 0x2260,
        },
        translation_asset: Some(DEFEAT_TRANSLATION),
    },
    QuestDialogueTranslationEntry {
        id: QUEST_VICTORY_DIALOGUE_ID,
        branch: QuestDialogueBranch::PlayerVictory,
        protected_display: "アルル「やった～！ボクの勝ちだ～！！",
        spans: &QUEST_VICTORY_DIALOGUE_SPANS,
        status: QuestDialogueTranslationStatus::PendingHumanTranslation,
        runtime: QuestDialogueRuntimeEvidence {
            screenshot_sha256: "b1eb1c426c4f6fb1d1e7891c9269f79cf7819fa225052bc6553fbea0ec455549",
            state_sha256: "616c8107d294afb18e6721f3d7f6961701bdeefcdd8fb15d5c17f8130d72bc5c",
            saved_stream_offset: 0x1BA0,
        },
        translation_asset: None,
    },
    QuestDialogueTranslationEntry {
        id: QUEST_DEFEAT_REWARD_ID,
        branch: QuestDialogueBranch::PlayerDefeatReward,
        protected_display: "　　ドラコはさいごの　１　個の\n　　特製ぷよまん　を手に入れた！",
        spans: &QUEST_DEFEAT_REWARD_SPANS,
        status: QuestDialogueTranslationStatus::PendingHumanTranslation,
        runtime: QuestDialogueRuntimeEvidence {
            screenshot_sha256: "f1b650c6699a98673e3dd83a34cb40608bb5960e4acfb77971b116aed2fad650",
            state_sha256: "dfc1cbdac0897ca437d5b3a90c25cb18f31c8e33c01f337076d286833d5eb5af",
            saved_stream_offset: 0x232C,
        },
        translation_asset: None,
    },
];

/// 현재 runtime 확인 표본과 번역 상태의 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestDialogueTranslationReport {
    pub observed_japanese_samples: usize,
    pub poc_approved: usize,
    pub pending_human_translation: usize,
}

/// QUEST 대사 ledger 검증 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDialogueTranslationError(String);

impl fmt::Display for QuestDialogueTranslationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestDialogueTranslationError {}

fn error(message: impl Into<String>) -> QuestDialogueTranslationError {
    QuestDialogueTranslationError(message.into())
}

fn has_japanese_character(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
        )
    })
}

fn has_hangul_syllable(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn normalized_translation(
    entry: QuestDialogueTranslationEntry,
) -> Result<Option<&'static str>, QuestDialogueTranslationError> {
    match (entry.status, entry.translation_asset) {
        (QuestDialogueTranslationStatus::PendingHumanTranslation, None) => Ok(None),
        (QuestDialogueTranslationStatus::PocApproved, Some(raw)) => {
            let Some(text) = raw.strip_suffix('\n') else {
                return Err(error(format!(
                    "{}: 번역 asset은 LF 하나로 끝나야 함",
                    entry.id
                )));
            };
            if text.is_empty()
                || text.contains('\r')
                || text.trim() != text
                || !has_hangul_syllable(text)
            {
                return Err(error(format!(
                    "{}: 번역 asset의 LF·공백·한글 계약 불일치",
                    entry.id
                )));
            }
            let expected_lines = entry.protected_display.lines().count();
            let actual_lines = text.lines().count();
            if actual_lines != expected_lines {
                return Err(error(format!(
                    "{}: 원문과 번역문 행 수 불일치: expected={}, actual={}",
                    entry.id, expected_lines, actual_lines
                )));
            }
            Ok(Some(text))
        }
        _ => Err(error(format!(
            "{}: 번역 상태와 translation asset 유무가 맞지 않음",
            entry.id
        ))),
    }
}

fn verify_entries(
    raw: &[u8],
    entries: &[QuestDialogueTranslationEntry],
) -> Result<QuestDialogueTranslationReport, QuestDialogueTranslationError> {
    if entries.is_empty() {
        return Err(error("QUEST 대사 translation ledger가 비어 있음"));
    }

    let mut ids = BTreeSet::new();
    let mut ranges = Vec::new();
    let mut report = QuestDialogueTranslationReport {
        observed_japanese_samples: 0,
        poc_approved: 0,
        pending_human_translation: 0,
    };

    for entry in entries {
        if !ids.insert(entry.id) {
            return Err(error(format!("QUEST 대사 ID 중복: {}", entry.id)));
        }
        if entry.spans.is_empty() {
            return Err(error(format!("{}: 보호 span이 없음", entry.id)));
        }
        if !has_japanese_character(entry.protected_display) {
            return Err(error(format!("{}: 일본어 번역 표본이 아님", entry.id)));
        }
        if !valid_sha256(entry.runtime.screenshot_sha256)
            || !valid_sha256(entry.runtime.state_sha256)
        {
            return Err(error(format!(
                "{}: runtime 증거 SHA-256 형식 오류",
                entry.id
            )));
        }
        if entry.runtime.saved_stream_offset >= raw.len() {
            return Err(error(format!(
                "{}: saved stream offset이 TOWN00 범위 밖임",
                entry.id
            )));
        }

        let mut joined_source = String::new();
        for span in entry.spans {
            if span.protected_source.is_empty() {
                return Err(error(format!("{}: 빈 보호 span", entry.id)));
            }
            let expected = encode_sjis(span.protected_source).map_err(|source| {
                error(format!("{}: 보호 원문 인코딩 실패: {source}", entry.id))
            })?;
            let end = span
                .offset
                .checked_add(expected.len())
                .ok_or_else(|| error(format!("{}: 보호 span 범위 overflow", entry.id)))?;
            let actual = raw
                .get(span.offset..end)
                .ok_or_else(|| error(format!("{}: 보호 span이 TOWN00 범위 밖임", entry.id)))?;
            if actual != expected {
                return Err(error(format!(
                    "{}: 보호 원문 바이트 불일치 at 0x{:X}",
                    entry.id, span.offset
                )));
            }
            ranges.push((span.offset, end, entry.id));
            joined_source.push_str(span.protected_source);
        }
        if joined_source != entry.protected_display.replace('\n', "") {
            return Err(error(format!(
                "{}: 보호 span과 표시 원문 연결 불일치",
                entry.id
            )));
        }

        normalized_translation(*entry)?;
        report.observed_japanese_samples += 1;
        match entry.status {
            QuestDialogueTranslationStatus::PocApproved => report.poc_approved += 1,
            QuestDialogueTranslationStatus::PendingHumanTranslation => {
                report.pending_human_translation += 1;
            }
        }
    }

    ranges.sort_unstable_by_key(|range| range.0);
    for pair in ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(error(format!(
                "QUEST 대사 보호 span 겹침: {}와 {}",
                pair[0].2, pair[1].2
            )));
        }
    }
    Ok(report)
}

/// 지원 원본 `TOWN00.SEQ`와 현재 runtime 표본 ledger를 함께 검증한다.
pub fn verify_quest_dialogue_translation_manifest(
    raw: &[u8],
) -> Result<QuestDialogueTranslationReport, QuestDialogueTranslationError> {
    verify_town00_seq_roundtrip(raw)
        .map_err(|source| error(format!("QUEST 대사 원본 계약 실패: {source}")))?;
    verify_entries(raw, &QUEST_DIALOGUE_TRANSLATIONS)
}

/// 원본 span까지 검증한 사람 승인 PoC 번역을 반환한다.
pub fn approved_quest_poc_translation(
    raw: &[u8],
    id: &str,
) -> Result<QuestApprovedPocTranslation, QuestDialogueTranslationError> {
    verify_quest_dialogue_translation_manifest(raw)?;
    let entry = QUEST_DIALOGUE_TRANSLATIONS
        .iter()
        .copied()
        .find(|entry| entry.id == id)
        .ok_or_else(|| error(format!("QUEST 대사 ledger에 없는 ID: {id}")))?;
    let text = normalized_translation(entry)?.ok_or_else(|| {
        error(format!(
            "{id}: 사람 승인 전이라 PoC 입력으로 사용할 수 없음"
        ))
    })?;
    Ok(QuestApprovedPocTranslation {
        id: entry.id,
        text,
        spans: entry.spans,
    })
}

fn escape_tsv_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn render_review_tsv_from(
    raw: &[u8],
    entries: &[QuestDialogueTranslationEntry],
    asset_name: &str,
    asset_sha256: &str,
) -> Result<String, QuestDialogueTranslationError> {
    if asset_name.is_empty() || !valid_sha256(asset_sha256) {
        return Err(error("QUEST 대사 review 자산 식별자 형식 오류"));
    }
    verify_entries(raw, entries)?;
    let mut output = String::from(
        "schema_version\tasset_name\tasset_sha256\tid\tbranch\tstatus\t\
         spans\tprotected_source\tko\tscreenshot_sha256\tstate_sha256\t\
         saved_stream_offset\n",
    );
    for &entry in entries {
        let spans = entry
            .spans
            .iter()
            .map(|span| {
                let bytes = encode_sjis(span.protected_source)
                    .expect("검증을 통과한 보호 원문은 Shift-JIS로 인코딩된다");
                format!("0x{:X}+{}", span.offset, bytes.len())
            })
            .collect::<Vec<_>>()
            .join(",");
        let translation = normalized_translation(entry)?.unwrap_or_default();
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:X}\n",
            QUEST_DIALOGUE_REVIEW_SCHEMA_VERSION,
            asset_name,
            asset_sha256,
            entry.id,
            entry.branch.label(),
            entry.status.label(),
            spans,
            escape_tsv_field(entry.protected_display),
            escape_tsv_field(translation),
            entry.runtime.screenshot_sha256,
            entry.runtime.state_sha256,
            entry.runtime.saved_stream_offset,
        ));
    }
    Ok(output)
}

/// runtime 표본 ledger를 사람이 검토할 수 있는 결정적 TSV로 만든다.
///
/// 이 파생물은 제품 입력이 아니며 다시 읽는 경로도 없다.
pub fn render_quest_dialogue_review_tsv(
    raw: &[u8],
) -> Result<String, QuestDialogueTranslationError> {
    verify_quest_dialogue_translation_manifest(raw)?;
    render_review_tsv_from(
        raw,
        &QUEST_DIALOGUE_TRANSLATIONS,
        "TOWN00.SEQ",
        TOWN00_SEQ_SHA256,
    )
}

#[cfg(test)]
#[path = "quest_translation_tests.rs"]
mod tests;
