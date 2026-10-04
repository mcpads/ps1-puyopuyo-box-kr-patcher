//! QUEST world-map 지명과 공용 예/아니요 literal의 원본·소비 경계.
//!
//! 두 surface는 `QUEST.EXE`의 NUL slot을 직접 읽지만 소비 방식이 다르다.
//! world-map 지명은 state dispatch 뒤 whole-string renderer로 가고, 공용
//! 선택지는 네 callback이 같은 두 literal을 direct glyph loop로 그린다.
//! 이 모듈은 보이는 한 문구만 고르지 않고 14개 slot과 모든 정적 참조를
//! 지원 원본에 결속한다. 상태·장비·도구 고정문은 menu 모듈이 같은 경계로
//! 검산한다. 번역문·page 공급·실행 표시는 소유하지 않는다.

#[path = "quest_menu_literals.rs"]
mod menu;
pub use menu::{
    QuestMenuGlyphCall, QUEST_MENU_GLYPH_CALLS, QUEST_MENU_LITERAL_COUNT, QUEST_MENU_LITERAL_SPECS,
};

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode, decode_slice, InstructionKind as K, Reg};
use crate::text::encode_sjis;

use super::xref::{find_address_loads, DEFAULT_WINDOW};

pub const QUEST_WORLD_MAP_LABEL_TABLE_RUNTIME: u32 = 0x8001_236C;
pub const QUEST_WORLD_MAP_LABEL_TABLE_END_RUNTIME: u32 = 0x8001_242C;
pub const QUEST_WORLD_MAP_LABEL_STATE_RUNTIME: u32 = 0x8006_62E4;
pub const QUEST_WORLD_MAP_LABEL_DISPATCH_TABLE_RUNTIME: u32 = 0x8001_28F0;
pub const QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME: u32 = 0x8006_63E0;
pub const QUEST_UI_CHOICE_YES_RUNTIME: u32 = 0x800B_E0F8;
pub const QUEST_UI_CHOICE_NO_RUNTIME: u32 = 0x800B_E100;
pub const QUEST_UI_CHOICE_TABLE_END_RUNTIME: u32 = 0x800B_E108;
pub const QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES: [u32; 8] = [
    0x8003_5F54,
    0x8003_5FE8,
    0x8003_7730,
    0x8003_77E4,
    0x8003_7DA8,
    0x8003_7E3C,
    0x8003_9B04,
    0x8003_9B98,
];

const QUEST_WHOLE_STRING_RENDERER_RUNTIME: u32 = 0x8003_CD04;
const QUEST_GLYPH_RENDERER_RUNTIME: u32 = 0x8002_BA40;
const QUEST_STRING_LENGTH_RUNTIME: u32 = 0x8009_1D98;
const QUEST_TEXT_WINDOW_NEXT_LINE_RUNTIME: u32 = 0x8002_C664;

/// EXE literal이 속한 표시 surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuestUiLiteralSurface {
    WorldMapLabel,
    BinaryChoice,
    Menu,
}

impl QuestUiLiteralSurface {
    pub const fn label(self) -> &'static str {
        match self {
            Self::WorldMapLabel => "world-map-label",
            Self::BinaryChoice => "binary-choice",
            Self::Menu => "menu",
        }
    }
}

/// source-bound UI literal의 안정 역할.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuestUiLiteralRole {
    PlumberVillage,
    SkeletonTCave,
    NeighborTownBridge,
    CherrybellTown,
    HappinessTower,
    HarpyForest,
    MountainPassCave,
    SevenleaseVillage,
    ZohDaimaohRuins,
    PakistaShop,
    PararaShop,
    SasorimanShop,
    ChoiceYes,
    ChoiceNo,
    MenuText(&'static str),
}

