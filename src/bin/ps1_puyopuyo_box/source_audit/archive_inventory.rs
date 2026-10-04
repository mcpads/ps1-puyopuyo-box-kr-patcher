//! Sound, QMP, VDD와 CNX 원본 inventory를 검증한다.

use ps1_puyopuyo_box::archive::{verify_box_sound_inventory, VddArchive, BOX_SOUND_ARCHIVE_PATHS};
use ps1_puyopuyo_box::compress::verify_box_cnx_inventory;
use ps1_puyopuyo_box::qmp::verify_box_qmp_inventory;

use super::{ArchiveAuditData, SourceAuditContext};
use crate::cli::disc_commands::audit_vdd_data;
use crate::cli::{BOX_ROUNDTRIP_VDDS, RPG_MAP_PATH};

pub(super) fn audit_archive_inventory(
    context: &SourceAuditContext<'_>,
) -> Result<ArchiveAuditData, String> {
    let iso = &context.iso;
    let sound_archives: Vec<_> = BOX_SOUND_ARCHIVE_PATHS
        .iter()
        .map(|&archive_path| {
            let raw = iso
                .read_file(archive_path)
                .map_err(|error| error.to_string())?;
            let archive = VddArchive::parse(&raw)
                .map_err(|error| format!("{archive_path} VDD 파싱 실패: {error}"))?;
            Ok((archive_path, archive))
        })
        .collect::<Result<_, String>>()?;
    let sound_inventory = verify_box_sound_inventory(
        sound_archives
            .iter()
            .map(|(archive_path, archive)| (*archive_path, archive)),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS Sound VDD inventory: archives {}, entries {}, validated VAB headers/bodies {}/{}, \
         pQES sequences {}",
        sound_inventory.archives,
        sound_inventory.entries,
        sound_inventory.vab_headers,
        sound_inventory.vab_bodies,
        sound_inventory.sound_sequences,
    );

    let rpg_map_raw = iso
        .read_file(RPG_MAP_PATH)
        .map_err(|error| error.to_string())?;
    let rpg_map = VddArchive::parse(&rpg_map_raw)
        .map_err(|error| format!("{RPG_MAP_PATH} VDD 파싱 실패: {error}"))?;
    let qmp_inventory = verify_box_qmp_inventory(&rpg_map).map_err(|error| error.to_string())?;
    println!(
        "PASS QMP inventory: maps {}, cells {}, dimensions {}, unique tile IDs {}",
        qmp_inventory.maps,
        qmp_inventory.cells,
        qmp_inventory.dimensions,
        qmp_inventory.unique_tile_ids,
    );

    let mut vdd_sources = Vec::with_capacity(BOX_ROUNDTRIP_VDDS);
    for entry in iso.entries() {
        if entry.is_directory()
            || !(entry.path().ends_with("SEQ.VDD;1") || entry.path().ends_with("GRP.VDD;1"))
        {
            continue;
        }
        let raw = iso
            .read_file(entry.path())
            .map_err(|error| error.to_string())?;
        vdd_sources.push((entry.path().to_string(), raw));
    }
    if vdd_sources.len() != BOX_ROUNDTRIP_VDDS {
        return Err(format!(
            "원본 SEQ·GRP VDD 수 불일치: expected={BOX_ROUNDTRIP_VDDS}, actual={}",
            vdd_sources.len()
        ));
    }
    audit_vdd_data(
        vdd_sources
            .iter()
            .map(|(source_id, raw)| (source_id.as_str(), raw.as_slice())),
        false,
    )?;
    let cnx_archives: Vec<_> = vdd_sources
        .iter()
        .map(|(source_id, raw)| {
            VddArchive::parse(raw)
                .map(|archive| (source_id.clone(), archive))
                .map_err(|error| format!("{source_id} VDD 파싱 실패: {error}"))
        })
        .collect::<Result<_, _>>()?;
    let cnx_inventory = verify_box_cnx_inventory(
        cnx_archives
            .iter()
            .map(|(source_id, archive)| (source_id.as_str(), archive)),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS CNX inventory: VDD {}, entries {}, CNX {} \
         (bin {}, bi_ {}, bit {}, cel {}), raw {}",
        cnx_inventory.archives,
        cnx_inventory.entries,
        cnx_inventory.cnx,
        cnx_inventory.bin,
        cnx_inventory.bi_,
        cnx_inventory.bit,
        cnx_inventory.cel,
        cnx_inventory.raw
    );

    Ok(ArchiveAuditData {
        vdd_sources,
        cnx_archives,
    })
}
