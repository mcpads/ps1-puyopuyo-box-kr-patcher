//! OPAA 아이템 획득 화면이 직접 참조하는 고정 SJIS 원문 계약.

use super::{QuestItemAcquisitionMessage, QuestItemAcquisitionMessageRole};

#[derive(Debug, Clone, Copy)]
pub(super) struct MessageSpec {
    pub(super) message: QuestItemAcquisitionMessage,
}

pub(super) const QUEST_ITEM_ACQUISITION_MESSAGE_SPECS: [MessageSpec; 7] = [
    MessageSpec {
        message: QuestItemAcquisitionMessage {
            role: QuestItemAcquisitionMessageRole::CurrencyAcquiredSuffix,
            runtime: 0x8001_2210,
            source_text: "Ｇを手に入れた!!",
            source_bytes_with_nul: b"\x82\x66\x82\xF0\x8E\xE8\x82\xC9\x93\xFC\x82\xEA\x82\xBD!!\0",
        },
    },
    MessageSpec {
        message: QuestItemAcquisitionMessage {
            role: QuestItemAcquisitionMessageRole::ItemAcquiredSuffix,
            runtime: 0x8001_2224,
            source_text: "を手に入れた!!",
            source_bytes_with_nul: b"\x82\xF0\x8E\xE8\x82\xC9\x93\xFC\x82\xEA\x82\xBD!!\0",
        },
    },
    MessageSpec {
        message: QuestItemAcquisitionMessage {
            role: QuestItemAcquisitionMessageRole::InventoryFullNotice,
            runtime: 0x8001_2234,
            source_text: "しかし　持ち物がいっぱいで持てない。",
            source_bytes_with_nul: b"\x82\xB5\x82\xA9\x82\xB5\x81\x40\x8E\x9D\x82\xBF\x95\xA8\x82\xAA\x82\xA2\x82\xC1\x82\xCF\x82\xA2\x82\xC5\x8E\x9D\x82\xC4\x82\xC8\x82\xA2\x81\x42\0",
        },
    },
    MessageSpec {
        message: QuestItemAcquisitionMessage {
            role: QuestItemAcquisitionMessageRole::DiscardPrompt,
            runtime: 0x8001_225C,
            source_text: "何か　すてますか？",
            source_bytes_with_nul: b"\x89\xBD\x82\xA9\x81\x40\x82\xB7\x82\xC4\x82\xDC\x82\xB7\x82\xA9\x81\x48\0",
        },
    },
    MessageSpec {
        message: QuestItemAcquisitionMessage {
            role: QuestItemAcquisitionMessageRole::AbandonSubject,
            runtime: 0x8001_2270,
            source_text: "アルルは、",
            source_bytes_with_nul: b"\x83\x41\x83\x8B\x83\x8B\x82\xCD\x81\x41\0",
        },
    },
    MessageSpec {
        message: QuestItemAcquisitionMessage {
            role: QuestItemAcquisitionMessageRole::AbandonParticle,
            runtime: 0x800B_E1F0,
            source_text: "を",
            source_bytes_with_nul: b"\x82\xF0\0",
        },
    },
    MessageSpec {
        message: QuestItemAcquisitionMessage {
            role: QuestItemAcquisitionMessageRole::AbandonPredicate,
            runtime: 0x8001_227C,
            source_text: "あきらめた",
            source_bytes_with_nul: b"\x82\xA0\x82\xAB\x82\xE7\x82\xDF\x82\xBD\0",
        },
    },
];
