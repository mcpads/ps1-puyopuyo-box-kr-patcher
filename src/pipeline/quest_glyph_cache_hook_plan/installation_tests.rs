use super::*;
use crate::pipeline::quest_item_name_glyph_cache_hook::{
    assemble_item_name_direct_glyph_sites, assemble_item_name_whole_string_sites,
};

#[test]
fn 이름_hook의_설치_id는_모든_실제_쓰기_주소에_대응한다() {
    for (sites, ids) in [
        (
            assemble_item_name_direct_glyph_sites(0x8011_0000).unwrap(),
            QUEST_ITEM_NAME_DIRECT_GLYPH_SITE_IDS.as_slice(),
        ),
        (
            assemble_item_name_whole_string_sites(0x8011_0000).unwrap(),
            QUEST_ITEM_NAME_WHOLE_STRING_SITE_IDS.as_slice(),
        ),
    ] {
        assert_eq!(sites.len(), ids.len(), "zip으로 설치 쓰기를 버리면 안 된다");
        for (site, id) in sites.iter().zip(ids) {
            let address = u32::from_str_radix(id.rsplit('-').next().unwrap(), 16).unwrap();
            assert_eq!(address, site.base_addr(), "Expected Write ID와 실제 주소");
        }
    }
}
