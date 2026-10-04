//! 원본 RPG_FNT1의 공통 문자도 동적 page와 같은 Denki 획으로 공급한다.
use super::cnx_development_entry::{
    prepare_cnx_development_entry, CnxDevelopmentEntrySpec, PreparedCnxDevelopmentEntry,
};
use crate::btx::{decode, encode};
use crate::disc::{Iso9660, VerifiedSourceImage};
use crate::font::{decode_quest_source_glyph, QuestGlyphSlot, QUEST_FONT_LAYOUT};
use sha2::{Digest, Sha256};

pub(super) fn prepare(source: &VerifiedSourceImage) -> Result<PreparedCnxDevelopmentEntry, String> {
    let iso = Iso9660::parse(source.image()).map_err(|e| e.to_string())?;
    let table = super::quest_poc::quest_code_table(&iso).map_err(|e| e.to_string())?;
    let font = crate::font::selected::load(crate::font::selected::DENKI)?;
    let (entry, ()) = prepare_cnx_development_entry(
        source,
        CnxDevelopmentEntrySpec {
            iso_path: "/QUEST/RPG_GRP.VDD;1",
            entry_name: "RPG_FNT1.BTX",
            expected_cnx_subtype: *b"bi_",
            patch_id: "quest-font-symbols",
            actor: "pipeline::quest_font_symbols",
        },
        |original| {
            if format!("{:x}", Sha256::digest(original))
                != "98151af59e76b8be63bb99d4a7c0c3ec96615ffdf1090ab511dc1dabf8ae0a4c"
            {
                return Err("RPG_FNT1 원본 SHA-256 불일치".into());
            }
            let mut pixels = decode(original, QUEST_FONT_LAYOUT).map_err(|e| e.to_string())?;
            for (index, code) in table.codes().iter().enumerate() {
                let Some(glyph) = decode_quest_source_glyph(*code) else {
                    continue;
                };
                if !crate::font::selected::quest_common_glyph(glyph) {
                    continue;
                }
                let slot = QuestGlyphSlot::for_index(index).map_err(|e| e.to_string())?;
                let cell = crate::font::selected::quest_cell(&font, glyph, slot.width)?;
                for row in 0..slot.height {
                    let start = (slot.y + row) * QUEST_FONT_LAYOUT.width + slot.x;
                    pixels[start..start + slot.width]
                        .copy_from_slice(&cell[row * slot.width..(row + 1) * slot.width]);
                }
            }
            Ok((
                encode(&pixels, QUEST_FONT_LAYOUT).map_err(|e| e.to_string())?,
                (),
            ))
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(entry)
}