impl QuestUiLiteralRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PlumberVillage => "plumber-village",
            Self::SkeletonTCave => "skeleton-t-cave",
            Self::NeighborTownBridge => "neighbor-town-bridge",
            Self::CherrybellTown => "cherrybell-town",
            Self::HappinessTower => "happiness-tower",
            Self::HarpyForest => "harpy-forest",
            Self::MountainPassCave => "mountain-pass-cave",
            Self::SevenleaseVillage => "sevenlease-village",
            Self::ZohDaimaohRuins => "zoh-daimaoh-ruins",
            Self::PakistaShop => "pakista-shop",
            Self::PararaShop => "parara-shop",
            Self::SasorimanShop => "sasoriman-shop",
            Self::ChoiceYes => "choice-yes",
            Self::ChoiceNo => "choice-no",
            Self::MenuText(role) => role,
        }
    }

    pub const fn surface(self) -> QuestUiLiteralSurface {
        match self {
            Self::ChoiceYes | Self::ChoiceNo => QuestUiLiteralSurface::BinaryChoice,
            Self::MenuText(_) => QuestUiLiteralSurface::Menu,
            _ => QuestUiLiteralSurface::WorldMapLabel,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LiteralSpec {
    role: QuestUiLiteralRole,
    runtime: u32,
    end_runtime: u32,
    source_text: &'static str,
}

const LITERAL_SPECS: [LiteralSpec; 14] = [
    LiteralSpec {
        role: QuestUiLiteralRole::PlumberVillage,
        runtime: 0x8001_236C,
        end_runtime: 0x8001_237C,
        source_text: "プランバーの村",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::SkeletonTCave,
        runtime: 0x8001_237C,
        end_runtime: 0x8001_238C,
        source_text: "スケＴの洞くつ",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::NeighborTownBridge,
        runtime: 0x8001_238C,
        end_runtime: 0x8001_239C,
        source_text: "となり町への橋",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::CherrybellTown,
        runtime: 0x8001_239C,
        end_runtime: 0x8001_23AC,
        source_text: "チェリベルの町",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::HappinessTower,
        runtime: 0x8001_23AC,
        end_runtime: 0x8001_23BC,
        source_text: "しあわせの塔",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::HarpyForest,
        runtime: 0x8001_23BC,
        end_runtime: 0x8001_23CC,
        source_text: "ハーピーの森",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::MountainPassCave,
        runtime: 0x8001_23CC,
        end_runtime: 0x8001_23D8,
        source_text: "峠の洞くつ",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::SevenleaseVillage,
        runtime: 0x8001_23D8,
        end_runtime: 0x8001_23EC,
        source_text: "セブンリースの村",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::ZohDaimaohRuins,
        runtime: 0x8001_23EC,
        end_runtime: 0x8001_2400,
        source_text: "ぞう大魔王の遺跡",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::PakistaShop,
        runtime: 0x8001_2400,
        end_runtime: 0x8001_2410,
        source_text: "パキスタの店",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::PararaShop,
        runtime: 0x8001_2410,
        end_runtime: 0x8001_241C,
        source_text: "パララの店",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::SasorimanShop,
        runtime: 0x8001_241C,
        end_runtime: QUEST_WORLD_MAP_LABEL_TABLE_END_RUNTIME,
        source_text: "さそりまんの店",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::ChoiceYes,
        runtime: QUEST_UI_CHOICE_YES_RUNTIME,
        end_runtime: QUEST_UI_CHOICE_NO_RUNTIME,
        source_text: "はい",
    },
    LiteralSpec {
        role: QuestUiLiteralRole::ChoiceNo,
        runtime: QUEST_UI_CHOICE_NO_RUNTIME,
        end_runtime: QUEST_UI_CHOICE_TABLE_END_RUNTIME,
        source_text: "いいえ",
    },
];

#[derive(Debug, Clone, Copy)]
struct MapReferenceSpec {
    state: u8,
    role: QuestUiLiteralRole,
    high_runtime: u32,
    jump_runtime: Option<u32>,
    low_runtime: u32,
}

const MAP_REFERENCE_SPECS: [MapReferenceSpec; 12] = [
    MapReferenceSpec {
        state: 0,
        role: QuestUiLiteralRole::PlumberVillage,
        high_runtime: 0x8006_6318,
        jump_runtime: Some(0x8006_631C),
        low_runtime: 0x8006_6320,
    },
    MapReferenceSpec {
        state: 1,
        role: QuestUiLiteralRole::SkeletonTCave,
        high_runtime: 0x8006_6328,
        jump_runtime: Some(0x8006_632C),
        low_runtime: 0x8006_6330,
    },
    MapReferenceSpec {
        state: 2,
        role: QuestUiLiteralRole::NeighborTownBridge,
        high_runtime: 0x8006_6338,
        jump_runtime: Some(0x8006_633C),
        low_runtime: 0x8006_6340,
    },
    MapReferenceSpec {
        state: 3,
        role: QuestUiLiteralRole::CherrybellTown,
        high_runtime: 0x8006_6348,
        jump_runtime: Some(0x8006_634C),
        low_runtime: 0x8006_6350,
    },
    MapReferenceSpec {
        state: 4,
        role: QuestUiLiteralRole::HappinessTower,
        high_runtime: 0x8006_6358,
        jump_runtime: Some(0x8006_635C),
        low_runtime: 0x8006_6360,
    },
    MapReferenceSpec {
        state: 5,
        role: QuestUiLiteralRole::HarpyForest,
        high_runtime: 0x8006_6368,
        jump_runtime: Some(0x8006_636C),
        low_runtime: 0x8006_6370,
    },
    MapReferenceSpec {
        state: 7,
        role: QuestUiLiteralRole::MountainPassCave,
        high_runtime: 0x8006_6388,
        jump_runtime: Some(0x8006_638C),
        low_runtime: 0x8006_6390,
    },
    MapReferenceSpec {
        state: 8,
        role: QuestUiLiteralRole::SevenleaseVillage,
        high_runtime: 0x8006_6398,
        jump_runtime: Some(0x8006_639C),
        low_runtime: 0x8006_63A0,
    },
    MapReferenceSpec {
        state: 9,
        role: QuestUiLiteralRole::ZohDaimaohRuins,
        high_runtime: 0x8006_63A8,
        jump_runtime: Some(0x8006_63AC),
        low_runtime: 0x8006_63B0,
    },
    MapReferenceSpec {
        state: 10,
        role: QuestUiLiteralRole::PakistaShop,
        high_runtime: 0x8006_63B8,
        jump_runtime: Some(0x8006_63BC),
        low_runtime: 0x8006_63C0,
    },
    MapReferenceSpec {
        state: 11,
        role: QuestUiLiteralRole::PararaShop,
        high_runtime: 0x8006_63C8,
        jump_runtime: Some(0x8006_63CC),
        low_runtime: 0x8006_63D0,
    },
    MapReferenceSpec {
        state: 12,
        role: QuestUiLiteralRole::SasorimanShop,
        high_runtime: 0x8006_63D8,
        jump_runtime: None,
        low_runtime: 0x8006_63DC,
    },
];

#[derive(Debug, Clone, Copy)]
struct ChoiceReferenceSpec {
    yes_high_runtime: u32,
    yes_register: Reg,
    yes_glyph_call_runtime: u32,
    next_line_runtime: u32,
    no_high_runtime: u32,
    no_register: Reg,
    no_glyph_call_runtime: u32,
}

const CHOICE_REFERENCE_SPECS: [ChoiceReferenceSpec; 4] = [
    ChoiceReferenceSpec {
        yes_high_runtime: 0x8003_5F28,
        yes_register: Reg::S2,
        yes_glyph_call_runtime: 0x8003_5F54,
        next_line_runtime: 0x8003_5FB8,
        no_high_runtime: 0x8003_5FC0,
        no_register: Reg::S2,
        no_glyph_call_runtime: 0x8003_5FE8,
    },
    ChoiceReferenceSpec {
        yes_high_runtime: 0x8003_7704,
        yes_register: Reg::S3,
        yes_glyph_call_runtime: 0x8003_7730,
        next_line_runtime: 0x8003_77B4,
        no_high_runtime: 0x8003_77BC,
        no_register: Reg::S3,
        no_glyph_call_runtime: 0x8003_77E4,
    },
    ChoiceReferenceSpec {
        yes_high_runtime: 0x8003_7D7C,
        yes_register: Reg::S2,
        yes_glyph_call_runtime: 0x8003_7DA8,
        next_line_runtime: 0x8003_7E0C,
        no_high_runtime: 0x8003_7E14,
        no_register: Reg::S2,
        no_glyph_call_runtime: 0x8003_7E3C,
    },
    ChoiceReferenceSpec {
        yes_high_runtime: 0x8003_9AD8,
        yes_register: Reg::S2,
        yes_glyph_call_runtime: 0x8003_9B04,
        next_line_runtime: 0x8003_9B68,
        no_high_runtime: 0x8003_9B70,
        no_register: Reg::S2,
        no_glyph_call_runtime: 0x8003_9B98,
    },
];

/// 원본에서 검증한 한 UI literal slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteral {
    pub role: QuestUiLiteralRole,
    pub surface: QuestUiLiteralSurface,
    pub runtime: u32,
    pub end_runtime: u32,
    pub source_text: String,
    pub source_slot: Vec<u8>,
}

/// 지명·공용 선택지·메뉴 literal과 실제 renderer 소비 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralSummary {
    pub literals: Vec<QuestUiLiteral>,
    pub world_map_labels: usize,
    pub binary_choices: usize,
    pub source_bytes: usize,
    pub world_map_state_values: Vec<u8>,
    pub world_map_dispatch_table_runtime: u32,
    pub world_map_whole_string_call_runtime: u32,
    pub binary_choice_reference_pairs: usize,
    pub binary_choice_direct_glyph_call_runtimes: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralError(String);

impl fmt::Display for QuestUiLiteralError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestUiLiteralError {}

fn error(message: impl Into<String>) -> QuestUiLiteralError {
    QuestUiLiteralError(message.into())
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), QuestUiLiteralError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "QUEST UI literal 명령 주소 사상 실패: 0x{runtime:08X}"
        ))
    })?;
    let raw = text
        .get(offset..offset + 4)
        .ok_or_else(|| error(format!("QUEST UI literal 명령 범위 이탈: 0x{runtime:08X}")))?;
    let actual = decode(
        runtime,
        u32::from_le_bytes(raw.try_into().expect("4바이트 명령")),
    );
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST UI literal typed 명령 불일치: 0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn literal_runtime(role: QuestUiLiteralRole) -> u32 {
    LITERAL_SPECS
        .iter()
        .find(|spec| spec.role == role)
        .expect("고정 UI literal 역할")
        .runtime
}

