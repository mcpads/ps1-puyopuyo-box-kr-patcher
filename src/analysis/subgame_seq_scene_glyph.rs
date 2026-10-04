//! scene-local `E1/E2` pair table과 BTX 자원 선택의 전수 정적 경계.
//!
//! PUYO1/PUYO2의 17개 장면은 `E2`로 main/auxiliary resource를 기록한 뒤
//! `E1`으로 scene-local 2바이트 pair source를 등록한다. HAP/SAT2는 고유
//! glyph prefix 바로 뒤에 첫 E0 message가 겹쳐 있으므로 suffix를 원문 그대로 보호한다. 공용 tokenizer와
//! short renderer 경로는 `subgame_seq_drac_glyph`에서 typed MIPS로 검증했다.
//! 이 모듈은 그 코드 계약을 34개 물리 SEQ의 실제 record, cell table,
//! descriptor, VDD/CNX/BTX 원문과 교차한다. 장면명과 같은 BTX는 비교
//! 후보일 뿐이며, `E2`가 고른 자산과 같다고 가정하지 않는다. VRAM upload,
//! draw hit와 실제 화면의 pixel source는 별도 runtime gate다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::{decompress, parse_header};
use crate::coords::{ExeMap, RuntimeAssetMap};
use crate::disasm::{
    BtxNameAnalysis, SubgameSeqDracConsumerAnalysis, SubgameSeqDracGlyphAnalysis,
    BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES, BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT,
    BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH, BOX_SUBGAME_SEQ_DRAC_CELL_BASE,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS, BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y, BOX_SUBGAME_SEQ_DRAC_CELL_HEADER_BYTES,
    BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT, BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE,
    BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET, BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME,
    BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH, BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE,
    BOX_SUBGAME_SEQ_DRAC_MAIN_GP_ACCESSES, BOX_SUBGAME_SEQ_DRAC_MAIN_RESOURCE_BRANCH_SITES,
};
use crate::script::{
    SubgameSeqLayoutAnalysis, SubgameSeqTextPointerEvidence, SubgameSeqTextRootAnalysis,
};
use crate::text::decode_sjis;

pub const SUBGAME_SEQ_SCENE_GLYPH_SCHEMA_VERSION: &str = "subgame-seq-scene-glyph-v2";
pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_ROWS: usize = 34;
pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_FAMILIES: usize = 17;
pub const BOX_SUBGAME_SEQ_SCENE_REPERTOIRE_SLOTS: usize = 1_556;
pub const BOX_SUBGAME_SEQ_SCENE_STRICT_SJIS_PAIRS: usize = 1_530;
pub const BOX_SUBGAME_SEQ_SCENE_OPAQUE_PAIRS: usize = 26;
pub const BOX_SUBGAME_SEQ_SCENE_GRID_SLOTS: usize = 1_522;
pub const BOX_SUBGAME_SEQ_SCENE_REPERTOIRE_COPY_MATCHES: usize = 17;
pub const BOX_SUBGAME_SEQ_SCENE_SAME_STEM_COPY_MATCHES: usize = 16;
pub const BOX_SUBGAME_SEQ_SCENE_PUYO1_SELECTED_SAME_STEM_ROWS: usize = 17;
pub const BOX_SUBGAME_SEQ_SCENE_PUYO2_SELECTED_SAME_STEM_ROWS: usize = 0;
pub const BOX_SUBGAME_SEQ_SCENE_PUYO1_SELECTED_ATLAS_ROWS: usize = 17;
pub const BOX_SUBGAME_SEQ_SCENE_PUYO2_SELECTED_ATLAS_ROWS: usize = 16;
pub const BOX_SUBGAME_SEQ_SCENE_PUYO1_SELECTED_SAME_STEM_CELLS: usize = 761;
pub const BOX_SUBGAME_SEQ_SCENE_PUYO2_SELECTED_SAME_STEM_CELLS: usize = 69;
pub const BOX_SUBGAME_SEQ_SCENE_CANONICAL_GLYPH_SLOTS: usize = 1_432;
pub const BOX_SUBGAME_SEQ_SCENE_PROTECTED_MESSAGE_SLOTS: usize = 124;
pub const BOX_SUBGAME_SEQ_SCENE_COMPOUND_ROWS: usize = 4;
pub const BOX_SUBGAME_SEQ_SCENE_COMPOUND_MESSAGE_RECORDS: usize = 26;
pub const BOX_SUBGAME_SEQ_SCENE_COMPOUND_GLYPH_TOKENS: usize = 394;
pub const BOX_SUBGAME_SEQ_SCENE_COMPOUND_CONTROL_TOKENS: usize = 78;
pub const BOX_SUBGAME_SEQ_SCENE_COMPOUND_VARIANT_RECORDS: usize = 14;

/// 지원 원본에서 생성한 34행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_REPORT_SHA256: [u8; 32] = [
    0xD0, 0x0E, 0x33, 0x25, 0x03, 0x13, 0xF6, 0x40, 0x54, 0x3D, 0x3A, 0xA2, 0x5B, 0x0B, 0xE7, 0xBB,
    0x37, 0x48, 0x03, 0xD5, 0xC4, 0xE6, 0x37, 0x36, 0x76, 0xEC, 0x3D, 0xAF, 0x46, 0x88, 0x42, 0x2B,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqSceneFamilySpec {
    pub sequence: &'static str,
    pub same_stem_asset: &'static str,
    pub repertoire_bytes: usize,
    pub grid_slots: usize,
    pub puyo1_resource_id: u16,
    pub puyo2_resource_id: u16,
}

