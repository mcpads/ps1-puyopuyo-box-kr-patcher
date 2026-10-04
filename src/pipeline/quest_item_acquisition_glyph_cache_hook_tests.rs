use crate::disasm::QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES;
use crate::isa::{decode_slice, InstructionKind as K, Reg};

use super::assemble_item_acquisition_whole_string_sites;

#[test]
fn 여섯_고정문은_같은_page래퍼와_원래_delay를_쓴다() {
    let entry = 0x8011_2000;
    let sites = assemble_item_acquisition_whole_string_sites(entry).expect("sites");
    assert_eq!(
        sites.len(),
        QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES.len()
    );
    for (site, expected_runtime) in sites
        .iter()
        .zip(QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES)
    {
        assert_eq!(site.base_addr(), expected_runtime);
        let instructions = decode_slice(site.base_addr(), site.bytes()).expect("decode");
        assert_eq!(instructions[0].kind, K::Jal { target: entry });
    }
    let suffix = decode_slice(sites[0].base_addr(), sites[0].bytes()).expect("suffix");
    assert_eq!(
        suffix[1].kind,
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0
        }
    );
    let particle = decode_slice(sites[4].base_addr(), sites[4].bytes()).expect("particle");
    assert_eq!(
        particle[1].kind,
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: -0x1E10
        }
    );
}
