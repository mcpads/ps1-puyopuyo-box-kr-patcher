//! BOX 간판을 기존 FIL_GRP 최종 합성에 기여한다.
use super::cnx_development_entry::{
    prepare_cnx_development_entry, CnxDevelopmentEntrySpec, PreparedCnxDevelopmentEntry,
};
use crate::disc::{DiscImage, Iso9660, VerifiedSourceImage};

pub(super) fn prepare_boxparts(
    source: &VerifiedSourceImage,
) -> Result<PreparedCnxDevelopmentEntry, String> {
    prepare_cnx_development_entry(
        source,
        CnxDevelopmentEntrySpec {
            iso_path: "/FIL_GRP.VDD;1",
            entry_name: "BOXPARTS.BTX",
            expected_cnx_subtype: *b"bit",
            patch_id: "box-player-signs",
            actor: "pipeline::boxparts_development",
        },
        |original| {
            Ok((
                crate::graphics::boxparts_surface::patch_boxparts_surface(original)?,
                (),
            ))
        },
    )
    .map(|(entry, ())| entry)
    .map_err(|e| e.to_string())
}

pub(super) fn verify_boxparts_palette(image: &[u8]) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let disc = DiscImage::from_bytes(image.to_vec()).map_err(|e| e.to_string())?;
    let iso = Iso9660::parse(&disc).map_err(|e| e.to_string())?;
    let exe = iso.read_file("/SLPS_031.14;1").map_err(|e| e.to_string())?;
    let palette = exe
        .get(0x1bac0..0x1bcc0)
        .ok_or("BOXPARTS palette 범위 누락")?;
    if format!("{:x}", Sha256::digest(palette)) != BOX_PALETTE_SHA {
        return Err("BOXPARTS 최종 palette 불일치".into());
    }
    Ok(())
}

const BOX_PALETTE_SHA: &str = "c2b04b86cdb9532b8490ffcbfe2e8b1f47390f1e9a04114890f5f927e0f4c711";