pub const BOX_SUBGAME_SEQ_SCENE_FAMILIES: [SubgameSeqSceneFamilySpec;
    BOX_SUBGAME_SEQ_SCENE_GLYPH_FAMILIES] = [
    SubgameSeqSceneFamilySpec {
        sequence: "SKEL_SEQ.SEQ",
        same_stem_asset: "MZ_SKEL.BTX",
        repertoire_bytes: 84,
        grid_slots: 42,
        puyo1_resource_id: 0x005A,
        puyo2_resource_id: 0x0089,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "NAS_SEQ.SEQ",
        same_stem_asset: "MZ_NAS.BTX",
        repertoire_bytes: 32,
        grid_slots: 16,
        puyo1_resource_id: 0x005B,
        puyo2_resource_id: 0x008A,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "MUMY_SEQ.SEQ",
        same_stem_asset: "MZ_MUMY.BTX",
        repertoire_bytes: 68,
        grid_slots: 34,
        puyo1_resource_id: 0x005C,
        puyo2_resource_id: 0x008B,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "DRAC_SEQ.SEQ",
        same_stem_asset: "MZ_DRA.BTX",
        repertoire_bytes: 112,
        grid_slots: 56,
        puyo1_resource_id: 0x005D,
        puyo2_resource_id: 0x008C,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "TARA_SEQ.SEQ",
        same_stem_asset: "MZ_TARA.BTX",
        repertoire_bytes: 48,
        grid_slots: 24,
        puyo1_resource_id: 0x005E,
        puyo2_resource_id: 0x008D,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "POTE_SEQ.SEQ",
        same_stem_asset: "MZ_POTE.BTX",
        repertoire_bytes: 82,
        grid_slots: 41,
        puyo1_resource_id: 0x0060,
        puyo2_resource_id: 0x008F,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "HAP_SEQ.SEQ",
        same_stem_asset: "MZ_HAP.BTX",
        repertoire_bytes: 104,
        grid_slots: 52,
        puyo1_resource_id: 0x005F,
        puyo2_resource_id: 0x008E,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "SCO_SEQ.SEQ",
        same_stem_asset: "MZ_SCO.BTX",
        repertoire_bytes: 78,
        grid_slots: 39,
        puyo1_resource_id: 0x0062,
        puyo2_resource_id: 0x0091,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "PANO_SEQ.SEQ",
        same_stem_asset: "MZ_PANO.BTX",
        repertoire_bytes: 92,
        grid_slots: 46,
        puyo1_resource_id: 0x0061,
        puyo2_resource_id: 0x0090,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "ZON_SEQ.SEQ",
        same_stem_asset: "MZ_ZON.BTX",
        repertoire_bytes: 44,
        grid_slots: 22,
        puyo1_resource_id: 0x0063,
        puyo2_resource_id: 0x0092,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "WIT_SEQ.SEQ",
        same_stem_asset: "MZ_WIT.BTX",
        repertoire_bytes: 86,
        grid_slots: 43,
        puyo1_resource_id: 0x0064,
        puyo2_resource_id: 0x0093,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "ZOH_SEQ.SEQ",
        same_stem_asset: "MZ_ZOH.BTX",
        repertoire_bytes: 106,
        grid_slots: 53,
        puyo1_resource_id: 0x0065,
        puyo2_resource_id: 0x0094,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "SHE_SEQ.SEQ",
        same_stem_asset: "MZ_SHE.BTX",
        repertoire_bytes: 118,
        grid_slots: 59,
        puyo1_resource_id: 0x0066,
        puyo2_resource_id: 0x0095,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "MIN_SEQ.SEQ",
        same_stem_asset: "MZ_MIN.BTX",
        repertoire_bytes: 98,
        grid_slots: 49,
        puyo1_resource_id: 0x0067,
        puyo2_resource_id: 0x0096,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "LUL_SEQ.SEQ",
        same_stem_asset: "MZ_LUL.BTX",
        repertoire_bytes: 124,
        grid_slots: 62,
        puyo1_resource_id: 0x0068,
        puyo2_resource_id: 0x0097,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "SAT1_SEQ.SEQ",
        same_stem_asset: "MZ_SAT1.BTX",
        repertoire_bytes: 86,
        grid_slots: 43,
        puyo1_resource_id: 0x0069,
        puyo2_resource_id: 0x0098,
    },
    SubgameSeqSceneFamilySpec {
        sequence: "SAT2_SEQ.SEQ",
        same_stem_asset: "MZ_SAT2.BTX",
        repertoire_bytes: 194,
        grid_slots: 80,
        puyo1_resource_id: 0x006A,
        puyo2_resource_id: 0x0099,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqSceneCompoundSpec {
    pub sequence: &'static str,
    pub canonical_glyph_slots: usize,
    pub message_records: usize,
    pub glyph_tokens: usize,
    pub control_tokens: usize,
    pub variant_records: usize,
    pub embedded_header: [u8; 4],
}

/// 고유 glyph prefix 바로 뒤에 첫 E0 message가 겹쳐 놓인 두 family.
pub const BOX_SUBGAME_SEQ_SCENE_COMPOUND_SPECS: [SubgameSeqSceneCompoundSpec; 2] = [
    SubgameSeqSceneCompoundSpec {
        sequence: "HAP_SEQ.SEQ",
        canonical_glyph_slots: 23,
        message_records: 2,
        glyph_tokens: 35,
        control_tokens: 7,
        variant_records: 2,
        embedded_header: [0x04, 0x09, 0x0A, 0x03],
    },
    SubgameSeqSceneCompoundSpec {
        sequence: "SAT2_SEQ.SEQ",
        canonical_glyph_slots: 64,
        message_records: 11,
        glyph_tokens: 162,
        control_tokens: 32,
        variant_records: 5,
        embedded_header: [0x0D, 0x06, 0x08, 0x04],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphScopeSpec {
    pub scope: &'static str,
    pub exe: &'static str,
    pub seq_archive: &'static str,
    pub grp_archive: &'static str,
    pub descriptor_base_runtime: u32,
    pub auxiliary_id: u16,
}

pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES: [SubgameSeqSceneGlyphScopeSpec; 2] = [
    SubgameSeqSceneGlyphScopeSpec {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        seq_archive: "/PUYO1/PP1_SEQ.VDD;1",
        grp_archive: "/PUYO1/PP1_GRP.VDD;1",
        descriptor_base_runtime: 0x8006_7E48,
        auxiliary_id: 0x0059,
    },
    SubgameSeqSceneGlyphScopeSpec {
        scope: "PUYO2",
        exe: "PUYO2.EXE",
        seq_archive: "/PUYO2/PP2_SEQ.VDD;1",
        grp_archive: "/PUYO2/PP2_GRP.VDD;1",
        descriptor_base_runtime: 0x8006_3E9C,
        auxiliary_id: 0x0088,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphRow {
    pub scope: &'static str,
    pub exe: &'static str,
    pub seq_archive: &'static str,
    pub grp_archive: &'static str,
    pub sequence: &'static str,
    pub sequence_sha256: [u8; 32],
    pub e2_record_offset: usize,
    pub e1_record_offset: usize,
    pub repertoire_offset: usize,
    pub repertoire_runtime: u32,
    pub repertoire_bytes: usize,
    pub repertoire_slots: usize,
    pub strict_sjis_pairs: usize,
    pub opaque_pairs: usize,
    pub repertoire_unique_pairs: usize,
    pub repertoire_sha256: [u8; 32],
    pub repertoire_copy_equal: bool,
    pub canonical_glyph_slots: usize,
    pub protected_message_slots: usize,
    pub compound_message_records: usize,
    pub compound_glyph_tokens: usize,
    pub compound_control_tokens: usize,
    pub compound_variant_records: usize,
    pub embedded_message_record_offset: Option<usize>,
    pub resource_id: u16,
    pub auxiliary_id: u16,
    pub auxiliary_asset: String,
    pub cell_setup_record_offset: usize,
    pub cell_table_offset: usize,
    pub cell_table_runtime: u32,
    pub cell_base: u16,
    pub cell_last: u16,
    pub grid_slots: usize,
    pub selected_asset: String,
    pub selected_vdd_index: usize,
    pub selected_stored_bytes: u32,
    pub selected_cnx_subtype: [u8; 3],
    pub selected_plain_bytes: usize,
    pub selected_plain_sha256: [u8; 32],
    pub selected_atlas_compatible: bool,
    pub same_stem_asset: &'static str,
    pub same_stem_resource_id: u16,
    pub same_stem_vdd_index: usize,
    pub same_stem_stored_bytes: u32,
    pub same_stem_cnx_subtype: [u8; 3],
    pub same_stem_plain_bytes: usize,
    pub same_stem_plain_sha256: [u8; 32],
    pub selected_is_same_stem: bool,
    pub selected_same_stem_equal_grid_cells: usize,
    pub same_stem_copy_equal: bool,
    pub e1_handler_runtime: u32,
    pub e2_handler_runtime: u32,
    pub pair_lookup_runtime: u32,
    pub renderer_runtime: u32,
}

impl SubgameSeqSceneGlyphRow {
    fn patch_strategy(&self) -> &'static str {
        if self.scope == "PUYO1"
            && self.canonical_glyph_slots <= self.grid_slots
            && self.selected_is_same_stem
        {
            "scene-local-canonical-pair-prefix-and-selected-btx-cell-rewrite-candidate-runtime-pending"
        } else if self.scope == "PUYO1" {
            "preserve-no-write-until-cell-tail-consumer-and-runtime-scene-contract"
        } else {
            "preserve-no-write-until-runtime-resolves-selected-vs-same-stem-resource"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphAnalysis {
    pub rows: Vec<SubgameSeqSceneGlyphRow>,
    pub families: usize,
    pub repertoire_slots: usize,
    pub strict_sjis_pairs: usize,
    pub opaque_pairs: usize,
    pub grid_slots: usize,
    pub repertoire_copy_matches: usize,
    pub same_stem_copy_matches: usize,
    pub puyo1_selected_same_stem_rows: usize,
    pub puyo2_selected_same_stem_rows: usize,
    pub puyo1_selected_atlas_rows: usize,
    pub puyo2_selected_atlas_rows: usize,
    pub puyo1_selected_same_stem_cells: usize,
    pub puyo2_selected_same_stem_cells: usize,
    pub canonical_glyph_slots: usize,
    pub protected_message_slots: usize,
    pub compound_rows: usize,
    pub compound_message_records: usize,
    pub compound_glyph_tokens: usize,
    pub compound_control_tokens: usize,
    pub compound_variant_records: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphError(String);

impl fmt::Display for SubgameSeqSceneGlyphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqSceneGlyphError {}

fn error(message: impl Into<String>) -> SubgameSeqSceneGlyphError {
    SubgameSeqSceneGlyphError(message.into())
}

#[derive(Debug)]
struct VerifiedAsset {
    name: String,
    resource_id: u16,
    vdd_index: usize,
    stored_bytes: u32,
    cnx_subtype: [u8; 3],
    plain_sha256: [u8; 32],
    plain: Vec<u8>,
}

#[derive(Debug)]
struct PendingRow {
    row: SubgameSeqSceneGlyphRow,
    repertoire: Vec<u8>,
    same_stem_plain: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedRepertoire {
    raw: Vec<u8>,
    strict_sjis_pairs: usize,
    opaque_pairs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCompoundMessages {
    canonical_glyph_slots: usize,
    protected_message_slots: usize,
    message_records: usize,
    glyph_tokens: usize,
    control_tokens: usize,
    variant_records: usize,
    embedded_message_record_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedMessage {
    end_offset: usize,
    glyph_tokens: usize,
    control_tokens: usize,
}

fn read_u16(raw: &[u8], offset: usize) -> Result<u16, SubgameSeqSceneGlyphError> {
    raw.get(offset..offset + 2)
        .map(|bytes| u16::from_le_bytes(bytes.try_into().expect("2바이트 halfword")))
        .ok_or_else(|| error(format!("scene glyph halfword 범위 이탈: +0x{offset:X}")))
}

fn read_i16(raw: &[u8], offset: usize) -> Result<i16, SubgameSeqSceneGlyphError> {
    read_u16(raw, offset).map(|value| value as i16)
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqSceneGlyphError> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("scene glyph word 범위 이탈: +0x{offset:X}")))
}

fn parse_repertoire(
    raw: &[u8],
    offset: usize,
    expected_bytes: usize,
) -> Result<ParsedRepertoire, SubgameSeqSceneGlyphError> {
    let end = offset
        .checked_add(expected_bytes)
        .ok_or_else(|| error("scene repertoire 끝 overflow"))?;
    let repertoire = raw
        .get(offset..end)
        .ok_or_else(|| error("scene repertoire 범위 이탈"))?
        .to_vec();
    if expected_bytes == 0
        || !expected_bytes.is_multiple_of(2)
        || raw.get(end..end + 2) != Some([0x65, 0x65].as_slice())
    {
        return Err(error("scene repertoire 길이/종결 불일치"));
    }
    let strict_sjis_pairs = repertoire
        .chunks_exact(2)
        .filter(|pair| decode_sjis(pair).is_ok_and(|decoded| decoded.chars().count() == 1))
        .count();
    let opaque_pairs = repertoire.len() / 2 - strict_sjis_pairs;
    Ok(ParsedRepertoire {
        raw: repertoire,
        strict_sjis_pairs,
        opaque_pairs,
    })
}

fn parse_compound_message(
    raw: &[u8],
    message_offset: usize,
    canonical_pairs: &[[u8; 2]],
) -> Result<ParsedMessage, SubgameSeqSceneGlyphError> {
    raw.get(message_offset..message_offset + 4)
        .ok_or_else(|| error("scene compound message header 범위 이탈"))?;
    let mut cursor = message_offset + 4;
    let mut glyph_tokens = 0;
    let mut control_tokens = 0;
    loop {
        if raw.get(cursor..cursor + 2) == Some([0x65, 0x65].as_slice()) {
            return Ok(ParsedMessage {
                end_offset: cursor + 2,
                glyph_tokens,
                control_tokens,
            });
        }
        let first = *raw
            .get(cursor)
            .ok_or_else(|| error("scene compound message 종결 누락"))?;
        match first {
            0x71 | 0x77 => {
                raw.get(cursor + 1)
                    .ok_or_else(|| error("scene compound argument control 잘림"))?;
                control_tokens += 1;
                cursor += 2;
            }
            0x72 | 0x73 => {
                control_tokens += 1;
                cursor += 1;
            }
            _ => {
                let pair: [u8; 2] = raw
                    .get(cursor..cursor + 2)
                    .ok_or_else(|| error("scene compound glyph pair 잘림"))?
                    .try_into()
                    .expect("2바이트 pair");
                if decode_sjis(&pair).is_err() || !canonical_pairs.contains(&pair) {
                    return Err(error(format!(
                        "scene compound message의 canonical prefix 밖 glyph: {:02X}{:02X}",
                        pair[0], pair[1]
                    )));
                }
                glyph_tokens += 1;
                cursor += 2;
            }
        }
    }
}

fn verify_compound_messages(
    raw: &[u8],
    runtime_map: &RuntimeAssetMap,
    repertoire_offset: usize,
    repertoire: &ParsedRepertoire,
    spec: SubgameSeqSceneCompoundSpec,
) -> Result<ParsedCompoundMessages, SubgameSeqSceneGlyphError> {
    let prefix_bytes = spec
        .canonical_glyph_slots
        .checked_mul(2)
        .ok_or_else(|| error("scene canonical glyph prefix overflow"))?;
    let prefix = repertoire
        .raw
        .get(..prefix_bytes)
        .ok_or_else(|| error("scene canonical glyph prefix 범위 이탈"))?;
    let canonical_pairs = prefix
        .chunks_exact(2)
        .map(|pair| [pair[0], pair[1]])
        .collect::<Vec<_>>();
    if canonical_pairs.len() != spec.canonical_glyph_slots
        || canonical_pairs
            .iter()
            .any(|pair| !decode_sjis(pair).is_ok_and(|decoded| decoded.chars().count() == 1))
        || canonical_pairs.iter().collect::<BTreeSet<_>>().len() != canonical_pairs.len()
    {
        return Err(error(
            "scene canonical glyph prefix가 고유한 엄격 SJIS가 아님",
        ));
    }
    let embedded_offset = repertoire_offset
        .checked_add(prefix_bytes)
        .ok_or_else(|| error("scene embedded message offset overflow"))?;
    if raw.get(embedded_offset..embedded_offset + 4) != Some(spec.embedded_header.as_slice()) {
        return Err(error("scene embedded message header 불일치"));
    }

    let mut records = Vec::new();
    for record_offset in (0..raw.len().saturating_sub(8)).step_by(4) {
        let Some(record) = raw.get(record_offset..record_offset + 8) else {
            continue;
        };
        if record[0] != 0xE0 || record[1] > 1 || record[2..4] != [0, 0] {
            continue;
        }
        let pointer = u32::from_le_bytes(record[4..8].try_into().expect("4바이트 pointer"));
        let Some(message_offset) = runtime_map.file_offset(pointer) else {
            continue;
        };
        let message = parse_compound_message(raw, message_offset, &canonical_pairs)?;
        let pointer_fields = raw
            .windows(4)
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == &record[4..8]).then_some(offset))
            .collect::<Vec<_>>();
        if pointer_fields != [record_offset + 4] {
            return Err(error(format!(
                "scene compound message pointer 전수 불일치: record=+0x{record_offset:X}, fields={pointer_fields:X?}"
            )));
        }
        records.push((record_offset, message_offset, record[1], message));
    }
    let embedded = records
        .iter()
        .find(|(_, message_offset, _, _)| *message_offset == embedded_offset)
        .ok_or_else(|| error("scene embedded message direct E0 pointer 누락"))?;
    let expected_embedded_end = repertoire_offset
        .checked_add(repertoire.raw.len())
        .and_then(|end| end.checked_add(2))
        .ok_or_else(|| error("scene embedded message end overflow"))?;
    if embedded.3.end_offset != expected_embedded_end {
        return Err(error(format!(
            "scene embedded message와 E1 pair source 끝 불일치: expected=+0x{expected_embedded_end:X}, actual=+0x{:X}",
            embedded.3.end_offset
        )));
    }
    let actual = (
        records.len(),
        records
            .iter()
            .map(|(_, _, _, message)| message.glyph_tokens)
            .sum::<usize>(),
        records
            .iter()
            .map(|(_, _, _, message)| message.control_tokens)
            .sum::<usize>(),
        records
            .iter()
            .filter(|(_, _, variant, _)| *variant == 1)
            .count(),
    );
    let expected = (
        spec.message_records,
        spec.glyph_tokens,
        spec.control_tokens,
        spec.variant_records,
    );
    if actual != expected {
        return Err(error(format!(
            "scene compound message 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    Ok(ParsedCompoundMessages {
        canonical_glyph_slots: spec.canonical_glyph_slots,
        protected_message_slots: repertoire.raw.len() / 2 - spec.canonical_glyph_slots,
        message_records: actual.0,
        glyph_tokens: actual.1,
        control_tokens: actual.2,
        variant_records: actual.3,
        embedded_message_record_offset: embedded_offset,
    })
}

fn verify_cell_layout(
    raw: &[u8],
    repertoire_slots: usize,
    expected_grid_slots: usize,
) -> Result<(), SubgameSeqSceneGlyphError> {
    let header = raw
        .get(
            BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET
                ..BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET + BOX_SUBGAME_SEQ_DRAC_CELL_HEADER_BYTES,
        )
        .ok_or_else(|| error("scene cell table header 범위 이탈"))?;
    let values = (
        read_u16(header, 0)?,
        read_u16(header, 2)?,
        read_u16(header, 4)?,
        read_u16(header, 6)?,
        read_i16(header, 8)?,
        read_i16(header, 10)?,
    );
    if values
        != (
            0,
            0,
            BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH,
            BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT,
            0,
            0,
        )
    {
        return Err(error(format!("scene cell table header 불일치: {values:?}")));
    }
    let mut grid_slots = 0usize;
    for index in 0..repertoire_slots {
        let cell = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_BASE) + index;
        let offset = BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET
            + BOX_SUBGAME_SEQ_DRAC_CELL_HEADER_BYTES
            + cell * BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE as usize;
        let actual = (
            read_u16(raw, offset)?,
            read_u16(raw, offset + 2)?,
            read_u16(raw, offset + 4)?,
            read_u16(raw, offset + 6)?,
            read_i16(raw, offset + 8)?,
            read_i16(raw, offset + 10)?,
        );
        let expected = (
            BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X
                + u16::try_from(index % BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS)
                    .map_err(|_| error("scene cell 열 변환 실패"))?
                    * BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH,
            BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y
                + u16::try_from(index / BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS)
                    .map_err(|_| error("scene cell 행 변환 실패"))?
                    * BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT,
            BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH,
            BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT,
            0,
            0,
        );
        if actual != expected {
            break;
        }
        grid_slots += 1;
    }
    if grid_slots != expected_grid_slots {
        return Err(error(format!(
            "scene cell grid prefix 불일치: expected={expected_grid_slots}, actual={grid_slots}"
        )));
    }
    Ok(())
}

fn grid_cell(plain: &[u8], index: usize) -> Result<Vec<&[u8]>, SubgameSeqSceneGlyphError> {
    if plain.len() != BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES {
        return Err(error("scene BTX atlas 길이 불일치"));
    }
    let row_bytes = usize::from(BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH) / 2;
    let cell_row_bytes = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH) / 2;
    let x = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X)
        + index % BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS
            * usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH);
    let y = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y)
        + index / BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS
            * usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT);
    (0..usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT))
        .map(|row| {
            let start = (y + row)
                .checked_mul(row_bytes)
                .and_then(|value| value.checked_add(x / 2))
                .ok_or_else(|| error("scene BTX cell offset overflow"))?;
            plain
                .get(start..start + cell_row_bytes)
                .ok_or_else(|| error("scene BTX cell 범위 이탈"))
        })
        .collect()
}

fn count_equal_grid_cells(
    selected: &[u8],
    same_stem: &[u8],
    grid_slots: usize,
) -> Result<usize, SubgameSeqSceneGlyphError> {
    let mut equal = 0usize;
    for index in 0..grid_slots {
        equal += usize::from(grid_cell(selected, index)? == grid_cell(same_stem, index)?);
    }
    Ok(equal)
}

fn load_asset_by_resource(
    scope: SubgameSeqSceneGlyphScopeSpec,
    resource_id: u16,
    map: &ExeMap,
    text: &[u8],
    archive: &VddArchive,
    btx_names: &BtxNameAnalysis,
) -> Result<VerifiedAsset, SubgameSeqSceneGlyphError> {
    let descriptor_runtime = scope
        .descriptor_base_runtime
        .checked_add(u32::from(resource_id) * BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE)
        .ok_or_else(|| error("scene descriptor 주소 overflow"))?;
    let pointer_runtime = descriptor_runtime
        .checked_add(4)
        .ok_or_else(|| error("scene descriptor pointer 주소 overflow"))?;
    let links = btx_names
        .links
        .iter()
        .filter(|link| {
            link.exe == scope.exe
                && link.archive == scope.grp_archive
                && link.descriptor_runtime_start == scope.descriptor_base_runtime
                && link.descriptor_pointer_runtime == pointer_runtime
        })
        .collect::<Vec<_>>();
    let [link] = links.as_slice() else {
        return Err(error(format!(
            "{} resource 0x{resource_id:04X} BTX 연결 수가 1이 아님: {}",
            scope.exe,
            links.len()
        )));
    };
    let matches = archive
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.name() == link.asset_name)
        .collect::<Vec<_>>();
    let [(vdd_index, entry)] = matches.as_slice() else {
        return Err(error(format!(
            "{}:{} VDD 행 수가 1이 아님: {}",
            scope.grp_archive,
            link.asset_name,
            matches.len()
        )));
    };
    let cnx = archive.read_file(entry);
    let header = parse_header(cnx).map_err(|source| {
        error(format!(
            "{}:{} CNX header 실패: {source}",
            scope.grp_archive, link.asset_name
        ))
    })?;
    let plain = decompress(cnx).map_err(|source| {
        error(format!(
            "{}:{} CNX 해제 실패: {source}",
            scope.grp_archive, link.asset_name
        ))
    })?;
    if plain.len() != header.decompressed_size as usize {
        return Err(error("scene BTX CNX 해제 길이 불일치"));
    }
    let descriptor_offset = map
        .text_offset(descriptor_runtime)
        .ok_or_else(|| error(format!("{} scene descriptor가 EXE 밖임", scope.exe)))?;
    let expected_words = [
        0x0001_0000,
        link.runtime_addr,
        entry.size,
        header.decompressed_size,
        0,
        0,
        0,
        0,
        0,
    ];
    for (index, expected) in expected_words.into_iter().enumerate() {
        let actual = read_u32(text, descriptor_offset + index * 4)?;
        if actual != expected {
            return Err(error(format!(
                "{}:{} descriptor word {index} 불일치: expected=0x{expected:08X}, actual=0x{actual:08X}",
                scope.exe, link.asset_name
            )));
        }
    }
    Ok(VerifiedAsset {
        name: link.asset_name.clone(),
        resource_id,
        vdd_index: *vdd_index,
        stored_bytes: entry.size,
        cnx_subtype: header.subtype,
        plain_sha256: Sha256::digest(&plain).into(),
        plain,
    })
}

fn asset_name_by_resource(
    scope: SubgameSeqSceneGlyphScopeSpec,
    resource_id: u16,
    btx_names: &BtxNameAnalysis,
) -> Result<String, SubgameSeqSceneGlyphError> {
    let pointer_runtime = scope
        .descriptor_base_runtime
        .checked_add(u32::from(resource_id) * BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE)
        .and_then(|runtime| runtime.checked_add(4))
        .ok_or_else(|| error("scene auxiliary descriptor 주소 overflow"))?;
    let links = btx_names
        .links
        .iter()
        .filter(|link| {
            link.exe == scope.exe
                && link.archive == scope.grp_archive
                && link.descriptor_runtime_start == scope.descriptor_base_runtime
                && link.descriptor_pointer_runtime == pointer_runtime
        })
        .collect::<Vec<_>>();
    let [link] = links.as_slice() else {
        return Err(error(format!(
            "{} auxiliary resource 0x{resource_id:04X} BTX 연결 수가 1이 아님: {}",
            scope.exe,
            links.len()
        )));
    };
    Ok(link.asset_name.clone())
}

fn load_asset_by_name(
    scope: SubgameSeqSceneGlyphScopeSpec,
    name: &str,
    map: &ExeMap,
    text: &[u8],
    archive: &VddArchive,
    btx_names: &BtxNameAnalysis,
) -> Result<VerifiedAsset, SubgameSeqSceneGlyphError> {
    let links = btx_names
        .links
        .iter()
        .filter(|link| {
            link.exe == scope.exe && link.archive == scope.grp_archive && link.asset_name == name
        })
        .collect::<Vec<_>>();
    let [link] = links.as_slice() else {
        return Err(error(format!(
            "{}:{name} same-stem BTX 연결 수가 1이 아님: {}",
            scope.exe,
            links.len()
        )));
    };
    let descriptor_runtime = link
        .descriptor_pointer_runtime
        .checked_sub(4)
        .ok_or_else(|| error("same-stem descriptor 주소 underflow"))?;
    let delta = descriptor_runtime
        .checked_sub(scope.descriptor_base_runtime)
        .ok_or_else(|| error("same-stem descriptor가 base 앞임"))?;
    if !delta.is_multiple_of(BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE) {
        return Err(error("same-stem descriptor stride 불일치"));
    }
    let resource_id = u16::try_from(delta / BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE)
        .map_err(|_| error("same-stem resource ID 변환 실패"))?;
    load_asset_by_resource(scope, resource_id, map, text, archive, btx_names)
}

fn expected_resource_id(
    scope: SubgameSeqSceneGlyphScopeSpec,
    family: SubgameSeqSceneFamilySpec,
) -> u16 {
    match scope.scope {
        "PUYO1" => family.puyo1_resource_id,
        "PUYO2" => family.puyo2_resource_id,
        _ => unreachable!("지원 scope는 두 개"),
    }
}

/// 34개 scene-local repertoire의 resource·cell·BTX 경계를 분석한다.
pub fn audit_subgame_seq_scene_glyphs<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    grp_archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
    layouts: &SubgameSeqLayoutAnalysis,
    roots: &SubgameSeqTextRootAnalysis,
    btx_names: &BtxNameAnalysis,
    shared_code: (
        &SubgameSeqDracConsumerAnalysis,
        &SubgameSeqDracGlyphAnalysis,
    ),
) -> Result<SubgameSeqSceneGlyphAnalysis, SubgameSeqSceneGlyphError> {
    let (drac_consumers, drac_glyphs) = shared_code;
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    let source_map = sources
        .into_iter()
        .map(|(archive, sequence, raw)| ((archive, sequence), raw))
        .collect::<BTreeMap<_, _>>();
    let archive_map = grp_archives.into_iter().collect::<BTreeMap<_, _>>();
    if executable_map.len() != BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES.len()
        || archive_map.len() != BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES.len()
    {
        return Err(error("scene glyph EXE/GRP 입력 집합 불일치"));
    }
    let family_names = BOX_SUBGAME_SEQ_SCENE_FAMILIES
        .iter()
        .map(|family| family.sequence)
        .collect::<BTreeSet<_>>();
    if family_names.len() != BOX_SUBGAME_SEQ_SCENE_FAMILIES.len() {
        return Err(error("scene glyph family spec 중복"));
    }

    let mut pending = Vec::with_capacity(BOX_SUBGAME_SEQ_SCENE_GLYPH_ROWS);
    let mut expected_root_fields = BTreeSet::new();
    for scope in BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES {
        let &(map, text) = executable_map
            .get(scope.exe)
            .ok_or_else(|| error(format!("{} scene glyph EXE 누락", scope.exe)))?;
        if text.len() != map.text_len as usize {
            return Err(error(format!("{} scene glyph text 길이 불일치", scope.exe)));
        }
        let archive = archive_map
            .get(scope.grp_archive)
            .copied()
            .ok_or_else(|| error(format!("{} scene glyph GRP 누락", scope.grp_archive)))?;
        let drac = drac_glyphs
            .rows
            .iter()
            .find(|row| row.scope == scope.scope)
            .ok_or_else(|| error(format!("{} shared DRAC glyph code 계약 누락", scope.scope)))?;
        let drac_consumer = drac_consumers
            .rows
            .iter()
            .find(|row| row.scope == scope.scope)
            .ok_or_else(|| {
                error(format!(
                    "{} shared DRAC token consumer 계약 누락",
                    scope.scope
                ))
            })?;
        if drac.exe != scope.exe
            || drac_consumer.exe != scope.exe
            || drac_consumer.body_tokenizer_runtime != drac.tokenizer_runtime
            || drac.main_resource_branch_sites != BOX_SUBGAME_SEQ_DRAC_MAIN_RESOURCE_BRANCH_SITES
            || drac.main_gp_accesses != BOX_SUBGAME_SEQ_DRAC_MAIN_GP_ACCESSES
            || drac.cell_base != BOX_SUBGAME_SEQ_DRAC_CELL_BASE
            || drac.cell_table_offset != BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET
            || drac.cell_table_runtime != BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME
            || drac.atlas_width != BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH
            || drac.atlas_height != BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT
        {
            return Err(error(format!(
                "{} shared DRAC glyph code 계약 불일치",
                scope.scope
            )));
        }
        let auxiliary_asset = asset_name_by_resource(scope, scope.auxiliary_id, btx_names)?;
        if auxiliary_asset != drac.auxiliary_asset {
            return Err(error(format!(
                "{} shared auxiliary asset 불일치: expected={}, actual={auxiliary_asset}",
                scope.scope, drac.auxiliary_asset
            )));
        }

        for family in BOX_SUBGAME_SEQ_SCENE_FAMILIES {
            let raw = source_map
                .get(&(scope.seq_archive, family.sequence))
                .copied()
                .ok_or_else(|| {
                    error(format!(
                        "{}:{} scene glyph SEQ 누락",
                        scope.seq_archive, family.sequence
                    ))
                })?;
            let layout = layouts
                .rows
                .iter()
                .find(|row| row.archive == scope.seq_archive && row.sequence == family.sequence)
                .ok_or_else(|| error("scene glyph layout 누락"))?;
            let sequence_sha256: [u8; 32] = Sha256::digest(raw).into();
            if layout.sequence_sha256 != sequence_sha256 || layout.bytes != raw.len() {
                return Err(error(format!(
                    "{} scene glyph layout 원본 불일치",
                    family.sequence
                )));
            }
            let runtime_map = layout
                .footer
                .ok_or_else(|| error("scene glyph overlay footer 누락"))?
                .runtime_map()
                .ok_or_else(|| error("scene glyph runtime 사상 overflow"))?;
            let records = (8..raw.len().saturating_sub(8))
                .step_by(4)
                .filter(|&e1_offset| {
                    raw.get(e1_offset..e1_offset + 4) == Some([0xE1, 0, 0, 0].as_slice())
                        && raw.get(e1_offset - 8) == Some(&0xE2)
                })
                .filter(|&e1_offset| {
                    let Ok(pointer) = read_u32(raw, e1_offset + 4) else {
                        return false;
                    };
                    runtime_map.file_offset(pointer).is_some_and(|target| {
                        roots.rows.iter().any(|row| {
                            row.archive == scope.seq_archive
                                && row.sequence == family.sequence
                                && row.candidate_offset == target
                                && row.evidence == SubgameSeqTextPointerEvidence::ExactStart
                                && row.roots.iter().any(|root| {
                                    root.candidate_delta == 0
                                        && root.pointer_field_offset == e1_offset + 4
                                })
                        })
                    })
                })
                .collect::<Vec<_>>();
            let [e1_record_offset] = records.as_slice() else {
                return Err(error(format!(
                    "{}:{} E1 exact-root record 수가 1이 아님: {}",
                    scope.scope,
                    family.sequence,
                    records.len()
                )));
            };
            let e1_record_offset = *e1_record_offset;
            let e2_record_offset = e1_record_offset - 8;
            let resource_id = read_u16(raw, e2_record_offset + 2)?;
            let auxiliary_id = read_u16(raw, e2_record_offset + 4)?;
            let expected_e2 = [
                0xE2,
                0,
                resource_id as u8,
                (resource_id >> 8) as u8,
                auxiliary_id as u8,
                (auxiliary_id >> 8) as u8,
                0,
                0,
            ];
            if raw.get(e2_record_offset..e1_record_offset) != Some(expected_e2.as_slice())
                || resource_id != expected_resource_id(scope, family)
                || auxiliary_id != scope.auxiliary_id
            {
                return Err(error(format!(
                    "{}:{} E2 record 불일치",
                    scope.scope, family.sequence
                )));
            }
            let repertoire_runtime = read_u32(raw, e1_record_offset + 4)?;
            let repertoire_offset = runtime_map
                .file_offset(repertoire_runtime)
                .ok_or_else(|| error("scene repertoire pointer가 overlay 밖임"))?;
            let repertoire = parse_repertoire(raw, repertoire_offset, family.repertoire_bytes)?;
            let compound_spec = BOX_SUBGAME_SEQ_SCENE_COMPOUND_SPECS
                .iter()
                .find(|spec| spec.sequence == family.sequence)
                .copied();
            let compound = if let Some(spec) = compound_spec {
                Some(verify_compound_messages(
                    raw,
                    &runtime_map,
                    repertoire_offset,
                    &repertoire,
                    spec,
                )?)
            } else {
                if repertoire.opaque_pairs != 0 {
                    return Err(error(format!(
                        "{} 비-compound scene pair source에 opaque pair가 있음",
                        family.sequence
                    )));
                }
                None
            };
            expected_root_fields.insert((
                scope.seq_archive,
                family.sequence,
                e1_record_offset + 4,
                repertoire_offset,
            ));

            let mut setup_record = [0u8; 16];
            setup_record[0] = 0x28;
            setup_record[1] = 0x12;
            setup_record[2] = 0x97;
            setup_record[4..6].copy_from_slice(&resource_id.to_le_bytes());
            setup_record[12..16]
                .copy_from_slice(&BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME.to_le_bytes());
            let setup_offsets = raw
                .windows(setup_record.len())
                .enumerate()
                .filter_map(|(offset, window)| (window == setup_record).then_some(offset))
                .collect::<Vec<_>>();
            let [cell_setup_record_offset] = setup_offsets.as_slice() else {
                return Err(error(format!(
                    "{}:{} cell setup record 수가 1이 아님: {}",
                    scope.scope,
                    family.sequence,
                    setup_offsets.len()
                )));
            };
            if runtime_map.file_offset(BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME)
                != Some(BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET)
            {
                return Err(error("scene cell table runtime/file 사상 불일치"));
            }
            let repertoire_slots = repertoire.raw.len() / 2;
            verify_cell_layout(raw, repertoire_slots, family.grid_slots)?;
            let selected =
                load_asset_by_resource(scope, resource_id, map, text, archive, btx_names)?;
            let same_stem =
                load_asset_by_name(scope, family.same_stem_asset, map, text, archive, btx_names)?;
            let selected_atlas_compatible = selected.cnx_subtype == *b"bi_"
                && selected.plain.len() == BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES;
            let same_stem_atlas_compatible = same_stem.cnx_subtype == *b"bi_"
                && same_stem.plain.len() == BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES;
            if !same_stem_atlas_compatible {
                return Err(error(format!(
                    "{}:{} same-stem scene atlas 형식/길이 불일치",
                    scope.scope, family.same_stem_asset
                )));
            }
            let equal_cells = if selected_atlas_compatible {
                count_equal_grid_cells(&selected.plain, &same_stem.plain, family.grid_slots)?
            } else {
                0
            };
            let cell_last = BOX_SUBGAME_SEQ_DRAC_CELL_BASE
                .checked_add(
                    u16::try_from(repertoire_slots - 1)
                        .map_err(|_| error("scene repertoire cell 변환 실패"))?,
                )
                .ok_or_else(|| error("scene repertoire cell 마지막 overflow"))?;
            let unique_pairs = repertoire
                .raw
                .chunks_exact(2)
                .map(|pair| [pair[0], pair[1]])
                .collect::<BTreeSet<_>>()
                .len();
            pending.push(PendingRow {
                row: SubgameSeqSceneGlyphRow {
                    scope: scope.scope,
                    exe: scope.exe,
                    seq_archive: scope.seq_archive,
                    grp_archive: scope.grp_archive,
                    sequence: family.sequence,
                    sequence_sha256,
                    e2_record_offset,
                    e1_record_offset,
                    repertoire_offset,
                    repertoire_runtime,
                    repertoire_bytes: repertoire.raw.len(),
                    repertoire_slots,
                    strict_sjis_pairs: repertoire.strict_sjis_pairs,
                    opaque_pairs: repertoire.opaque_pairs,
                    repertoire_unique_pairs: unique_pairs,
                    repertoire_sha256: Sha256::digest(&repertoire.raw).into(),
                    repertoire_copy_equal: false,
                    canonical_glyph_slots: compound
                        .as_ref()
                        .map_or(repertoire_slots, |analysis| analysis.canonical_glyph_slots),
                    protected_message_slots: compound
                        .as_ref()
                        .map_or(0, |analysis| analysis.protected_message_slots),
                    compound_message_records: compound
                        .as_ref()
                        .map_or(0, |analysis| analysis.message_records),
                    compound_glyph_tokens: compound
                        .as_ref()
                        .map_or(0, |analysis| analysis.glyph_tokens),
                    compound_control_tokens: compound
                        .as_ref()
                        .map_or(0, |analysis| analysis.control_tokens),
                    compound_variant_records: compound
                        .as_ref()
                        .map_or(0, |analysis| analysis.variant_records),
                    embedded_message_record_offset: compound
                        .as_ref()
                        .map(|analysis| analysis.embedded_message_record_offset),
                    resource_id,
                    auxiliary_id,
                    auxiliary_asset: auxiliary_asset.clone(),
                    cell_setup_record_offset: *cell_setup_record_offset,
                    cell_table_offset: BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET,
                    cell_table_runtime: BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME,
                    cell_base: BOX_SUBGAME_SEQ_DRAC_CELL_BASE,
                    cell_last,
                    grid_slots: family.grid_slots,
                    selected_asset: selected.name.clone(),
                    selected_vdd_index: selected.vdd_index,
                    selected_stored_bytes: selected.stored_bytes,
                    selected_cnx_subtype: selected.cnx_subtype,
                    selected_plain_bytes: selected.plain.len(),
                    selected_plain_sha256: selected.plain_sha256,
                    selected_atlas_compatible,
                    same_stem_asset: family.same_stem_asset,
                    same_stem_resource_id: same_stem.resource_id,
                    same_stem_vdd_index: same_stem.vdd_index,
                    same_stem_stored_bytes: same_stem.stored_bytes,
                    same_stem_cnx_subtype: same_stem.cnx_subtype,
                    same_stem_plain_bytes: same_stem.plain.len(),
                    same_stem_plain_sha256: same_stem.plain_sha256,
                    selected_is_same_stem: selected.name == family.same_stem_asset,
                    selected_same_stem_equal_grid_cells: equal_cells,
                    same_stem_copy_equal: false,
                    e1_handler_runtime: drac.e1_handler_runtime,
                    e2_handler_runtime: drac.e2_handler_runtime,
                    pair_lookup_runtime: drac.pair_lookup_runtime,
                    renderer_runtime: drac.renderer_runtime,
                },
                repertoire: repertoire.raw,
                same_stem_plain: same_stem.plain,
            });
        }
    }

    let source_map_ref = &source_map;
    let actual_root_fields = roots
        .rows
        .iter()
        .flat_map(|row| {
            row.roots.iter().filter_map(move |root| {
                if root.candidate_delta != 0 || root.pointer_field_offset < 4 {
                    return None;
                }
                let raw = source_map_ref.get(&(row.archive.as_str(), row.sequence.as_str()))?;
                (raw.get(root.pointer_field_offset - 4..root.pointer_field_offset)
                    == Some([0xE1, 0, 0, 0].as_slice()))
                .then_some((
                    row.archive.as_str(),
                    row.sequence.as_str(),
                    root.pointer_field_offset,
                    root.target_offset,
                ))
            })
        })
        .collect::<BTreeSet<_>>();
    if actual_root_fields != expected_root_fields {
        return Err(error(format!(
            "scene E1 exact-root 전수 불일치: expected={}, actual={}",
            expected_root_fields.len(),
            actual_root_fields.len()
        )));
    }

    for family in BOX_SUBGAME_SEQ_SCENE_FAMILIES {
        let indices = pending
            .iter()
            .enumerate()
            .filter_map(|(index, row)| (row.row.sequence == family.sequence).then_some(index))
            .collect::<Vec<_>>();
        let [first, second] = indices.as_slice() else {
            return Err(error(format!(
                "{} scene copy 수가 2가 아님",
                family.sequence
            )));
        };
        let repertoire_equal = pending[*first].repertoire == pending[*second].repertoire;
        let same_stem_equal = pending[*first].same_stem_plain == pending[*second].same_stem_plain;
        pending[*first].row.repertoire_copy_equal = repertoire_equal;
        pending[*second].row.repertoire_copy_equal = repertoire_equal;
        pending[*first].row.same_stem_copy_equal = same_stem_equal;
        pending[*second].row.same_stem_copy_equal = same_stem_equal;
    }

    let mut rows = pending.into_iter().map(|row| row.row).collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.sequence, row.scope));
    let analysis = SubgameSeqSceneGlyphAnalysis {
        families: rows
            .iter()
            .map(|row| row.sequence)
            .collect::<BTreeSet<_>>()
            .len(),
        repertoire_slots: rows.iter().map(|row| row.repertoire_slots).sum(),
        strict_sjis_pairs: rows.iter().map(|row| row.strict_sjis_pairs).sum(),
        opaque_pairs: rows.iter().map(|row| row.opaque_pairs).sum(),
        grid_slots: rows.iter().map(|row| row.grid_slots).sum(),
        repertoire_copy_matches: rows
            .iter()
            .filter(|row| row.scope == "PUYO1" && row.repertoire_copy_equal)
            .count(),
        same_stem_copy_matches: rows
            .iter()
            .filter(|row| row.scope == "PUYO1" && row.same_stem_copy_equal)
            .count(),
        puyo1_selected_same_stem_rows: rows
            .iter()
            .filter(|row| row.scope == "PUYO1" && row.selected_is_same_stem)
            .count(),
        puyo2_selected_same_stem_rows: rows
            .iter()
            .filter(|row| row.scope == "PUYO2" && row.selected_is_same_stem)
            .count(),
        puyo1_selected_atlas_rows: rows
            .iter()
            .filter(|row| row.scope == "PUYO1" && row.selected_atlas_compatible)
            .count(),
        puyo2_selected_atlas_rows: rows
            .iter()
            .filter(|row| row.scope == "PUYO2" && row.selected_atlas_compatible)
            .count(),
        puyo1_selected_same_stem_cells: rows
            .iter()
            .filter(|row| row.scope == "PUYO1")
            .map(|row| row.selected_same_stem_equal_grid_cells)
            .sum(),
        puyo2_selected_same_stem_cells: rows
            .iter()
            .filter(|row| row.scope == "PUYO2")
            .map(|row| row.selected_same_stem_equal_grid_cells)
            .sum(),
        canonical_glyph_slots: rows.iter().map(|row| row.canonical_glyph_slots).sum(),
        protected_message_slots: rows.iter().map(|row| row.protected_message_slots).sum(),
        compound_rows: rows
            .iter()
            .filter(|row| row.embedded_message_record_offset.is_some())
            .count(),
        compound_message_records: rows.iter().map(|row| row.compound_message_records).sum(),
        compound_glyph_tokens: rows.iter().map(|row| row.compound_glyph_tokens).sum(),
        compound_control_tokens: rows.iter().map(|row| row.compound_control_tokens).sum(),
        compound_variant_records: rows.iter().map(|row| row.compound_variant_records).sum(),
        rows,
    };
    Ok(analysis)
}

