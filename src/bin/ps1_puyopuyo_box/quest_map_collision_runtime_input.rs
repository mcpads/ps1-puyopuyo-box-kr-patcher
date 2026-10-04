//! QUEST collision runtime 보고서들이 공유하는 exact 입력 검증.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::coords::ExeMap;
use ps1_puyopuyo_box::disasm::verify_box_quest_map_collision;
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::qmp::{verify_box_qmp_inventory, QmpMap};
use ps1_puyopuyo_box::runtime::{
    inspect_quest_map_collision_state, MednafenPsxState, QuestMapCollisionSource,
    QuestMapCollisionStateReport,
};
use ps1_puyopuyo_box::source::BIN_SHA1;
use sha2::{Digest, Sha256};

use super::disc_commands::read_box_executables;
use super::RPG_MAP_PATH;

pub(super) struct QuestMapCollisionRuntimeInput {
    pub quest_map: &'static ExeMap,
    pub quest_text: Vec<u8>,
    pub source_image_sha1: &'static str,
    pub source_image_sha256: String,
    pub observed_image_sha256: String,
    pub state_raw: Vec<u8>,
    pub state_sha256: String,
    pub state_version: u32,
    pub report: QuestMapCollisionStateReport,
}

pub(super) fn parse_quest_pixel(raw: &OsStr, role: &str) -> Result<i32, String> {
    let text = raw
        .to_str()
        .ok_or_else(|| format!("QUEST {role} pixel이 UTF-8이 아님"))?;
    text.parse::<i32>()
        .map_err(|error| format!("QUEST {role} pixel 파싱 실패: {text}: {error}"))
}

pub(super) fn load_quest_map_collision_runtime_input(
    source_path: &Path,
    observed_image_path: &Path,
    state_path: &Path,
) -> Result<QuestMapCollisionRuntimeInput, String> {
    let source = VerifiedSourceImage::open(source_path).map_err(|error| error.to_string())?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let source_image_sha256 = format!("{:x}", Sha256::digest(source.image().raw_bytes()));
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|executable| executable.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    verify_box_quest_map_collision(quest.map, &quest.text).map_err(|error| error.to_string())?;

    let archive_raw = iso
        .read_file(RPG_MAP_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&archive_raw)
        .map_err(|error| format!("{RPG_MAP_PATH} VDD 파싱 실패: {error}"))?;
    verify_box_qmp_inventory(&archive).map_err(|error| error.to_string())?;
    let maps = archive
        .entries()
        .iter()
        .map(|entry| {
            QmpMap::parse(archive.read_file(entry))
                .map_err(|error| format!("{RPG_MAP_PATH}:{}: {error}", entry.name()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let map_sources = archive
        .entries()
        .iter()
        .zip(&maps)
        .enumerate()
        .map(|(index, (entry, map))| {
            let resource_id = u16::try_from(index + 1)
                .map_err(|_| "QUEST QMP resource ID overflow".to_string())?;
            Ok(QuestMapCollisionSource {
                resource_id,
                name: entry.name(),
                map,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let observed_image = fs::read(observed_image_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", observed_image_path.display()))?;
    if observed_image.len() != source.image().raw_bytes().len() {
        return Err(format!(
            "관측 BIN 크기 불일치: expected={}, actual={}",
            source.image().raw_bytes().len(),
            observed_image.len(),
        ));
    }
    let observed_image_sha256 = format!("{:x}", Sha256::digest(&observed_image));

    let state_raw = fs::read(state_path)
        .map_err(|error| format!("{} 읽기 실패: {error}", state_path.display()))?;
    let state_sha256 = format!("{:x}", Sha256::digest(&state_raw));
    let state = MednafenPsxState::parse(&state_raw).map_err(|error| error.to_string())?;
    let state_version = state.state_version();
    let report = inspect_quest_map_collision_state(&state, &map_sources)
        .map_err(|error| error.to_string())?;

    Ok(QuestMapCollisionRuntimeInput {
        quest_map: quest.map,
        quest_text: quest.text.clone(),
        source_image_sha1: BIN_SHA1,
        source_image_sha256,
        observed_image_sha256,
        state_raw,
        state_sha256,
        state_version,
        report,
    })
}