fn address_load_references(
    map: &ExeMap,
    text: &[u8],
    target: u32,
) -> Result<Vec<(u32, u32)>, QuestUiLiteralError> {
    let instructions = decode_slice(map.load_addr, text)
        .ok_or_else(|| error("QUEST UI literal text가 4바이트 정렬이 아님"))?;
    let mut references = find_address_loads(&instructions, DEFAULT_WINDOW)
        .into_iter()
        .filter(|load| load.value == target)
        .map(|load| (load.lui_addr, load.combine_addr))
        .collect::<Vec<_>>();
    references.sort_unstable();
    references.dedup();
    Ok(references)
}

fn raw_pointer_locations(map: &ExeMap, text: &[u8], target: u32) -> Vec<u32> {
    let needle = target.to_le_bytes();
    text.windows(needle.len())
        .enumerate()
        .filter_map(|(offset, window)| {
            if window != needle {
                return None;
            }
            map.load_addr.checked_add(u32::try_from(offset).ok()?)
        })
        .collect()
}

fn verify_literal_slots(
    map: &ExeMap,
    text: &[u8],
) -> Result<Vec<QuestUiLiteral>, QuestUiLiteralError> {
    let mut roles = BTreeSet::new();
    let mut runtimes = BTreeSet::new();
    let mut literals = Vec::with_capacity(LITERAL_SPECS.len());
    for spec in
        LITERAL_SPECS
            .into_iter()
            .chain(QUEST_MENU_LITERAL_SPECS.iter().map(
                |&(role, runtime, end_runtime, source_text)| LiteralSpec {
                    role: QuestUiLiteralRole::MenuText(role),
                    runtime,
                    end_runtime,
                    source_text,
                },
            ))
    {
        let start = map
            .text_offset(spec.runtime)
            .ok_or_else(|| error(format!("{}: UI literal 시작 사상 실패", spec.role.label())))?;
        let end = map
            .text_offset(spec.end_runtime - 1)
            .and_then(|offset| offset.checked_add(1))
            .ok_or_else(|| error(format!("{}: UI literal 끝 사상 실패", spec.role.label())))?;
        let actual = text
            .get(start..end)
            .ok_or_else(|| error(format!("{}: UI literal slot 범위 이탈", spec.role.label())))?;
        let encoded = encode_sjis(spec.source_text).map_err(|source| {
            error(format!(
                "{}: UI literal 기준문 Shift-JIS 변환 실패: {source}",
                spec.role.label()
            ))
        })?;
        let mut expected = encoded;
        expected.push(0);
        if expected.len() > actual.len() {
            return Err(error(format!(
                "{}: UI literal 기준문이 slot을 넘음",
                spec.role.label()
            )));
        }
        expected.resize(actual.len(), 0);
        if actual != expected
            || !roles.insert(spec.role)
            || !runtimes.insert(spec.runtime)
            || spec.role.surface() == QuestUiLiteralSurface::WorldMapLabel
                && !(QUEST_WORLD_MAP_LABEL_TABLE_RUNTIME <= spec.runtime
                    && spec.end_runtime <= QUEST_WORLD_MAP_LABEL_TABLE_END_RUNTIME)
        {
            return Err(error(format!(
                "{}: QUEST UI literal 원본 slot/역할 경계 불일치",
                spec.role.label()
            )));
        }
        literals.push(QuestUiLiteral {
            role: spec.role,
            surface: spec.role.surface(),
            runtime: spec.runtime,
            end_runtime: spec.end_runtime,
            source_text: spec.source_text.to_owned(),
            source_slot: actual.to_vec(),
        });
    }
    Ok(literals)
}