/// 지원 원본의 34개 scene-local glyph 자원 경계를 고정한다.
pub fn verify_box_subgame_seq_scene_glyphs<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    grp_archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
    layouts: &SubgameSeqLayoutAnalysis,
    roots: &SubgameSeqTextRootAnalysis,
    btx_names: &BtxNameAnalysis,
    shared_code: (
        &SubgameSeqDracConsumerAnalysis,
        &SubgameSeqDracGlyphAnalysis,
    ),
) -> Result<SubgameSeqSceneGlyphAnalysis, SubgameSeqSceneGlyphError> {
    let analysis = audit_subgame_seq_scene_glyphs(
        executables,
        sources,
        grp_archives,
        layouts,
        roots,
        btx_names,
        shared_code,
    )?;
    let actual = [
        analysis.rows.len(),
        analysis.families,
        analysis.repertoire_slots,
        analysis.strict_sjis_pairs,
        analysis.opaque_pairs,
        analysis.grid_slots,
        analysis.repertoire_copy_matches,
        analysis.same_stem_copy_matches,
        analysis.puyo1_selected_same_stem_rows,
        analysis.puyo2_selected_same_stem_rows,
        analysis.puyo1_selected_atlas_rows,
        analysis.puyo2_selected_atlas_rows,
        analysis.puyo1_selected_same_stem_cells,
        analysis.puyo2_selected_same_stem_cells,
        analysis.canonical_glyph_slots,
        analysis.protected_message_slots,
        analysis.compound_rows,
        analysis.compound_message_records,
        analysis.compound_glyph_tokens,
        analysis.compound_control_tokens,
        analysis.compound_variant_records,
    ];
    let expected = [
        BOX_SUBGAME_SEQ_SCENE_GLYPH_ROWS,
        BOX_SUBGAME_SEQ_SCENE_GLYPH_FAMILIES,
        BOX_SUBGAME_SEQ_SCENE_REPERTOIRE_SLOTS,
        BOX_SUBGAME_SEQ_SCENE_STRICT_SJIS_PAIRS,
        BOX_SUBGAME_SEQ_SCENE_OPAQUE_PAIRS,
        BOX_SUBGAME_SEQ_SCENE_GRID_SLOTS,
        BOX_SUBGAME_SEQ_SCENE_REPERTOIRE_COPY_MATCHES,
        BOX_SUBGAME_SEQ_SCENE_SAME_STEM_COPY_MATCHES,
        BOX_SUBGAME_SEQ_SCENE_PUYO1_SELECTED_SAME_STEM_ROWS,
        BOX_SUBGAME_SEQ_SCENE_PUYO2_SELECTED_SAME_STEM_ROWS,
        BOX_SUBGAME_SEQ_SCENE_PUYO1_SELECTED_ATLAS_ROWS,
        BOX_SUBGAME_SEQ_SCENE_PUYO2_SELECTED_ATLAS_ROWS,
        BOX_SUBGAME_SEQ_SCENE_PUYO1_SELECTED_SAME_STEM_CELLS,
        BOX_SUBGAME_SEQ_SCENE_PUYO2_SELECTED_SAME_STEM_CELLS,
        BOX_SUBGAME_SEQ_SCENE_CANONICAL_GLYPH_SLOTS,
        BOX_SUBGAME_SEQ_SCENE_PROTECTED_MESSAGE_SLOTS,
        BOX_SUBGAME_SEQ_SCENE_COMPOUND_ROWS,
        BOX_SUBGAME_SEQ_SCENE_COMPOUND_MESSAGE_RECORDS,
        BOX_SUBGAME_SEQ_SCENE_COMPOUND_GLYPH_TOKENS,
        BOX_SUBGAME_SEQ_SCENE_COMPOUND_CONTROL_TOKENS,
        BOX_SUBGAME_SEQ_SCENE_COMPOUND_VARIANT_RECORDS,
    ];
    if actual != expected {
        return Err(error(format!(
            "scene glyph 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    let report = render_subgame_seq_scene_glyphs_tsv(&analysis);
    let report_sha256: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if report_sha256 != BOX_SUBGAME_SEQ_SCENE_GLYPH_REPORT_SHA256 {
        return Err(error(format!(
            "scene glyph report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_SCENE_GLYPH_REPORT_SHA256),
            hex(&report_sha256)
        )));
    }
    Ok(analysis)
}

/// scene-local resource 선택과 same-stem 비교를 결정적 TSV로 만든다.
pub fn render_subgame_seq_scene_glyphs_tsv(analysis: &SubgameSeqSceneGlyphAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\texe\tseq_archive\tgrp_archive\tsequence\tsequence_sha256\t\
         e2_record_offset\te1_record_offset\trepertoire_offset\trepertoire_runtime\t\
         repertoire_bytes\trepertoire_slots\tstrict_sjis_pairs\topaque_pairs\t\
         repertoire_unique_pairs\trepertoire_sha256\t\
         repertoire_copy_equal\tcanonical_glyph_slots\tprotected_message_slots\t\
         compound_message_records\tcompound_glyph_tokens\tcompound_control_tokens\t\
         compound_variant_records\tembedded_message_record_offset\t\
         resource_id\tauxiliary_id\tauxiliary_asset\t\
         cell_setup_record_offset\tcell_table_offset\tcell_table_runtime\tcell_base\tcell_last\t\
         grid_slots\tselected_asset\tselected_vdd_index\tselected_stored_bytes\t\
         selected_cnx_subtype\tselected_plain_bytes\tselected_plain_sha256\t\
         selected_atlas_compatible\tsame_stem_asset\tsame_stem_resource_id\t\
         same_stem_vdd_index\tsame_stem_stored_bytes\tsame_stem_cnx_subtype\t\
         same_stem_plain_bytes\tsame_stem_plain_sha256\t\
         selected_is_same_stem\tselected_same_stem_equal_grid_cells\tsame_stem_copy_equal\t\
         e1_handler_runtime\te2_handler_runtime\tpair_lookup_runtime\trenderer_runtime\t\
         consumer_contract\truntime_verdict\tpatch_strategy\n",
    );
    for row in &analysis.rows {
        let fields = vec![
            SUBGAME_SEQ_SCENE_GLYPH_SCHEMA_VERSION.to_string(),
            row.scope.to_string(),
            row.exe.to_string(),
            row.seq_archive.to_string(),
            row.grp_archive.to_string(),
            row.sequence.to_string(),
            hex(&row.sequence_sha256),
            format!("0x{:04X}", row.e2_record_offset),
            format!("0x{:04X}", row.e1_record_offset),
            format!("0x{:04X}", row.repertoire_offset),
            format!("0x{:08X}", row.repertoire_runtime),
            row.repertoire_bytes.to_string(),
            row.repertoire_slots.to_string(),
            row.strict_sjis_pairs.to_string(),
            row.opaque_pairs.to_string(),
            row.repertoire_unique_pairs.to_string(),
            hex(&row.repertoire_sha256),
            row.repertoire_copy_equal.to_string(),
            row.canonical_glyph_slots.to_string(),
            row.protected_message_slots.to_string(),
            row.compound_message_records.to_string(),
            row.compound_glyph_tokens.to_string(),
            row.compound_control_tokens.to_string(),
            row.compound_variant_records.to_string(),
            row.embedded_message_record_offset
                .map_or_else(String::new, |offset| format!("0x{offset:04X}")),
            format!("0x{:04X}", row.resource_id),
            format!("0x{:04X}", row.auxiliary_id),
            row.auxiliary_asset.clone(),
            format!("0x{:04X}", row.cell_setup_record_offset),
            format!("0x{:04X}", row.cell_table_offset),
            format!("0x{:08X}", row.cell_table_runtime),
            row.cell_base.to_string(),
            row.cell_last.to_string(),
            row.grid_slots.to_string(),
            row.selected_asset.clone(),
            row.selected_vdd_index.to_string(),
            row.selected_stored_bytes.to_string(),
            String::from_utf8_lossy(&row.selected_cnx_subtype).into_owned(),
            row.selected_plain_bytes.to_string(),
            hex(&row.selected_plain_sha256),
            row.selected_atlas_compatible.to_string(),
            row.same_stem_asset.to_string(),
            format!("0x{:04X}", row.same_stem_resource_id),
            row.same_stem_vdd_index.to_string(),
            row.same_stem_stored_bytes.to_string(),
            String::from_utf8_lossy(&row.same_stem_cnx_subtype).into_owned(),
            row.same_stem_plain_bytes.to_string(),
            hex(&row.same_stem_plain_sha256),
            row.selected_is_same_stem.to_string(),
            row.selected_same_stem_equal_grid_cells.to_string(),
            row.same_stem_copy_equal.to_string(),
            format!("0x{:08X}", row.e1_handler_runtime),
            format!("0x{:08X}", row.e2_handler_runtime),
            format!("0x{:08X}", row.pair_lookup_runtime),
            format!("0x{:08X}", row.renderer_runtime),
            "shared-e1-e2-pair-index-plus-30-short-renderer-static".to_string(),
            "unverified".to_string(),
            row.patch_strategy().to_string(),
        ];
        writeln!(output, "{}", fields.join("\t")).expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_scene_glyph_tests.rs"]
mod subgame_seq_scene_glyph_tests;
