//! QUEST 번역 후보를 장면 문맥 단위의 작업 배치로 묶는다.
//!
//! manifest는 장면·분기·화자·순서를, work TSV는 `ko/status/notes`만
//! 소유한다. 원문·raw stream·control signature는 현재 원본에서 매번
//! 다시 결합하며, 추출·manifest·용어·문체 baseline이 하나라도 drift하면
//! 작업물을 거부한다. 초벌 번역과 사람 승인, build eligibility는 서로
//! 다른 상태이며 이 모듈은 아직 번역문을 제품 입력으로 내보내지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::quest_seq_text_extraction::{
    serialize_quest_seq_extracted_text_stream, QuestSeqTextExtractionAnalysis,
};
use super::quest_seq_translation_units::{
    render_quest_seq_translation_units_tsv, QuestSeqTranslationUnitAnalysis,
    QuestSeqTranslationUnitPolicy,
};
use super::quest_terminology::{
    quest_terminology_report_sha256, QuestTerminologyCandidateAnalysis, QuestTerminologyDecision,
};

pub const QUEST_TRANSLATION_BATCH_SCHEMA_VERSION: &str = "quest-translation-batches-v2";
pub const QUEST_TRANSLATION_WORK_SCHEMA_VERSION: &str = "quest-translation-work-v2";
const MANIFEST: &str = include_str!("../../assets/translations/quest_batch_manifest.tsv");
const STYLE_CANDIDATES: &str = include_str!("../../assets/translations/quest_style_candidates.tsv");
const WORK_FILES: &[(&str, &str)] = &[
    (
        "quest-kawa-seriri-crying-approach-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-seriri-crying-approach-001.tsv"
        ),
    ),
    (
        "quest-kawa-seriri-friendship-followup-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-seriri-friendship-followup-001.tsv"
        ),
    ),
    (
        "quest-kawa-seriri-medal-choice-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-seriri-medal-choice-001.tsv"
        ),
    ),
    (
        "quest-kawa-seriri-medal-gift-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-seriri-medal-gift-001.tsv"
        ),
    ),
    (
        "quest-kawa-seriri-misunderstanding-resolution-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-seriri-misunderstanding-resolution-001.tsv"
        ),
    ),
    (
        "quest-kawa-seriri-waiting-ending-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-seriri-waiting-ending-001.tsv"
        ),
    ),
    (
        "quest-kawa-suketoudara-accusation-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-suketoudara-accusation-001.tsv"
        ),
    ),
    (
        "quest-kawa-suketoudara-first-meeting-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-suketoudara-first-meeting-001.tsv"
        ),
    ),
    (
        "quest-kawa-suketoudara-medal-aftermath-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-suketoudara-medal-aftermath-001.tsv"
        ),
    ),
    (
        "quest-kawa-taraxi-eastbound-service-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-taraxi-eastbound-service-001.tsv"
        ),
    ),
    (
        "quest-kawa-taraxi-first-crossing-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-taraxi-first-crossing-001.tsv"
        ),
    ),
    (
        "quest-kawa-taraxi-signs-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-taraxi-signs-001.tsv"
        ),
    ),
    (
        "quest-kawa-taraxi-westbound-service-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-kawa-taraxi-westbound-service-001.tsv"
        ),
    ),
    (
        "quest-hasi-schezo-aftermath-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-hasi-schezo-aftermath-001.tsv"
        ),
    ),
    (
        "quest-hasi-schezo-discovery-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-hasi-schezo-discovery-001.tsv"
        ),
    ),
    (
        "quest-hasi-schezo-wakeup-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-hasi-schezo-wakeup-001.tsv"
        ),
    ),
    (
        "quest-iseki-deeper-quakes-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-iseki-deeper-quakes-001.tsv"
        ),
    ),
    (
        "quest-iseki-earthquake-route-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-iseki-earthquake-route-001.tsv"
        ),
    ),
    (
        "quest-iseki-minotauros-medal-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-iseki-minotauros-medal-001.tsv"
        ),
    ),
    (
        "quest-iseki-minotauros-training-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-iseki-minotauros-training-001.tsv"
        ),
    ),
    (
        "quest-iseki-six-medals-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-iseki-six-medals-001.tsv"
        ),
    ),
    (
        "quest-iseki-zoh-misunderstanding-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-iseki-zoh-misunderstanding-001.tsv"
        ),
    ),
    (
        "quest-miti-draco-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-miti-draco-rematch-001.tsv"
        ),
    ),
    (
        "quest-miti-medal-rival-hint-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-miti-medal-rival-hint-001.tsv"
        ),
    ),
    (
        "quest-miti-sudden-impact-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-miti-sudden-impact-001.tsv"
        ),
    ),
    (
        "quest-mori-earplug-aftermath-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-mori-earplug-aftermath-001.tsv"
        ),
    ),
    (
        "quest-mori-harpy-first-confrontation-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-mori-harpy-first-confrontation-001.tsv"
        ),
    ),
    (
        "quest-mori-harpy-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-mori-harpy-rematch-001.tsv"
        ),
    ),
    (
        "quest-mori-harpy-song-barrier-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-mori-harpy-song-barrier-001.tsv"
        ),
    ),
    (
        "quest-mori-owlbear-honey-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-mori-owlbear-honey-001.tsv"
        ),
    ),
    (
        "quest-shez-cave-confrontation-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-shez-cave-confrontation-001.tsv"
        ),
    ),
    (
        "quest-shez-cave-inspection-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-shez-cave-inspection-001.tsv"
        ),
    ),
    (
        "quest-shez-hot-spring-signs-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-shez-hot-spring-signs-001.tsv"
        ),
    ),
    (
        "quest-shez-six-medals-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-shez-six-medals-001.tsv"
        ),
    ),
    (
        "quest-shez-wall-notes-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-shez-wall-notes-001.tsv"
        ),
    ),
    (
        "quest-suke-skeleton-tea-confrontation-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-suke-skeleton-tea-confrontation-001.tsv"
        ),
    ),
    (
        "quest-suke-tea-battle-aftermath-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-suke-tea-battle-aftermath-001.tsv"
        ),
    ),
    (
        "quest-suke-tea-scent-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-suke-tea-scent-001.tsv"
        ),
    ),
    (
        "quest-town00-carbuncle-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-carbuncle-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town00-carbuncle-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-carbuncle-rematch-001.tsv"
        ),
    ),
    (
        "quest-town00-draco-puyoman-aftermath-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-draco-puyoman-aftermath-001.tsv"
        ),
    ),
    (
        "quest-town00-house-post-subjects-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-house-post-subjects-001.tsv"
        ),
    ),
    (
        "quest-town00-jumper-battles-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-jumper-battles-001.tsv"
        ),
    ),
    (
        "quest-town00-jumper-boots-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-jumper-boots-001.tsv"
        ),
    ),
    (
        "quest-town00-kikimora-battles-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-kikimora-battles-001.tsv"
        ),
    ),
    (
        "quest-town00-kikimora-cleaning-request-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-kikimora-cleaning-request-001.tsv"
        ),
    ),
    (
        "quest-town00-lagnus-post-messages-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-lagnus-post-messages-001.tsv"
        ),
    ),
    (
        "quest-town00-medal-journey-departure-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-medal-journey-departure-001.tsv"
        ),
    ),
    (
        "quest-town00-medal-legend-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-medal-legend-001.tsv"
        ),
    ),
    (
        "quest-town00-momomo-shop-battles-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-momomo-shop-battles-001.tsv"
        ),
    ),
    (
        "quest-town00-nununun-consolation-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-nununun-consolation-001.tsv"
        ),
    ),
    (
        "quest-town00-plumber-sign-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-plumber-sign-001.tsv"
        ),
    ),
    (
        "quest-town00-puyoman-results-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-puyoman-results-001.tsv"
        ),
    ),
    (
        "quest-town00-samurai-hot-tea-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-samurai-hot-tea-001.tsv"
        ),
    ),
    (
        "quest-town00-skeleton-medal-hint-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town00-skeleton-medal-hint-001.tsv"
        ),
    ),
    (
        "quest-town10-badge-lady-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-badge-lady-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town10-badge-lady-help-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-badge-lady-help-001.tsv"
        ),
    ),
    (
        "quest-town10-badge-lady-rematches-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-badge-lady-rematches-001.tsv"
        ),
    ),
    (
        "quest-town10-cherrybell-sign-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-cherrybell-sign-001.tsv"
        ),
    ),
    (
        "quest-town10-child-play-car-request-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-child-play-car-request-001.tsv"
        ),
    ),
    (
        "quest-town10-incubus-medal-hint-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-incubus-medal-hint-001.tsv"
        ),
    ),
    (
        "quest-town10-lagnus-fans-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-lagnus-fans-001.tsv"
        ),
    ),
    (
        "quest-town10-lagnus-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-lagnus-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town10-lagnus-medal-hints-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-lagnus-medal-hints-001.tsv"
        ),
    ),
    (
        "quest-town10-lagnus-medal-intro-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-lagnus-medal-intro-001.tsv"
        ),
    ),
    (
        "quest-town10-lagnus-post-messages-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-lagnus-post-messages-001.tsv"
        ),
    ),
    (
        "quest-town10-lagnus-post-subjects-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-lagnus-post-subjects-001.tsv"
        ),
    ),
    (
        "quest-town10-lagnus-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-lagnus-rematch-001.tsv"
        ),
    ),
    (
        "quest-town10-lagnus-wall-notes-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-lagnus-wall-notes-001.tsv"
        ),
    ),
    (
        "quest-town10-nohoho-shop-battles-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-nohoho-shop-battles-001.tsv"
        ),
    ),
    (
        "quest-town10-noisy-forest-villagers-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-noisy-forest-villagers-001.tsv"
        ),
    ),
    (
        "quest-town10-panotty-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-panotty-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town10-panotty-flute-ambient-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-panotty-flute-ambient-001.tsv"
        ),
    ),
    (
        "quest-town10-panotty-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-panotty-rematch-001.tsv"
        ),
    ),
    (
        "quest-town10-rulue-medal-scent-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-rulue-medal-scent-001.tsv"
        ),
    ),
    (
        "quest-town10-shadow-puyo-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town10-shadow-puyo-001.tsv"
        ),
    ),
    (
        "quest-town20-child-father-worry-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-child-father-worry-001.tsv"
        ),
    ),
    (
        "quest-town20-child-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-child-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town20-child-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-child-rematch-001.tsv"
        ),
    ),
    (
        "quest-town20-domino-villager-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-domino-villager-001.tsv"
        ),
    ),
    (
        "quest-town20-earthquake-villagers-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-earthquake-villagers-001.tsv"
        ),
    ),
    (
        "quest-town20-fufufu-battles-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-fufufu-battles-001.tsv"
        ),
    ),
    (
        "quest-town20-fufufu-shop-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-fufufu-shop-001.tsv"
        ),
    ),
    (
        "quest-town20-lagnus-post-messages-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-lagnus-post-messages-001.tsv"
        ),
    ),
    (
        "quest-town20-lagnus-post-subjects-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-lagnus-post-subjects-001.tsv"
        ),
    ),
    (
        "quest-town20-sevenlease-sign-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-sevenlease-sign-001.tsv"
        ),
    ),
    (
        "quest-town20-shadow-puyo-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-shadow-puyo-001.tsv"
        ),
    ),
    (
        "quest-town20-sleepy-flirt-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-sleepy-flirt-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town20-sleepy-flirt-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-sleepy-flirt-rematch-001.tsv"
        ),
    ),
    (
        "quest-town20-sleepy-flirt-request-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-sleepy-flirt-request-001.tsv"
        ),
    ),
    (
        "quest-town20-zoh-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-zoh-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town20-zoh-hungry-shop-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-zoh-hungry-shop-001.tsv"
        ),
    ),
    (
        "quest-town20-zoh-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town20-zoh-rematch-001.tsv"
        ),
    ),
    (
        "quest-town30-pakista-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town30-pakista-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town30-pakista-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town30-pakista-rematch-001.tsv"
        ),
    ),
    (
        "quest-town30-pakista-shop-greeting-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town30-pakista-shop-greeting-001.tsv"
        ),
    ),
    (
        "quest-town30-parara-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town30-parara-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town30-parara-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town30-parara-rematch-001.tsv"
        ),
    ),
    (
        "quest-town30-parara-shop-greeting-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town30-parara-shop-greeting-001.tsv"
        ),
    ),
    (
        "quest-town30-sasoriman-first-battle-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town30-sasoriman-first-battle-001.tsv"
        ),
    ),
    (
        "quest-town30-sasoriman-rematch-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town30-sasoriman-rematch-001.tsv"
        ),
    ),
    (
        "quest-town30-sasoriman-shop-greeting-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-town30-sasoriman-shop-greeting-001.tsv"
        ),
    ),
    (
        "quest-tou-feast-ending-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-tou-feast-ending-001.tsv"
        ),
    ),
    (
        "quest-tou-return-home-ending-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-tou-return-home-ending-001.tsv"
        ),
    ),
    (
        "quest-tou-rulue-outlet-confrontation-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-tou-rulue-outlet-confrontation-001.tsv"
        ),
    ),
    (
        "quest-tou-satan-proposal-choice-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-tou-satan-proposal-choice-001.tsv"
        ),
    ),
    (
        "quest-tou-satan-rejection-chase-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-tou-satan-rejection-chase-001.tsv"
        ),
    ),
    (
        "quest-tou-satan-schezo-epilogue-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-tou-satan-schezo-epilogue-001.tsv"
        ),
    ),
    (
        "quest-tou-satan-wedding-reveal-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-tou-satan-wedding-reveal-001.tsv"
        ),
    ),
    (
        "quest-tou-tower-door-open-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-tou-tower-door-open-001.tsv"
        ),
    ),
    (
        "quest-tou-tower-medal-gate-001",
        include_str!(
            "../../assets/translations/scripts/in_progress/quest-tou-tower-medal-gate-001.tsv"
        ),
    ),
];
const MANIFEST_HEADER: &str = "batch_id\tscene_id\tscene_title\tsequence_context\t\
     source_evidence\tscene_order\tbranch\tstream_id\trole\tspeaker_id\tspeaker\t\
     addressee_id\taddressee\tconfidence\tcontext_note";
