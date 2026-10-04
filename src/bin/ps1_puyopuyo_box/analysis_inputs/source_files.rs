//! 지원 원본에서 QUEST와 서브게임 SEQ 원천을 읽는다.

use std::path::Path;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::{decompress, is_cnx};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::script::BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS;

use crate::cli::{RPG_SEQ_PATH, TOWN00_NAME};

pub(in crate::cli) fn read_town00_source(source_path: &Path) -> Result<Vec<u8>, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    let entry = archive
        .find(TOWN00_NAME)
        .ok_or_else(|| format!("{RPG_SEQ_PATH}:{TOWN00_NAME} 엔트리가 없음"))?;
    let cnx = archive.read_file(entry);
    if !is_cnx(cnx) {
        return Err(format!("{RPG_SEQ_PATH}:{TOWN00_NAME}가 CNX v2가 아님"));
    }
    decompress(cnx).map_err(|error| format!("{RPG_SEQ_PATH}:{TOWN00_NAME}: {error}"))
}

pub(in crate::cli) fn read_quest_seq_sources(
    archive: &VddArchive,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    archive
        .entries()
        .iter()
        .map(|entry| {
            let name = entry.name();
            let cnx = archive.read_file(entry);
            if !is_cnx(cnx) {
                return Err(format!("{RPG_SEQ_PATH}:{name}가 CNX v2가 아님"));
            }
            let raw = decompress(cnx).map_err(|error| format!("{RPG_SEQ_PATH}:{name}: {error}"))?;
            Ok((name.to_string(), raw))
        })
        .collect()
}

pub(in crate::cli) fn read_subgame_seq_sources(
    archives: &[(&str, VddArchive)],
) -> Result<Vec<(String, String, Vec<u8>)>, String> {
    let mut sources = Vec::new();
    for expectation in BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS {
        let (_, archive) = archives
            .iter()
            .find(|(archive_path, _)| *archive_path == expectation.archive)
            .ok_or_else(|| format!("{} archive가 없음", expectation.archive))?;
        for entry in archive
            .entries()
            .iter()
            .filter(|entry| entry.name().to_ascii_uppercase().ends_with(".SEQ"))
        {
            let sequence = entry.name();
            let cnx = archive.read_file(entry);
            if !is_cnx(cnx) {
                return Err(format!(
                    "{}:{sequence}가 CNX v2가 아님",
                    expectation.archive
                ));
            }
            let raw = decompress(cnx)
                .map_err(|error| format!("{}:{sequence}: {error}", expectation.archive))?;
            sources.push((expectation.archive.to_string(), sequence.to_string(), raw));
        }
    }
    Ok(sources)
}
