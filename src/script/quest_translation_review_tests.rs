use super::*;

fn review_input() -> String {
    format!(
        "{REVIEW_HEADER}\n\
         {QUEST_TRANSLATION_REVIEW_ASSET_SCHEMA_VERSION}\tbatch\tcontext\tstream-agree\t\
         原文一\t초안 하나\t초안 하나\treview-agrees\t\t동일함을 확인함\n\
         {QUEST_TRANSLATION_REVIEW_ASSET_SCHEMA_VERSION}\tbatch\tcontext\tstream-proposal\t\
         原文二\t초안 둘\t검토안 둘\treview-proposes-change\t\t수정 근거를 확인함\n\
         {QUEST_TRANSLATION_REVIEW_ASSET_SCHEMA_VERSION}\tbatch\tcontext\tstream-human\t\
         原文三\t초안 셋\t검토안 셋\tneeds-human-decision\tdecision.voice\t사람 판단이 필요함\n"
    )
}

#[test]
fn 세_검토상태를_각자의_내용계약으로_읽는다() {
    let input = review_input();
    let entries = parse_review_baseline("batch", &input).unwrap();

    assert_eq!(
        entries.iter().map(|entry| entry.status).collect::<Vec<_>>(),
        vec![
            QuestTranslationIndependentReviewStatus::ReviewAgrees,
            QuestTranslationIndependentReviewStatus::ReviewProposesChange,
            QuestTranslationIndependentReviewStatus::NeedsHumanDecision,
        ]
    );
    assert!(entries
        .iter()
        .all(|entry| entry.review_asset_sha256 == sha256(input.as_bytes())));
}

#[test]
fn 검토상태와_내용_관계가_어긋나면_거부한다() {
    let agrees_with_change = format!(
        "{REVIEW_HEADER}\n\
         {QUEST_TRANSLATION_REVIEW_ASSET_SCHEMA_VERSION}\tbatch\thash\tstream\t\
         原文\t초안입니다\t수정안입니다\treview-agrees\t\t검토 메모\n"
    );
    assert!(parse_review_baseline("batch", &agrees_with_change)
        .unwrap_err()
        .to_string()
        .contains("review-agrees"));

    let human_without_dependency = format!(
        "{REVIEW_HEADER}\n\
         {QUEST_TRANSLATION_REVIEW_ASSET_SCHEMA_VERSION}\tbatch\thash\tstream\t\
         原文\t초안입니다\t초안입니다\tneeds-human-decision\t\t검토 메모\n"
    );
    assert!(parse_review_baseline("batch", &human_without_dependency)
        .unwrap_err()
        .to_string()
        .contains("dependency 누락"));
}

#[test]
fn 의미문자가_없는_발화는_승인대기_placeholder와_문장부호만_허용한다() {
    assert!(is_complete_korean_review_candidate(
        "スケルトンＴ「・・・・・・・・・・・",
        "{quest.character.skeleton_t}「..........."
    ));
    assert!(is_complete_korean_review_candidate("・・・", "…"));
    assert!(!is_complete_korean_review_candidate(
        "意味のある原文",
        "..........."
    ));
    assert!(!is_complete_korean_review_candidate(
        "　＊「・・・・",
        "..........."
    ));
    assert!(!is_complete_korean_review_candidate(
        "スケルトンＴ「・・・・・・・・・・・",
        "{quest.character.skeleton_t}「silence"
    ));
    assert!(!is_complete_korean_review_candidate(
        "スケルトンＴ「・・・・・・・・・・・",
        "{quest.character.skeleton_t「..........."
    ));
    assert!(!is_complete_korean_review_candidate("原文", "한글 {.}"));
    assert!(!is_complete_korean_review_candidate(
        "原文",
        "한글 {quest..character}"
    ));
}

#[test]
fn 검토자산의_batch와_행의_batch가_다르면_거부한다() {
    let error = parse_review_baseline("another-batch", &review_input()).unwrap_err();

    assert!(error.to_string().contains("batch 불일치"));
}

#[test]
fn 검토자산_hash는_고정상수가_아니라_현재입력에서_계산한다() {
    let before = review_input();
    let after = before.replacen("동일함을 확인함", "동일함을 다시 확인함", 1);
    let before_hash = &parse_review_baseline("batch", &before).unwrap()[0].review_asset_sha256;
    let after_hash = &parse_review_baseline("batch", &after).unwrap()[0].review_asset_sha256;

    assert_ne!(before_hash, after_hash);
    assert_eq!(after_hash, &sha256(after.as_bytes()));
}

#[test]
fn 기존_독립검토는_확장된_번역후보의_부분집합이어야한다() {
    let reviewed = BTreeSet::from(["existing-a", "existing-b"]);
    let expanded = BTreeSet::from(["existing-a", "existing-b", "new-draft"]);
    assert_eq!(verify_review_coverage(&reviewed, &expanded), Ok(()));

    let drifted = BTreeSet::from(["existing-a", "removed"]);
    assert!(verify_review_coverage(&drifted, &expanded).is_err());
}

#[test]
fn 한_검토자산의_stream_id중복을_거부한다() {
    let duplicated = review_input().replace("stream-proposal", "stream-agree");

    assert!(parse_review_baseline("batch", &duplicated)
        .unwrap_err()
        .to_string()
        .contains("stream ID 중복"));
}
