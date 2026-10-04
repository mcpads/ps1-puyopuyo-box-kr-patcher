use super::*;

fn path(runtime: u32) -> QuestItemDescriptionSelectionPath {
    QuestItemDescriptionSelectionPath {
        input_call_runtime: 0,
        no_input_branch_runtime: 0,
        constructor_call_runtime: runtime,
        cursor_changed_branch_runtime: 0,
        cursor_changed_handler_runtime: 0,
        state_reset_runtime: 0,
        selected_index_store_runtime: 0,
    }
}

#[test]
fn constructor_call은_원본_delay_slot을_보존한다() {
    let sites = assemble_item_description_residency_sites(
        &[path(0x8003_8BF8), path(0x8003_9504), path(0x8003_AA8C)],
        0x8011_0000,
    )
    .unwrap();

    assert_eq!(sites.len(), QUEST_ITEM_DESCRIPTION_RESIDENCY_PATHS);
    for site in sites {
        assert_eq!(site.bytes().len(), 8);
        assert_eq!(
            site.instructions()[0].kind,
            K::Jal {
                target: 0x8011_0000,
            }
        );
        assert_eq!(
            site.instructions()[1].kind,
            K::Andi {
                rt: Reg::A0,
                rs: Reg::V0,
                imm: 0x00FF,
            }
        );
    }
}