const STYLE_HEADER: &str = "style_id\tscope\trule\tstatus\tevidence\tnotes";
const WORK_HEADER: &str = "schema_version\tbatch_id\tbatch_context_sha256\t\
     translation_units_sha256\tterminology_sha256\tstyle_sha256\tstream_id\t\
     protected_source\tko\tstatus\tnotes";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTranslationBatchRole {
    Translate,
    ProtectedContextOnly,
}

impl QuestTranslationBatchRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Translate => "translate",
            Self::ProtectedContextOnly => "protected-context-only",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "translate" => Some(Self::Translate),
            "protected-context-only" => Some(Self::ProtectedContextOnly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTranslationContextConfidence {
    Confirmed,
    HighConfidence,
    NeedsHumanReview,
}

impl QuestTranslationContextConfidence {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::HighConfidence => "high-confidence",
            Self::NeedsHumanReview => "needs-human-review",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "confirmed" => Some(Self::Confirmed),
            "high-confidence" => Some(Self::HighConfidence),
            "needs-human-review" => Some(Self::NeedsHumanReview),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTranslationWorkStatus {
    DraftNeedsIndependentReview,
    NeedsHumanTermDecision,
    ProtectedContextOnly,
}

impl QuestTranslationWorkStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DraftNeedsIndependentReview => "draft-needs-independent-review",
            Self::NeedsHumanTermDecision => "needs-human-term-decision",
            Self::ProtectedContextOnly => "protected-context-only",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "draft-needs-independent-review" => Some(Self::DraftNeedsIndependentReview),
            "needs-human-term-decision" => Some(Self::NeedsHumanTermDecision),
            "protected-context-only" => Some(Self::ProtectedContextOnly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationBatchEntry {
    pub batch_id: String,
    pub batch_context_sha256: String,
    pub scene_id: String,
    pub scene_title: String,
    pub sequence_context: String,
    pub source_evidence: String,
    pub scene_order: usize,
    pub branch: String,
    pub stream_id: String,
    pub role: QuestTranslationBatchRole,
    pub speaker_id: String,
    pub speaker: String,
    pub addressee_id: String,
    pub addressee: String,
    pub confidence: QuestTranslationContextConfidence,
    pub context_note: String,
    pub protected_raw_hex: String,
    pub protected_source: String,
    pub translation_policy: QuestSeqTranslationUnitPolicy,
    pub control_signature: String,
    pub runtime_evidence: Option<&'static str>,
    pub matched_term_ids: Vec<String>,
    pub matched_term_decisions: Vec<&'static str>,
    pub korean_draft: String,
    pub work_status: QuestTranslationWorkStatus,
    pub work_notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationBatchAnalysis {
    pub entries: Vec<QuestTranslationBatchEntry>,
    pub batches: usize,
    pub translate_entries: usize,
    pub protected_context_entries: usize,
    pub drafted_entries: usize,
    pub term_decision_entries: usize,
    pub runtime_observed_entries: usize,
    pub affected_terms: usize,
    pub build_eligible_entries: usize,
    pub draft_hangul_repertoire: String,
    pub unassigned_translation_candidate_ids: Vec<String>,
    pub manifest_sha256: String,
    pub style_sha256: String,
    pub translation_units_sha256: String,
    pub terminology_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationBatchError(String);

impl fmt::Display for QuestTranslationBatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationBatchError {}

fn error(message: impl Into<String>) -> QuestTranslationBatchError {
    QuestTranslationBatchError(message.into())
}

fn verify_work_protected_source(
    stream_id: &str,
    work_source: &str,
    current_source: &str,
) -> Result<(), QuestTranslationBatchError> {
    if work_source != current_source {
        return Err(error(format!(
            "{stream_id}: work 보호 원문이 현재 ROM 추출 원문과 불일치"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestEntry {
    batch_id: String,
    scene_id: String,
    scene_title: String,
    sequence_context: String,
    source_evidence: String,
    scene_order: usize,
    branch: String,
    stream_id: String,
    role: QuestTranslationBatchRole,
    speaker_id: String,
    speaker: String,
    addressee_id: String,
    addressee: String,
    confidence: QuestTranslationContextConfidence,
    context_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkEntry {
    batch_id: String,
    stream_id: String,
    protected_source: String,
    korean_draft: String,
    status: QuestTranslationWorkStatus,
    notes: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn is_japanese(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{ff66}'..='\u{ff9f}'
    )
}

fn has_hangul(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

fn parse_manifest(input: &str) -> Result<Vec<ManifestEntry>, QuestTranslationBatchError> {
    let mut lines = input.lines();
    if lines.next().unwrap_or_default() != MANIFEST_HEADER {
        return Err(error("QUEST 번역 배치 manifest header 불일치"));
    }
    let mut entries = Vec::new();
    let mut stream_ids = BTreeSet::new();
    let mut orders = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            return Err(error(format!(
                "QUEST 번역 배치 manifest 빈 행: line={}",
                index + 2
            )));
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        let [batch_id, scene_id, scene_title, sequence_context, source_evidence, scene_order, branch, stream_id, role, speaker_id, speaker, addressee_id, addressee, confidence, context_note] =
            columns.as_slice()
        else {
            return Err(error(format!(
                "QUEST 번역 배치 manifest 열 수 불일치: line={}",
                index + 2
            )));
        };
        if columns.iter().any(|value| value.is_empty()) {
            return Err(error(format!(
                "QUEST 번역 배치 manifest 필수값 누락: line={}",
                index + 2
            )));
        }
        let scene_order = scene_order.parse::<usize>().map_err(|_| {
            error(format!(
                "QUEST 번역 배치 scene_order 숫자 아님: line={}",
                index + 2
            ))
        })?;
        if scene_order == 0 {
            return Err(error("QUEST 번역 배치 scene_order는 1부터 시작해야 함"));
        }
        if !matches!(*branch, "victory" | "defeat" | "shared") {
            return Err(error(format!("QUEST 번역 배치 branch 미지원: {branch}")));
        }
        let role = QuestTranslationBatchRole::parse(role)
            .ok_or_else(|| error(format!("QUEST 번역 배치 role 미지원: {role}")))?;
        let confidence = QuestTranslationContextConfidence::parse(confidence)
            .ok_or_else(|| error(format!("QUEST 번역 배치 confidence 미지원: {confidence}")))?;
        if !stream_ids.insert((*stream_id).to_owned()) {
            return Err(error(format!(
                "QUEST 번역 배치 stream 중복 배정: {stream_id}"
            )));
        }
        if !orders.insert(((*batch_id).to_owned(), scene_order)) {
            return Err(error(format!(
                "QUEST 번역 배치 scene_order 중복: {batch_id}#{scene_order}"
            )));
        }
        entries.push(ManifestEntry {
            batch_id: (*batch_id).to_owned(),
            scene_id: (*scene_id).to_owned(),
            scene_title: (*scene_title).to_owned(),
            sequence_context: (*sequence_context).to_owned(),
            source_evidence: (*source_evidence).to_owned(),
            scene_order,
            branch: (*branch).to_owned(),
            stream_id: (*stream_id).to_owned(),
            role,
            speaker_id: (*speaker_id).to_owned(),
            speaker: (*speaker).to_owned(),
            addressee_id: (*addressee_id).to_owned(),
            addressee: (*addressee).to_owned(),
            confidence,
            context_note: (*context_note).to_owned(),
        });
    }
    if entries.is_empty() {
        return Err(error("QUEST 번역 배치 manifest가 비어 있음"));
    }

    let mut by_batch = BTreeMap::<&str, Vec<&ManifestEntry>>::new();
    for entry in &entries {
        by_batch.entry(&entry.batch_id).or_default().push(entry);
    }
    for (batch_id, mut batch_entries) in by_batch {
        batch_entries.sort_by_key(|entry| entry.scene_order);
        let first = batch_entries[0];
        for (index, entry) in batch_entries.iter().enumerate() {
            if entry.scene_order != index + 1 {
                return Err(error(format!(
                    "QUEST 번역 배치 scene_order 비연속: {batch_id}"
                )));
            }
            if (
                &entry.scene_id,
                &entry.scene_title,
                &entry.sequence_context,
                &entry.source_evidence,
            ) != (
                &first.scene_id,
                &first.scene_title,
                &first.sequence_context,
                &first.source_evidence,
            ) {
                return Err(error(format!(
                    "QUEST 번역 배치 장면 metadata 불일치: {batch_id}"
                )));
            }
        }
    }
    entries.sort_by(|left, right| {
        left.batch_id
            .cmp(&right.batch_id)
            .then(left.scene_order.cmp(&right.scene_order))
    });
    Ok(entries)
}

fn render_batch_context_tsv(entries: &[&ManifestEntry]) -> String {
    let mut output = format!("{MANIFEST_HEADER}\n");
    for entry in entries {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            entry.batch_id,
            entry.scene_id,
            entry.scene_title,
            entry.sequence_context,
            entry.source_evidence,
            entry.scene_order,
            entry.branch,
            entry.stream_id,
            entry.role.label(),
            entry.speaker_id,
            entry.speaker,
            entry.addressee_id,
            entry.addressee,
            entry.confidence.label(),
            entry.context_note,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

fn batch_context_hashes(entries: &[ManifestEntry]) -> BTreeMap<String, String> {
    let mut grouped = BTreeMap::<String, Vec<&ManifestEntry>>::new();
    for entry in entries {
        grouped
            .entry(entry.batch_id.clone())
            .or_default()
            .push(entry);
    }
    grouped
        .into_iter()
        .map(|(batch_id, mut entries)| {
            entries.sort_by_key(|entry| entry.scene_order);
            let digest = sha256(render_batch_context_tsv(&entries).as_bytes());
            (batch_id, digest)
        })
        .collect()
}

fn verify_style_candidates(input: &str) -> Result<(), QuestTranslationBatchError> {
    let mut lines = input.lines();
    if lines.next().unwrap_or_default() != STYLE_HEADER {
        return Err(error("QUEST 문체 후보 header 불일치"));
    }
    let mut ids = BTreeSet::new();
    let mut rows = 0usize;
    for (index, line) in lines.enumerate() {
        let columns = line.split('\t').collect::<Vec<_>>();
        let [style_id, scope, rule, status, evidence, notes] = columns.as_slice() else {
            return Err(error(format!(
                "QUEST 문체 후보 열 수 불일치: line={}",
                index + 2
            )));
        };
        if [style_id, scope, rule, status, evidence, notes]
            .iter()
            .any(|value| value.is_empty())
        {
            return Err(error(format!(
                "QUEST 문체 후보 필수값 누락: line={}",
                index + 2
            )));
        }
        if !matches!(*status, "candidate" | "needs-human-review" | "protected") {
            return Err(error(format!("QUEST 문체 후보 status 미지원: {status}")));
        }
        if !ids.insert(*style_id) {
            return Err(error(format!("QUEST 문체 후보 ID 중복: {style_id}")));
        }
        rows += 1;
    }
    if rows == 0 {
        return Err(error("QUEST 문체 후보가 비어 있음"));
    }
    Ok(())
}

fn parse_work_files(
    batch_context_sha256: &BTreeMap<String, String>,
    expected_translation_units_sha256: &str,
    expected_terminology_sha256: &str,
    style_sha256: &str,
) -> Result<Vec<WorkEntry>, QuestTranslationBatchError> {
    let mut entries = Vec::new();
    let mut ids = BTreeSet::new();
    for (expected_batch_id, input) in WORK_FILES {
        let mut lines = input.lines();
        if lines.next().unwrap_or_default() != WORK_HEADER {
            return Err(error(format!(
                "{expected_batch_id}: QUEST 번역 work header 불일치"
            )));
        }
        for (index, line) in lines.enumerate() {
            let columns = line.split('\t').collect::<Vec<_>>();
            let [schema_version, batch_id, actual_batch_context_sha256, translation_units_sha256, terminology_sha256, actual_style_sha256, stream_id, protected_source, korean_draft, status, notes] =
                columns.as_slice()
            else {
                return Err(error(format!(
                    "{expected_batch_id}: QUEST 번역 work 열 수 불일치: line={}",
                    index + 2
                )));
            };
            if *schema_version != QUEST_TRANSLATION_WORK_SCHEMA_VERSION {
                return Err(error(format!(
                    "{expected_batch_id}: QUEST 번역 work schema 불일치"
                )));
            }
            if *batch_id != *expected_batch_id {
                return Err(error(format!(
                    "{expected_batch_id}: QUEST 번역 work batch ID 불일치"
                )));
            }
            let expected_batch_context_sha256 =
                batch_context_sha256.get(*batch_id).ok_or_else(|| {
                    error(format!(
                        "{expected_batch_id}: QUEST 번역 work에 대응하는 manifest batch 없음"
                    ))
                })?;
            let baselines = [
                (
                    *actual_batch_context_sha256,
                    expected_batch_context_sha256.as_str(),
                ),
                (*translation_units_sha256, expected_translation_units_sha256),
                (*terminology_sha256, expected_terminology_sha256),
                (*actual_style_sha256, style_sha256),
            ];
            if baselines
                .iter()
                .any(|(actual, expected)| actual != expected)
            {
                return Err(error(format!(
                    "{expected_batch_id}: QUEST 번역 work baseline drift: stream={stream_id}"
                )));
            }
            let status = QuestTranslationWorkStatus::parse(status).ok_or_else(|| {
                error(format!(
                    "{expected_batch_id}: QUEST 번역 work status 미지원: {status}"
                ))
            })?;
            if stream_id.is_empty() || protected_source.is_empty() || notes.is_empty() {
                return Err(error(format!(
                    "{expected_batch_id}: QUEST 번역 work 필수값 누락: line={}",
                    index + 2
                )));
            }
            match status {
                QuestTranslationWorkStatus::ProtectedContextOnly => {
                    if !korean_draft.is_empty() {
                        return Err(error(format!(
                            "{stream_id}: 보호 문맥에 한국어 초안이 있음"
                        )));
                    }
                }
                QuestTranslationWorkStatus::DraftNeedsIndependentReview => {
                    if korean_draft.is_empty() || !has_hangul(korean_draft) {
                        return Err(error(format!("{stream_id}: 한국어 초안이 비어 있음")));
                    }
                    if korean_draft.chars().any(is_japanese) {
                        return Err(error(format!("{stream_id}: 한국어 초안에 일본어가 남음")));
                    }
                }
                QuestTranslationWorkStatus::NeedsHumanTermDecision => {
                    if korean_draft.is_empty()
                        || !has_hangul(korean_draft)
                        || !korean_draft.contains('{')
                        || !korean_draft.contains('}')
                    {
                        return Err(error(format!(
                            "{stream_id}: 용어 결정 placeholder가 없는 초안"
                        )));
                    }
                    if korean_draft.chars().any(is_japanese) {
                        return Err(error(format!("{stream_id}: 한국어 초안에 일본어가 남음")));
                    }
                }
            }
            if !ids.insert((*stream_id).to_owned()) {
                return Err(error(format!(
                    "QUEST 번역 work stream ID 중복: {stream_id}"
                )));
            }
            entries.push(WorkEntry {
                batch_id: (*batch_id).to_owned(),
                stream_id: (*stream_id).to_owned(),
                protected_source: (*protected_source).to_owned(),
                korean_draft: (*korean_draft).to_owned(),
                status,
                notes: (*notes).to_owned(),
            });
        }
    }
    Ok(entries)
}

/// 고정 manifest·work와 현재 원본 파생 원장을 결합한다.
pub fn audit_quest_translation_batches(
    extraction: &QuestSeqTextExtractionAnalysis,
    translation_units: &QuestSeqTranslationUnitAnalysis,
    terminology: &QuestTerminologyCandidateAnalysis,
) -> Result<QuestTranslationBatchAnalysis, QuestTranslationBatchError> {
    verify_style_candidates(STYLE_CANDIDATES)?;
    let manifest_sha256 = sha256(MANIFEST.as_bytes());
    let style_sha256 = sha256(STYLE_CANDIDATES.as_bytes());
    let translation_units_sha256 =
        sha256(render_quest_seq_translation_units_tsv(translation_units).as_bytes());
    let terminology_sha256 = quest_terminology_report_sha256(terminology);
    let manifest = parse_manifest(MANIFEST)?;
    let batch_context_sha256 = batch_context_hashes(&manifest);
    let work = parse_work_files(
        &batch_context_sha256,
        &translation_units_sha256,
        &terminology_sha256,
        &style_sha256,
    )?;
    let work_by_id = work
        .iter()
        .map(|entry| (entry.stream_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    if work_by_id.len() != work.len() || work.len() != manifest.len() {
        return Err(error("QUEST 번역 manifest와 work 분모 불일치"));
    }

    let streams_by_id = extraction
        .streams
        .iter()
        .map(|stream| (stream.id(), stream))
        .collect::<BTreeMap<_, _>>();
    if streams_by_id.len() != extraction.streams.len() {
        return Err(error("QUEST 번역 배치 입력 extraction stream ID 중복"));
    }
    let units_by_id = translation_units
        .units
        .iter()
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    if units_by_id.len() != translation_units.units.len() {
        return Err(error("QUEST 번역 배치 입력 translation unit ID 중복"));
    }

    let mut entries = Vec::with_capacity(manifest.len());
    let mut affected_term_ids = BTreeSet::new();
    for context in manifest {
        let stream = streams_by_id
            .get(&context.stream_id)
            .ok_or_else(|| error(format!("{}: 추출 stream 없음", context.stream_id)))?;
        let unit = units_by_id
            .get(context.stream_id.as_str())
            .ok_or_else(|| error(format!("{}: 번역 검토 단위 없음", context.stream_id)))?;
        if stream.expected_raw_sha256 != unit.stream_sha256
            || stream.record_offset != unit.record_offset
            || stream.end_offset != unit.end_offset
        {
            return Err(error(format!(
                "{}: 추출 stream과 번역 검토 단위 불일치",
                context.stream_id
            )));
        }
        let work_entry = work_by_id
            .get(context.stream_id.as_str())
            .ok_or_else(|| error(format!("{}: 번역 work 행 없음", context.stream_id)))?;
        if work_entry.batch_id != context.batch_id {
            return Err(error(format!(
                "{}: manifest와 work batch ID 불일치",
                context.stream_id
            )));
        }
        verify_work_protected_source(
            &context.stream_id,
            &work_entry.protected_source,
            &unit.protected_source,
        )?;
        match (context.role, unit.policy, work_entry.status) {
            (
                QuestTranslationBatchRole::Translate,
                QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate,
                QuestTranslationWorkStatus::DraftNeedsIndependentReview
                | QuestTranslationWorkStatus::NeedsHumanTermDecision,
            )
            | (
                QuestTranslationBatchRole::ProtectedContextOnly,
                QuestSeqTranslationUnitPolicy::ProtectedRawReview,
                QuestTranslationWorkStatus::ProtectedContextOnly,
            ) => {}
            _ => {
                return Err(error(format!(
                    "{}: role/policy/work status 조합이 안전하지 않음",
                    context.stream_id
                )));
            }
        }
        let raw = serialize_quest_seq_extracted_text_stream(stream)
            .map_err(|source| error(format!("{}: raw 보호 실패: {source}", context.stream_id)))?;
        let matched_terms = terminology
            .terms
            .iter()
            .filter(|term| unit.protected_source.contains(&term.protected_source))
            .collect::<Vec<_>>();
        let conflicting_terms = matched_terms
            .iter()
            .filter(|term| term.decision == QuestTerminologyDecision::ConflictingSiblingCandidates)
            .collect::<Vec<_>>();
        if work_entry.status == QuestTranslationWorkStatus::DraftNeedsIndependentReview
            && !conflicting_terms.is_empty()
        {
            return Err(error(format!(
                "{}: 충돌 용어를 사람이 결정하지 않은 일반 초안",
                context.stream_id
            )));
        }
        if work_entry.status == QuestTranslationWorkStatus::NeedsHumanTermDecision {
            if conflicting_terms.is_empty() {
                return Err(error(format!(
                    "{}: 실제 충돌 용어가 없는 needs-human-term-decision",
                    context.stream_id
                )));
            }
            for term in &conflicting_terms {
                let placeholder = format!("{{{}}}", term.id);
                if !work_entry.korean_draft.contains(&placeholder) {
                    return Err(error(format!(
                        "{}: 충돌 용어 placeholder 누락: {placeholder}",
                        context.stream_id
                    )));
                }
            }
        }
        for term in &matched_terms {
            affected_term_ids.insert(term.id.clone());
            if let Some(approved) = term.approved_ko.as_deref() {
                if context.role == QuestTranslationBatchRole::Translate
                    && !work_entry.korean_draft.contains(approved)
                {
                    return Err(error(format!(
                        "{}: 승인 용어 누락: {} -> {approved}",
                        context.stream_id, term.id
                    )));
                }
            }
        }
        let context_sha256 = batch_context_sha256
            .get(&context.batch_id)
            .expect("manifest batch hash는 계산되어 있음")
            .clone();
        entries.push(QuestTranslationBatchEntry {
            batch_id: context.batch_id,
            batch_context_sha256: context_sha256,
            scene_id: context.scene_id,
            scene_title: context.scene_title,
            sequence_context: context.sequence_context,
            source_evidence: context.source_evidence,
            scene_order: context.scene_order,
            branch: context.branch,
            stream_id: context.stream_id,
            role: context.role,
            speaker_id: context.speaker_id,
            speaker: context.speaker,
            addressee_id: context.addressee_id,
            addressee: context.addressee,
            confidence: context.confidence,
            context_note: context.context_note,
            protected_raw_hex: hex(&raw),
            protected_source: unit.protected_source.clone(),
            translation_policy: unit.policy,
            control_signature: unit.control_signature.clone(),
            runtime_evidence: unit.runtime_evidence,
            matched_term_ids: matched_terms.iter().map(|term| term.id.clone()).collect(),
            matched_term_decisions: matched_terms
                .iter()
                .map(|term| term.decision.label())
                .collect(),
            korean_draft: work_entry.korean_draft.clone(),
            work_status: work_entry.status,
            work_notes: work_entry.notes.clone(),
        });
    }
    let batches = entries
        .iter()
        .map(|entry| entry.batch_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let translate_entries = entries
        .iter()
        .filter(|entry| entry.role == QuestTranslationBatchRole::Translate)
        .count();
    let protected_context_entries = entries.len() - translate_entries;
    let drafted_entries = entries
        .iter()
        .filter(|entry| entry.role == QuestTranslationBatchRole::Translate)
        .filter(|entry| !entry.korean_draft.is_empty())
        .count();
    let term_decision_entries = entries
        .iter()
        .filter(|entry| entry.work_status == QuestTranslationWorkStatus::NeedsHumanTermDecision)
        .count();
    let runtime_observed_entries = entries
        .iter()
        .filter(|entry| entry.runtime_evidence.is_some())
        .count();
    let draft_hangul_repertoire = entries
        .iter()
        .flat_map(|entry| entry.korean_draft.chars())
        .filter(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    let assigned_translation_ids = entries
        .iter()
        .filter(|entry| entry.role == QuestTranslationBatchRole::Translate)
        .map(|entry| entry.stream_id.as_str())
        .collect::<BTreeSet<_>>();
    let unassigned_translation_candidate_ids = translation_units
        .units
        .iter()
        .filter(|unit| {
            unit.policy == QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate
                && !assigned_translation_ids.contains(unit.id.as_str())
        })
        .map(|unit| unit.id.clone())
        .collect::<Vec<_>>();
    Ok(QuestTranslationBatchAnalysis {
        entries,
        batches,
        translate_entries,
        protected_context_entries,
        drafted_entries,
        term_decision_entries,
        runtime_observed_entries,
        affected_terms: affected_term_ids.len(),
        build_eligible_entries: 0,
        draft_hangul_repertoire,
        unassigned_translation_candidate_ids,
        manifest_sha256,
        style_sha256,
        translation_units_sha256,
        terminology_sha256,
    })
}

/// 지원 원본과 현재 version 관리 번역 입력을 결합해 매번 검증한다.
pub fn verify_box_quest_translation_batches(
    extraction: &QuestSeqTextExtractionAnalysis,
    translation_units: &QuestSeqTranslationUnitAnalysis,
    terminology: &QuestTerminologyCandidateAnalysis,
) -> Result<QuestTranslationBatchAnalysis, QuestTranslationBatchError> {
    audit_quest_translation_batches(extraction, translation_units, terminology)
}

/// 원본 raw·control과 장면 문맥·초벌 번역을 결합한 결정적 검토 TSV.
pub fn render_quest_translation_batches_tsv(analysis: &QuestTranslationBatchAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tbatch_id\tmanifest_sha256\tbatch_context_sha256\t\
         translation_units_sha256\t\
         terminology_sha256\tstyle_sha256\tscene_id\tscene_title\tsequence_context\t\
         source_evidence\tscene_order\tbranch\tstream_id\trole\tspeaker_id\tspeaker\t\
         addressee_id\taddressee\tconfidence\tcontext_note\ttranslation_policy\t\
         protected_raw_hex\tprotected_source\tcontrol_signature\truntime_evidence\t\
         matched_term_ids\tmatched_term_decisions\tko\twork_status\tbuild_eligible\t\
         draft_unique_hangul_syllables\tdraft_hangul_repertoire\t\
         unassigned_translation_candidates\tnotes\n",
    );
    for entry in &analysis.entries {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tno\t{}\t{}\t{}\t{}",
            QUEST_TRANSLATION_BATCH_SCHEMA_VERSION,
            entry.batch_id,
            analysis.manifest_sha256,
            entry.batch_context_sha256,
            analysis.translation_units_sha256,
            analysis.terminology_sha256,
            analysis.style_sha256,
            entry.scene_id,
            escape_tsv(&entry.scene_title),
            escape_tsv(&entry.sequence_context),
            escape_tsv(&entry.source_evidence),
            entry.scene_order,
            entry.branch,
            entry.stream_id,
            entry.role.label(),
            entry.speaker_id,
            escape_tsv(&entry.speaker),
            entry.addressee_id,
            escape_tsv(&entry.addressee),
            entry.confidence.label(),
            escape_tsv(&entry.context_note),
            entry.translation_policy.label(),
            entry.protected_raw_hex,
            escape_tsv(&entry.protected_source),
            entry.control_signature,
            entry.runtime_evidence.unwrap_or("static-only"),
            entry.matched_term_ids.join(","),
            entry.matched_term_decisions.join(","),
            escape_tsv(&entry.korean_draft),
            entry.work_status.label(),
            analysis.draft_hangul_repertoire.chars().count(),
            analysis.draft_hangul_repertoire,
            analysis.unassigned_translation_candidate_ids.len(),
            escape_tsv(&entry.work_notes),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// 현재 manifest·work·원문·용어 결합 결과의 lineage digest.
pub fn quest_translation_batch_report_sha256(analysis: &QuestTranslationBatchAnalysis) -> String {
    sha256(render_quest_translation_batches_tsv(analysis).as_bytes())
}

#[cfg(test)]
#[path = "quest_translation_batches_tests.rs"]
mod tests;
