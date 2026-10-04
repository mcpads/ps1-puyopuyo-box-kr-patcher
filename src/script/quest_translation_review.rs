//! QUEST 장면별 초벌의 독립 2차 검토와 사람 결정 대기열.
//!
//! 이 모듈은 기존 work TSV를 수정하지 않는다. 검토 자산의 보호 원문과
//! 초벌 snapshot을 현재 원본에서 다시 만든 번역 배치와 대조하고,
//! `동의 / 수정 제안 / 사람 결정 필요`를 별도 상태로 기록한다. 어느
//! 상태도 사람 승인이나 제품 build eligibility로 승격하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::quest_translation_batches::{
    quest_translation_batch_report_sha256, QuestTranslationBatchAnalysis,
    QuestTranslationBatchEntry, QuestTranslationBatchRole, QuestTranslationContextConfidence,
    QuestTranslationWorkStatus,
};

pub const QUEST_TRANSLATION_REVIEW_ASSET_SCHEMA_VERSION: &str = "quest-translation-review-v2";
pub const QUEST_TRANSLATION_REVIEW_SCHEMA_VERSION: &str = "quest-translation-review-v3";
const REVIEW_ASSETS: &[ReviewAsset] = &[
    ReviewAsset {
        batch_id: "quest-town00-puyoman-results-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-puyoman-results-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-hasi-schezo-aftermath-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-hasi-schezo-aftermath-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-hasi-schezo-discovery-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-hasi-schezo-discovery-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-hasi-schezo-wakeup-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-hasi-schezo-wakeup-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-miti-sudden-impact-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-miti-sudden-impact-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-miti-draco-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-miti-draco-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-miti-medal-rival-hint-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-miti-medal-rival-hint-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-zoh-hungry-shop-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-zoh-hungry-shop-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-iseki-six-medals-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-iseki-six-medals-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-iseki-earthquake-route-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-iseki-earthquake-route-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-iseki-deeper-quakes-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-iseki-deeper-quakes-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-iseki-zoh-misunderstanding-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-iseki-zoh-misunderstanding-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-iseki-minotauros-training-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-iseki-minotauros-training-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-iseki-minotauros-medal-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-iseki-minotauros-medal-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-shez-six-medals-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-shez-six-medals-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-shez-cave-confrontation-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-shez-cave-confrontation-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-shez-cave-inspection-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-shez-cave-inspection-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-shez-hot-spring-signs-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-shez-hot-spring-signs-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-shez-wall-notes-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-shez-wall-notes-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-suke-skeleton-tea-confrontation-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-suke-skeleton-tea-confrontation-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-suke-tea-battle-aftermath-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-suke-tea-battle-aftermath-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-suke-tea-scent-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-suke-tea-scent-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-tou-feast-ending-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-tou-feast-ending-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-tou-return-home-ending-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-tou-return-home-ending-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-tou-rulue-outlet-confrontation-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-tou-rulue-outlet-confrontation-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-tou-satan-proposal-choice-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-tou-satan-proposal-choice-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-tou-satan-rejection-chase-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-tou-satan-rejection-chase-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-tou-satan-schezo-epilogue-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-tou-satan-schezo-epilogue-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-tou-satan-wedding-reveal-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-tou-satan-wedding-reveal-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-tou-tower-door-open-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-tou-tower-door-open-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-tou-tower-medal-gate-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-tou-tower-medal-gate-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-jumper-boots-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-jumper-boots-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-badge-lady-help-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-badge-lady-help-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-lagnus-fans-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-lagnus-fans-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-panotty-flute-ambient-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-panotty-flute-ambient-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-shadow-puyo-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-shadow-puyo-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-domino-villager-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-domino-villager-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-shadow-puyo-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-shadow-puyo-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town30-pakista-shop-greeting-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town30-pakista-shop-greeting-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town30-parara-shop-greeting-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town30-parara-shop-greeting-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town30-sasoriman-shop-greeting-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town30-sasoriman-shop-greeting-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town30-pakista-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town30-pakista-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town30-pakista-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town30-pakista-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town30-parara-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town30-parara-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town30-sasoriman-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town30-sasoriman-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town30-sasoriman-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town30-sasoriman-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-carbuncle-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-carbuncle-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-plumber-sign-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-plumber-sign-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-skeleton-medal-hint-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-skeleton-medal-hint-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-cherrybell-sign-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-cherrybell-sign-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-child-play-car-request-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-child-play-car-request-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-lagnus-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-lagnus-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-lagnus-wall-notes-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-lagnus-wall-notes-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-panotty-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-panotty-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-child-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-child-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-fufufu-shop-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-fufufu-shop-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-sevenlease-sign-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-sevenlease-sign-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-sleepy-flirt-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-sleepy-flirt-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town30-parara-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town30-parara-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-seriri-crying-approach-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-seriri-crying-approach-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-seriri-friendship-followup-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-seriri-friendship-followup-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-seriri-medal-choice-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-seriri-medal-choice-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-seriri-medal-gift-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-seriri-medal-gift-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-seriri-misunderstanding-resolution-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-seriri-misunderstanding-resolution-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-seriri-waiting-ending-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-seriri-waiting-ending-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-suketoudara-accusation-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-suketoudara-accusation-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-suketoudara-first-meeting-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-suketoudara-first-meeting-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-suketoudara-medal-aftermath-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-suketoudara-medal-aftermath-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-taraxi-first-crossing-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-taraxi-first-crossing-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-taraxi-signs-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-taraxi-signs-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-taraxi-westbound-service-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-taraxi-westbound-service-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-kawa-taraxi-eastbound-service-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-kawa-taraxi-eastbound-service-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-mori-harpy-song-barrier-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-mori-harpy-song-barrier-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-mori-harpy-first-confrontation-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-mori-harpy-first-confrontation-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-mori-harpy-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-mori-harpy-rematch-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-mori-earplug-aftermath-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-mori-earplug-aftermath-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-mori-owlbear-honey-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-mori-owlbear-honey-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-medal-legend-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-medal-legend-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-nununun-consolation-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-nununun-consolation-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-medal-journey-departure-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-medal-journey-departure-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-momomo-shop-battles-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-momomo-shop-battles-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-samurai-hot-tea-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-samurai-hot-tea-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-kikimora-cleaning-request-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-kikimora-cleaning-request-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-carbuncle-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-carbuncle-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-draco-puyoman-aftermath-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-draco-puyoman-aftermath-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-house-post-subjects-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-house-post-subjects-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-jumper-battles-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-jumper-battles-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-kikimora-battles-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-kikimora-battles-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town00-lagnus-post-messages-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town00-lagnus-post-messages-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-badge-lady-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-badge-lady-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-badge-lady-rematches-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-badge-lady-rematches-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-incubus-medal-hint-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-incubus-medal-hint-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-lagnus-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-lagnus-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-lagnus-medal-hints-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-lagnus-medal-hints-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-lagnus-medal-intro-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-lagnus-medal-intro-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-lagnus-post-messages-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-lagnus-post-messages-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-lagnus-post-subjects-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-lagnus-post-subjects-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-nohoho-shop-battles-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-nohoho-shop-battles-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-noisy-forest-villagers-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-noisy-forest-villagers-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-panotty-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-panotty-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town10-rulue-medal-scent-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town10-rulue-medal-scent-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-child-father-worry-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-child-father-worry-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-child-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-child-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-earthquake-villagers-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-earthquake-villagers-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-fufufu-battles-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-fufufu-battles-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-lagnus-post-messages-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-lagnus-post-messages-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-lagnus-post-subjects-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-lagnus-post-subjects-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-sleepy-flirt-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-sleepy-flirt-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-sleepy-flirt-request-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-sleepy-flirt-request-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-zoh-first-battle-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-zoh-first-battle-001-review.tsv"
        ),
    },
    ReviewAsset {
        batch_id: "quest-town20-zoh-rematch-001",
        source: include_str!(
            "../../assets/translations/reviews/in_progress/quest-town20-zoh-rematch-001-review.tsv"
        ),
    },
];
const REVIEW_HEADER: &str = "schema_version\tbatch_id\tbatch_context_sha256\tstream_id\t\
     protected_source\tdraft_ko\treview_ko\tstatus\tdecision_dependencies\treview_notes";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReviewAsset {
    batch_id: &'static str,
    source: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTranslationReviewRequirement {
    ProtectedContextOnly,
    HumanTermDecision,
    HumanContextDecision,
    RuntimeSampleIndependentReview,
    IndependentReview,
}

impl QuestTranslationReviewRequirement {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProtectedContextOnly => "protected-context-only",
            Self::HumanTermDecision => "human-term-decision",
            Self::HumanContextDecision => "human-context-decision",
            Self::RuntimeSampleIndependentReview => "runtime-sample-independent-review",
            Self::IndependentReview => "independent-review",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTranslationIndependentReviewStatus {
    ReviewAgrees,
    ReviewProposesChange,
    NeedsHumanDecision,
}

impl QuestTranslationIndependentReviewStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReviewAgrees => "review-agrees",
            Self::ReviewProposesChange => "review-proposes-change",
            Self::NeedsHumanDecision => "needs-human-decision",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "review-agrees" => Some(Self::ReviewAgrees),
            "review-proposes-change" => Some(Self::ReviewProposesChange),
            "needs-human-decision" => Some(Self::NeedsHumanDecision),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationReviewEntry {
    pub batch_entry: QuestTranslationBatchEntry,
    pub requirement: QuestTranslationReviewRequirement,
    pub independent_review_status: Option<QuestTranslationIndependentReviewStatus>,
    pub reviewed_korean: String,
    pub decision_dependencies: Vec<String>,
    pub review_notes: String,
    pub review_asset_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationReviewAnalysis {
    pub entries: Vec<QuestTranslationReviewEntry>,
    pub review_assets: usize,
    pub protected_context_entries: usize,
    pub human_term_decision_entries: usize,
    pub human_context_decision_entries: usize,
    pub runtime_sample_entries: usize,
    pub independent_review_queue_entries: usize,
    pub reviewed_entries: usize,
    pub review_agreements: usize,
    pub review_proposals: usize,
    pub review_human_decision_rows: usize,
    pub unreviewed_drafts: usize,
    pub build_eligible_entries: usize,
    pub translation_batch_report_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationReviewError(String);

impl fmt::Display for QuestTranslationReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationReviewError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReviewBaselineEntry {
    batch_id: String,
    batch_context_sha256: String,
    stream_id: String,
    protected_source: String,
    draft_korean: String,
    reviewed_korean: String,
    status: QuestTranslationIndependentReviewStatus,
    decision_dependencies: Vec<String>,
    notes: String,
    review_asset_sha256: String,
}

fn error(message: impl Into<String>) -> QuestTranslationReviewError {
    QuestTranslationReviewError(message.into())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn has_hangul(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
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

fn placeholders_are_well_formed(value: &str) -> bool {
    let mut inside_placeholder = false;
    let mut placeholder_has_content = false;
    let mut placeholder_ends_with_separator = false;

    for character in value.chars() {
        if inside_placeholder {
            match character {
                '}' if placeholder_has_content && !placeholder_ends_with_separator => {
                    inside_placeholder = false;
                    placeholder_has_content = false;
                }
                'a'..='z' => {
                    placeholder_has_content = true;
                    placeholder_ends_with_separator = false;
                }
                '0'..='9' if placeholder_has_content => {
                    placeholder_ends_with_separator = false;
                }
                '.' | '_' if placeholder_has_content && !placeholder_ends_with_separator => {
                    placeholder_ends_with_separator = true;
                }
                _ => return false,
            }
        } else {
            match character {
                '{' => {
                    inside_placeholder = true;
                    placeholder_ends_with_separator = false;
                }
                '}' => return false,
                _ => {}
            }
        }
    }

    !inside_placeholder
}

fn has_only_placeholders_and_punctuation(value: &str) -> bool {
    let mut saw_placeholder = false;
    let mut inside_placeholder = false;
    let mut placeholder_has_content = false;

    for character in value.chars() {
        if inside_placeholder {
            match character {
                '}' if placeholder_has_content => {
                    saw_placeholder = true;
                    inside_placeholder = false;
                    placeholder_has_content = false;
                }
                '}' => return false,
                'a'..='z' | '0'..='9' | '.' | '_' => placeholder_has_content = true,
                _ => return false,
            }
        } else {
            match character {
                '{' => inside_placeholder = true,
                '}' => return false,
                _ if character.is_alphanumeric() || is_japanese(character) => return false,
                _ => {}
            }
        }
    }

    saw_placeholder && !inside_placeholder
}

fn has_only_review_punctuation_and_spacing(value: &str) -> bool {
    let mut saw_punctuation = false;

    for character in value.chars() {
        if character.is_whitespace() {
            continue;
        }
        if matches!(
            character,
            '.' | ','
                | ':'
                | ';'
                | '!'
                | '?'
                | '-'
                | '~'
                | '…'
                | '⋯'
                | '・'
                | '。'
                | '、'
                | '！'
                | '？'
                | '「'
                | '」'
                | '『'
                | '』'
                | '（'
                | '）'
                | '('
                | ')'
                | '〜'
                | '～'
                | '‼'
                | '⁇'
        ) {
            saw_punctuation = true;
        } else {
            return false;
        }
    }

    saw_punctuation
}

pub(crate) fn is_complete_korean_review_candidate(protected_source: &str, value: &str) -> bool {
    placeholders_are_well_formed(value)
        && !value.chars().any(is_japanese)
        && (has_hangul(value)
            || has_only_placeholders_and_punctuation(value)
            || (has_only_review_punctuation_and_spacing(protected_source)
                && has_only_review_punctuation_and_spacing(value)))
}

fn parse_review_baseline(
    expected_batch_id: &str,
    input: &str,
) -> Result<Vec<ReviewBaselineEntry>, QuestTranslationReviewError> {
    let mut lines = input.lines();
    if lines.next().unwrap_or_default() != REVIEW_HEADER {
        return Err(error("QUEST 독립 검토 header 불일치"));
    }
    let mut entries = Vec::new();
    let mut stream_ids = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let columns = line.split('\t').collect::<Vec<_>>();
        let [schema_version, batch_id, batch_context_sha256, stream_id, protected_source, draft_korean, reviewed_korean, status, decision_dependencies, notes] =
            columns.as_slice()
        else {
            return Err(error(format!(
                "QUEST 독립 검토 열 수 불일치: line={}",
                index + 2
            )));
        };
        if *schema_version != QUEST_TRANSLATION_REVIEW_ASSET_SCHEMA_VERSION {
            return Err(error(format!(
                "QUEST 독립 검토 schema 불일치: line={}",
                index + 2
            )));
        }
        if *batch_id != expected_batch_id {
            return Err(error(format!(
                "QUEST 독립 검토 batch 불일치: line={}",
                index + 2
            )));
        }
        if [
            batch_context_sha256,
            stream_id,
            protected_source,
            draft_korean,
            reviewed_korean,
            notes,
        ]
        .iter()
        .any(|value| value.is_empty())
        {
            return Err(error(format!(
                "QUEST 독립 검토 필수값 누락: line={}",
                index + 2
            )));
        }
        if !is_complete_korean_review_candidate(protected_source, reviewed_korean) {
            return Err(error(format!(
                "{stream_id}: 독립 검토 번역문이 한국어 완성 후보가 아님"
            )));
        }
        let status = QuestTranslationIndependentReviewStatus::parse(status)
            .ok_or_else(|| error(format!("QUEST 독립 검토 status 미지원: {status}")))?;
        let dependencies = if decision_dependencies.is_empty() {
            Vec::new()
        } else {
            decision_dependencies
                .split(';')
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        if dependencies.iter().any(|dependency| dependency.is_empty())
            || dependencies.iter().collect::<BTreeSet<_>>().len() != dependencies.len()
        {
            return Err(error(format!(
                "{stream_id}: 독립 검토 결정 dependency가 비었거나 중복됨"
            )));
        }
        match status {
            QuestTranslationIndependentReviewStatus::ReviewAgrees => {
                if reviewed_korean != draft_korean || !dependencies.is_empty() {
                    return Err(error(format!(
                        "{stream_id}: review-agrees 내용 또는 dependency 불일치"
                    )));
                }
            }
            QuestTranslationIndependentReviewStatus::ReviewProposesChange => {
                if reviewed_korean == draft_korean || !dependencies.is_empty() {
                    return Err(error(format!(
                        "{stream_id}: review-proposes-change 내용 또는 dependency 불일치"
                    )));
                }
            }
            QuestTranslationIndependentReviewStatus::NeedsHumanDecision => {
                if dependencies.is_empty() {
                    return Err(error(format!(
                        "{stream_id}: needs-human-decision dependency 누락"
                    )));
                }
            }
        }
        if !stream_ids.insert((*stream_id).to_owned()) {
            return Err(error(format!(
                "QUEST 독립 검토 stream ID 중복: {stream_id}"
            )));
        }
        entries.push(ReviewBaselineEntry {
            batch_id: (*batch_id).to_owned(),
            batch_context_sha256: (*batch_context_sha256).to_owned(),
            stream_id: (*stream_id).to_owned(),
            protected_source: (*protected_source).to_owned(),
            draft_korean: (*draft_korean).to_owned(),
            reviewed_korean: (*reviewed_korean).to_owned(),
            status,
            decision_dependencies: dependencies,
            notes: (*notes).to_owned(),
            review_asset_sha256: sha256(input.as_bytes()),
        });
    }
    if entries.is_empty() {
        return Err(error("QUEST 독립 검토 자산이 비어 있음"));
    }
    Ok(entries)
}

fn requirement(entry: &QuestTranslationBatchEntry) -> QuestTranslationReviewRequirement {
    if entry.role == QuestTranslationBatchRole::ProtectedContextOnly {
        QuestTranslationReviewRequirement::ProtectedContextOnly
    } else if entry.work_status == QuestTranslationWorkStatus::NeedsHumanTermDecision {
        QuestTranslationReviewRequirement::HumanTermDecision
    } else if entry.confidence == QuestTranslationContextConfidence::NeedsHumanReview {
        QuestTranslationReviewRequirement::HumanContextDecision
    } else if entry.runtime_evidence.is_some() {
        QuestTranslationReviewRequirement::RuntimeSampleIndependentReview
    } else {
        QuestTranslationReviewRequirement::IndependentReview
    }
}

fn verify_review_snapshot(
    review: &ReviewBaselineEntry,
    current: &QuestTranslationBatchEntry,
) -> Result<(), QuestTranslationReviewError> {
    if review.batch_id != current.batch_id
        || review.batch_context_sha256 != current.batch_context_sha256
        || review.protected_source != current.protected_source
        || review.draft_korean != current.korean_draft
    {
        return Err(error(format!(
            "{}: 독립 검토의 batch context/보호 원문/초벌 baseline drift",
            review.stream_id
        )));
    }
    Ok(())
}

fn verify_review_coverage(
    reviewed_ids: &BTreeSet<&str>,
    batch_translation_ids: &BTreeSet<&str>,
) -> Result<(), QuestTranslationReviewError> {
    if !reviewed_ids.is_subset(batch_translation_ids) {
        return Err(error(
            "QUEST 독립 검토에 현재 batch의 번역 후보가 아닌 stream이 있음",
        ));
    }
    Ok(())
}

fn audit_quest_translation_reviews_with_assets(
    batches: &QuestTranslationBatchAnalysis,
    review_assets: &[ReviewAsset],
) -> Result<QuestTranslationReviewAnalysis, QuestTranslationReviewError> {
    let mut asset_batch_ids = BTreeSet::new();
    let mut review_baseline = Vec::new();
    for asset in review_assets {
        if !asset_batch_ids.insert(asset.batch_id) {
            return Err(error(format!(
                "QUEST 독립 검토 자산 batch 중복: {}",
                asset.batch_id
            )));
        }
        let parsed = parse_review_baseline(asset.batch_id, asset.source)?;
        let batch_translation_ids = batches
            .entries
            .iter()
            .filter(|entry| {
                entry.batch_id == asset.batch_id
                    && entry.role == QuestTranslationBatchRole::Translate
            })
            .map(|entry| entry.stream_id.as_str())
            .collect::<BTreeSet<_>>();
        if batch_translation_ids.is_empty() {
            return Err(error(format!(
                "QUEST 독립 검토 자산의 현재 번역 batch가 없음: {}",
                asset.batch_id
            )));
        }
        let reviewed_ids = parsed
            .iter()
            .map(|entry| entry.stream_id.as_str())
            .collect::<BTreeSet<_>>();
        verify_review_coverage(&reviewed_ids, &batch_translation_ids)?;
        review_baseline.extend(parsed);
    }
    let review_by_id = review_baseline
        .iter()
        .map(|entry| (entry.stream_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    if review_by_id.len() != review_baseline.len() {
        return Err(error("QUEST 독립 검토 stream ID 중복"));
    }

    let mut entries = Vec::with_capacity(batches.entries.len());
    for batch_entry in &batches.entries {
        let review = review_by_id.get(batch_entry.stream_id.as_str()).copied();
        if let Some(review) = review {
            verify_review_snapshot(review, batch_entry)?;
        }
        entries.push(QuestTranslationReviewEntry {
            batch_entry: batch_entry.clone(),
            requirement: requirement(batch_entry),
            independent_review_status: review.map(|entry| entry.status),
            reviewed_korean: review
                .map(|entry| entry.reviewed_korean.clone())
                .unwrap_or_default(),
            decision_dependencies: review
                .map(|entry| entry.decision_dependencies.clone())
                .unwrap_or_default(),
            review_notes: review.map(|entry| entry.notes.clone()).unwrap_or_default(),
            review_asset_sha256: review.map(|entry| entry.review_asset_sha256.clone()),
        });
    }

    let protected_context_entries = entries
        .iter()
        .filter(|entry| {
            entry.requirement == QuestTranslationReviewRequirement::ProtectedContextOnly
        })
        .count();
    let human_term_decision_entries = entries
        .iter()
        .filter(|entry| entry.requirement == QuestTranslationReviewRequirement::HumanTermDecision)
        .count();
    let human_context_decision_entries = entries
        .iter()
        .filter(|entry| {
            entry.requirement == QuestTranslationReviewRequirement::HumanContextDecision
        })
        .count();
    let runtime_sample_entries = entries
        .iter()
        .filter(|entry| {
            entry.requirement == QuestTranslationReviewRequirement::RuntimeSampleIndependentReview
        })
        .count();
    let independent_review_queue_entries = entries
        .iter()
        .filter(|entry| entry.requirement == QuestTranslationReviewRequirement::IndependentReview)
        .count();
    let reviewed_entries = review_baseline.len();
    let review_agreements = review_baseline
        .iter()
        .filter(|entry| entry.status == QuestTranslationIndependentReviewStatus::ReviewAgrees)
        .count();
    let review_proposals = review_baseline
        .iter()
        .filter(|entry| {
            entry.status == QuestTranslationIndependentReviewStatus::ReviewProposesChange
        })
        .count();
    let review_human_decision_rows = review_baseline
        .iter()
        .filter(|entry| entry.status == QuestTranslationIndependentReviewStatus::NeedsHumanDecision)
        .count();
    let unreviewed_drafts = batches.translate_entries - reviewed_entries;

    Ok(QuestTranslationReviewAnalysis {
        entries,
        review_assets: review_assets.len(),
        protected_context_entries,
        human_term_decision_entries,
        human_context_decision_entries,
        runtime_sample_entries,
        independent_review_queue_entries,
        reviewed_entries,
        review_agreements,
        review_proposals,
        review_human_decision_rows,
        unreviewed_drafts,
        build_eligible_entries: 0,
        translation_batch_report_sha256: quest_translation_batch_report_sha256(batches),
    })
}

/// 현재 번역 배치에 version 관리 독립 검토 자산을 결합하고 전체 검토 큐를 만든다.
pub fn audit_quest_translation_reviews(
    batches: &QuestTranslationBatchAnalysis,
) -> Result<QuestTranslationReviewAnalysis, QuestTranslationReviewError> {
    audit_quest_translation_reviews_with_assets(batches, REVIEW_ASSETS)
}

/// 지원 원본과 현재 version 관리 검토 자산을 결합해 매번 검증한다.
pub fn verify_box_quest_translation_reviews(
    batches: &QuestTranslationBatchAnalysis,
) -> Result<QuestTranslationReviewAnalysis, QuestTranslationReviewError> {
    audit_quest_translation_reviews(batches)
}

/// 보호 원문·초벌·독립 검토 제안을 한 행에서 대조하는 결정적 TSV.
pub fn render_quest_translation_reviews_tsv(analysis: &QuestTranslationReviewAnalysis) -> String {
    let mut output = String::from(
        "schema_version\ttranslation_batch_report_sha256\treview_asset_sha256\tbatch_id\t\
         batch_context_sha256\tscene_id\tscene_title\tscene_order\tbranch\tstream_id\trole\t\
         confidence\truntime_evidence\tqueue_requirement\tprotected_source\tdraft_ko\t\
         work_status\tindependent_review_status\treview_ko\tdecision_dependencies\t\
         work_notes\treview_notes\tbuild_eligible\n",
    );
    for entry in &analysis.entries {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             {}\t{}\t{}\t{}\tno",
            QUEST_TRANSLATION_REVIEW_SCHEMA_VERSION,
            analysis.translation_batch_report_sha256,
            entry.review_asset_sha256.as_deref().unwrap_or_default(),
            entry.batch_entry.batch_id,
            entry.batch_entry.batch_context_sha256,
            entry.batch_entry.scene_id,
            escape_tsv(&entry.batch_entry.scene_title),
            entry.batch_entry.scene_order,
            entry.batch_entry.branch,
            entry.batch_entry.stream_id,
            entry.batch_entry.role.label(),
            entry.batch_entry.confidence.label(),
            entry.batch_entry.runtime_evidence.unwrap_or("static-only"),
            entry.requirement.label(),
            escape_tsv(&entry.batch_entry.protected_source),
            escape_tsv(&entry.batch_entry.korean_draft),
            entry.batch_entry.work_status.label(),
            entry
                .independent_review_status
                .map(QuestTranslationIndependentReviewStatus::label)
                .unwrap_or("not-reviewed"),
            escape_tsv(&entry.reviewed_korean),
            entry.decision_dependencies.join(";"),
            escape_tsv(&entry.batch_entry.work_notes),
            escape_tsv(&entry.review_notes),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// 현재 batch와 독립 검토 자산 결합 결과의 lineage digest.
pub fn quest_translation_review_report_sha256(analysis: &QuestTranslationReviewAnalysis) -> String {
    sha256(render_quest_translation_reviews_tsv(analysis).as_bytes())
}

#[cfg(test)]
#[path = "quest_translation_review_tests.rs"]
mod tests;