fn verify_world_map_consumer(map: &ExeMap, text: &[u8]) -> Result<Vec<u8>, QuestUiLiteralError> {
    for (runtime, expected) in [
        (
            QUEST_WORLD_MAP_LABEL_STATE_RUNTIME,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xA4,
            },
        ),
        (
            0x8006_62EC,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 13,
            },
        ),
        (
            0x8006_62F4,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        (
            0x8006_62F8,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_WORLD_MAP_LABEL_DISPATCH_TABLE_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8006_6304,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (0x8006_630C, K::Jr { rs: Reg::V0 }),
        (
            QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
            K::Jal {
                target: QUEST_WHOLE_STRING_RENDERER_RUNTIME,
            },
        ),
        (
            QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME + 4,
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
    ] {
        require_instruction(map, text, runtime, expected)?;
    }

    let mut states = Vec::with_capacity(MAP_REFERENCE_SPECS.len());
    for reference in MAP_REFERENCE_SPECS {
        let target = literal_runtime(reference.role);
        require_instruction(
            map,
            text,
            reference.high_runtime - 4,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: 9,
            },
        )?;
        require_instruction(
            map,
            text,
            reference.high_runtime,
            K::Lui {
                rt: Reg::A1,
                imm: (target >> 16) as u16,
            },
        )?;
        if let Some(jump_runtime) = reference.jump_runtime {
            require_instruction(
                map,
                text,
                jump_runtime,
                K::J {
                    target: QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
                },
            )?;
        }
        require_instruction(
            map,
            text,
            reference.low_runtime,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: target as u16 as i16,
            },
        )?;
        let references = address_load_references(map, text, target)?;
        let raw_pointers = raw_pointer_locations(map, text, target);
        if references != [(reference.high_runtime, reference.low_runtime)]
            || !raw_pointers.is_empty()
        {
            return Err(error(format!(
                "{}: world-map literal 정적 참조 분모 불일치: address={references:08X?}, raw={raw_pointers:08X?}",
                reference.role.label()
            )));
        }
        states.push(reference.state);
    }
    Ok(states)
}

