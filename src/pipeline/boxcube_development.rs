//! BOX 모드 큐브을 기존 FIL_GRP 최종 합성에 기여한다.
use super::cnx_development_entry::{
    prepare_cnx_development_entry, CnxDevelopmentEntrySpec, PreparedCnxDevelopmentEntry,
};
use crate::disc::{DiscImage, Iso9660, VerifiedSourceImage};

pub(super) fn prepare_boxcube(
    source: &VerifiedSourceImage,
) -> Result<PreparedCnxDevelopmentEntry, String> {
    prepare_cnx_development_entry(
        source,
        CnxDevelopmentEntrySpec {
            iso_path: "/FIL_GRP.VDD;1",
            entry_name: "BOXCUBE.BTX",
            expected_cnx_subtype: *b"bit",
            patch_id: "box-mode-cubes",
            actor: "pipeline::boxcube_development",
        },
        |original| {
            Ok((
                crate::graphics::boxcube_surface::patch_boxcube_surface(original)?,
                (),
            ))
        },
    )
    .map(|(entry, ())| entry)
    .map_err(|e| e.to_string())
}

pub(super) fn verify_boxcube_palette(image: &[u8]) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let disc = DiscImage::from_bytes(image.to_vec()).map_err(|e| e.to_string())?;
    let iso = Iso9660::parse(&disc).map_err(|e| e.to_string())?;
    let exe = iso.read_file("/SLPS_031.14;1").map_err(|e| e.to_string())?;
    let palette = exe
        .get(0x1b834..0x1ba34)
        .ok_or("BOXCUBE palette 범위 누락")?;
    if format!("{:x}", Sha256::digest(palette)) != BOX_PALETTE_SHA {
        return Err("BOXCUBE 최종 palette 불일치".into());
    }
    Ok(())
}

const BOX_PALETTE_SHA: &str = "c550d1893bad7aba2343538254f5c6c19477450beba30cfa97cce5a6f9e36680";