fn verify_choice_literal_load(
    map: &ExeMap,
    text: &[u8],
    high_runtime: u32,
    register: Reg,
    target: u32,
) -> Result<(), QuestUiLiteralError> {
    require_instruction(
        map,
        text,
        high_runtime,
        K::Lui {
            rt: Reg::V0,
            imm: 0x800C,
        },
    )?;
    require_instruction(
        map,
        text,
        high_runtime + 4,
        K::Addiu {
            rt: register,
            rs: Reg::V0,
            imm: target as u16 as i16,
        },
    )?;
    require_instruction(
        map,
        text,
        high_runtime + 12,
        K::Jal {
            target: QUEST_STRING_LENGTH_RUNTIME,
        },
    )?;
    require_instruction(
        map,
        text,
        high_runtime + 16,
        K::Addu {
            rd: Reg::A0,
            rs: register,
            rt: Reg::ZERO,
        },
    )
}

fn verify_binary_choice_consumers(
    map: &ExeMap,
    text: &[u8],
) -> Result<Vec<u32>, QuestUiLiteralError> {
    let yes_references = address_load_references(map, text, QUEST_UI_CHOICE_YES_RUNTIME)?;
    let no_references = address_load_references(map, text, QUEST_UI_CHOICE_NO_RUNTIME)?;
    let yes_raw_pointers = raw_pointer_locations(map, text, QUEST_UI_CHOICE_YES_RUNTIME);
    let no_raw_pointers = raw_pointer_locations(map, text, QUEST_UI_CHOICE_NO_RUNTIME);
    let expected_yes = CHOICE_REFERENCE_SPECS
        .iter()
        .map(|spec| (spec.yes_high_runtime, spec.yes_high_runtime + 4))
        .collect::<Vec<_>>();
    let expected_no = CHOICE_REFERENCE_SPECS
        .iter()
        .map(|spec| (spec.no_high_runtime, spec.no_high_runtime + 4))
        .collect::<Vec<_>>();
    if yes_references != expected_yes
        || no_references != expected_no
        || !yes_raw_pointers.is_empty()
        || !no_raw_pointers.is_empty()
    {
        return Err(error(format!(
            "QUEST 공용 선택지 정적 참조 분모 불일치: yes={yes_references:08X?}, no={no_references:08X?}, yes_raw={yes_raw_pointers:08X?}, no_raw={no_raw_pointers:08X?}"
        )));
    }

    let mut calls = Vec::with_capacity(QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES.len());
    for spec in CHOICE_REFERENCE_SPECS {
        verify_choice_literal_load(
            map,
            text,
            spec.yes_high_runtime,
            spec.yes_register,
            QUEST_UI_CHOICE_YES_RUNTIME,
        )?;
        verify_choice_literal_load(
            map,
            text,
            spec.no_high_runtime,
            spec.no_register,
            QUEST_UI_CHOICE_NO_RUNTIME,
        )?;
        require_instruction(
            map,
            text,
            spec.next_line_runtime,
            K::Jal {
                target: QUEST_TEXT_WINDOW_NEXT_LINE_RUNTIME,
            },
        )?;
        require_instruction(
            map,
            text,
            spec.next_line_runtime + 4,
            K::Addu {
                rd: Reg::S0,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        )?;
        for runtime in [spec.yes_glyph_call_runtime, spec.no_glyph_call_runtime] {
            require_instruction(
                map,
                text,
                runtime,
                K::Jal {
                    target: QUEST_GLYPH_RENDERER_RUNTIME,
                },
            )?;
            require_instruction(
                map,
                text,
                runtime + 4,
                K::Addiu {
                    rt: Reg::S0,
                    rs: Reg::S0,
                    imm: 2,
                },
            )?;
            calls.push(runtime);
        }
    }
    if calls != QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES {
        return Err(error("QUEST 공용 선택지 direct glyph site 순서 drift"));
    }
    Ok(calls)
}

/// 지원 원본의 world-map 지명 12개와 공용 선택지 2개를 전수 검증한다.
pub fn verify_box_quest_ui_literals(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestUiLiteralSummary, QuestUiLiteralError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error("QUEST UI literal EXE 이름/길이 불일치"));
    }
    let literals = verify_literal_slots(map, text)?;
    let world_map_state_values = verify_world_map_consumer(map, text)?;
    let binary_choice_direct_glyph_call_runtimes = verify_binary_choice_consumers(map, text)?;
    let world_map_labels = literals
        .iter()
        .filter(|literal| literal.surface == QuestUiLiteralSurface::WorldMapLabel)
        .count();
    let binary_choices = literals
        .iter()
        .filter(|l| l.surface == QuestUiLiteralSurface::BinaryChoice)
        .count();
    menu::verify_menu_consumers(map, text)?;
    let source_bytes = literals
        .iter()
        .map(|literal| literal.source_slot.len())
        .sum();
    if literals.len() != LITERAL_SPECS.len() + QUEST_MENU_LITERAL_COUNT
        || world_map_labels != MAP_REFERENCE_SPECS.len()
        || binary_choices != 2
        || world_map_state_values.len() != world_map_labels
        || binary_choice_direct_glyph_call_runtimes.len()
            != QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES.len()
    {
        return Err(error("QUEST UI literal 최종 분모 drift"));
    }
    Ok(QuestUiLiteralSummary {
        literals,
        world_map_labels,
        binary_choices,
        source_bytes,
        world_map_state_values,
        world_map_dispatch_table_runtime: QUEST_WORLD_MAP_LABEL_DISPATCH_TABLE_RUNTIME,
        world_map_whole_string_call_runtime: QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
        binary_choice_reference_pairs: CHOICE_REFERENCE_SPECS.len(),
        binary_choice_direct_glyph_call_runtimes,
    })
}

#[cfg(test)]
#[path = "quest_ui_literals_tests.rs"]
mod tests;
