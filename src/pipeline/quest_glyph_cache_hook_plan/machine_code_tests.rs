use super::super::{
    QUEST_FONT_DRAW_DESCRIPTOR_SITE_SOURCE_ID, QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_SOURCE_ID,
    QUEST_FONT_OBJECT_MARKER_SITE_SOURCE_ID, QUEST_GLYPH_CACHE_ASSIGNMENT_MODE_STACK_OFFSET,
    QUEST_GLYPH_CACHE_FRAME_BYTES, QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME,
};
use super::*;
use crate::disasm::{
    QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME, QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME,
    QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME, QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
    QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, QUEST_FONT_UPLOAD_TARGET_RUNTIME,
    QUEST_GLYPH_RENDERER_CONTINUATION_RUNTIME, QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
    QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET, QUEST_ITEM_NAME_CALLBACK_EPILOGUE_RUNTIME,
    QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES, QUEST_ITEM_NAME_GLYPH_WRITER_RETURN_RUNTIME,
    QUEST_ITEM_NAME_RESIDENCY_CONTINUATION_RUNTIME, QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES,
    QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME, QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_RETURN_RUNTIME,
    QUEST_ITEM_RECORD_TABLE_RUNTIME, QUEST_PAGE_CARRIER_CAPACITY_BYTES,
    QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET, QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
    QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME, QUEST_SEQ_TEXT_CONSUMER_RUNTIME,
    QUEST_SEQ_TEXT_GLYPH_RENDERER_RETURN_RUNTIME,
};
use crate::pipeline::quest_glyph_atlas_bank_policy::QuestGlyphAtlasBank;
use crate::pipeline::quest_item_glyph_cache_dispatch::{
    QUEST_ITEM_DESCRIPTION_MODE_CONSTRUCTOR, QUEST_ITEM_NAME_MODE_DIRECT_GLYPH,
    QUEST_ITEM_NAME_MODE_WHOLE_STRING, QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME,
    QUEST_ITEM_TEXT_MODE_RESIDENCY,
};
use crate::pipeline::quest_item_name_glyph_cache_hook::{
    assemble_item_name_direct_glyph_sites, assemble_item_name_residency_site,
    assemble_item_name_whole_string_sites, assemble_item_text_mode_residency_site,
};
use crate::pipeline::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use crate::script::BOX_QUEST_ITEM_RECORDS;
use std::collections::BTreeSet;

const TEST_BODY_OFFSET: usize = 2_048;

fn test_layout() -> QuestGlyphCacheHookLayout {
    QuestGlyphCacheHookLayout {
        slot_rects_offset: 0,
        korean_descriptor_offset: 16,
        glyph_row_dictionary_offset: 64,
        glyph_cell_indices_offset: 512,
        page_directory_offset: 1_050,
        page_assignments_offset: 1_200,
        scratch_offset: 1_600,
    }
}

fn test_body_runtime() -> u32 {
    QUEST_PAGE_CARRIER_START_RUNTIME + TEST_BODY_OFFSET as u32
}

fn test_body() -> Result<AssembledQuestGlyphCacheHookBody, QuestGlyphCacheHookPlanError> {
    assemble_body_for_layout(test_layout(), 4, 32, 64, test_body_runtime())
}

fn test_item_name_body() -> Result<AssembledQuestGlyphCacheHookBody, QuestGlyphCacheHookPlanError> {
    assemble_body_for_layout_with_item_name_page(
        test_layout(),
        4,
        32,
        64,
        test_body_runtime(),
        Some(4),
    )
}

fn test_multi_page_item_body(
) -> Result<AssembledQuestGlyphCacheHookBody, QuestGlyphCacheHookPlanError> {
    assemble_body_for_layout_with_item_text_pages(
        test_layout(),
        4,
        32,
        64,
        test_body_runtime(),
        Some(QuestItemTextPageMode {
            default_name_page_id: 3,
            first_page_id: 3,
            last_page_id: 4,
            shared_slots: 0,
        }),
    )
}

fn address_load_index(
    instructions: &[crate::isa::Instruction],
    register: Reg,
    value: u32,
) -> usize {
    instructions
        .windows(2)
        .position(|pair| {
            pair[0].kind
                == K::Lui {
                    rt: register,
                    imm: (value.wrapping_add(0x8000) >> 16) as u16,
                }
                && pair[1].kind
                    == K::Addiu {
                        rt: register,
                        rs: register,
                        imm: value as u16 as i16,
                    }
        })
        .unwrap()
}

fn mode_dispatch_target(
    instructions: &[crate::isa::Instruction],
    dispatch_start: usize,
    mode: i16,
) -> u32 {
    instructions[dispatch_start..]
        .windows(2)
        .find_map(|pair| {
            if pair[0].kind
                != (K::Addiu {
                    rt: Reg::T1,
                    rs: Reg::ZERO,
                    imm: mode,
                })
            {
                return None;
            }
            if !matches!(
                pair[1].kind,
                K::Beq {
                    rs: Reg::T0,
                    rt: Reg::T1,
                    ..
                }
            ) {
                return None;
            }
            match pair[1].flow_kind() {
                crate::isa::FlowKind::Branch { taken } => Some(taken),
                _ => None,
            }
        })
        .unwrap()
}

fn default_dispatch_target(instructions: &[crate::isa::Instruction], dispatch_start: usize) -> u32 {
    instructions[dispatch_start..]
        .iter()
        .find_map(|instruction| {
            if !matches!(
                instruction.kind,
                K::Beq {
                    rs: Reg::ZERO,
                    rt: Reg::ZERO,
                    ..
                }
            ) {
                return None;
            }
            match instruction.flow_kind() {
                crate::isa::FlowKind::Branch { taken } => Some(taken),
                _ => None,
            }
        })
        .unwrap()
}

#[test]
fn 합성_package_section과_scratch와_code는_순서와_정렬을_보존한다() {
    let layout = test_layout();
    assert!(layout.slot_rects_offset < layout.glyph_row_dictionary_offset);
    assert!(layout.slot_rects_offset < layout.korean_descriptor_offset);
    assert!(layout.korean_descriptor_offset < layout.glyph_row_dictionary_offset);
    assert!(layout.glyph_row_dictionary_offset < layout.glyph_cell_indices_offset);
    assert!(layout.glyph_cell_indices_offset < layout.page_directory_offset);
    assert!(layout.page_directory_offset < layout.page_assignments_offset);
    assert!(layout.page_assignments_offset < layout.scratch_offset);
    assert!(layout.scratch_offset < TEST_BODY_OFFSET);
    assert!(layout.scratch_offset.is_multiple_of(4));
}

#[test]
fn typed_body는_carrier에들고_load_delay를_검산한다() {
    let body = test_body().unwrap().code;
    assert!(TEST_BODY_OFFSET + body.bytes().len() <= QUEST_PAGE_CARRIER_CAPACITY_BYTES);
    verify_load_delays(&body).unwrap();
}

#[test]
fn body는_역전된page_assignment를_absolute_u16으로_읽는다() {
    let body = test_body().unwrap().code;
    let kinds = body
        .instructions()
        .iter()
        .map(|instruction| instruction.kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&K::Addiu {
        rt: Reg::T1,
        rs: Reg::ZERO,
        imm: 0x0080,
    }));
    assert!(kinds.contains(&K::Sw {
        rt: Reg::T0,
        base: Reg::SP,
        offset: 64,
    }));
    assert!(kinds.contains(&K::Lbu {
        rt: Reg::S4,
        base: Reg::S2,
        offset: 0,
    }));
    assert!(kinds.contains(&K::Lbu {
        rt: Reg::T0,
        base: Reg::S2,
        offset: 1,
    }));
    assert!(!kinds
        .iter()
        .any(|kind| matches!(kind, K::Lhu { base: Reg::S2, .. })));
}

#[test]
fn 공용loader의_모든진입은_assignment_mode를_자기stack에둔다() {
    let body = test_item_name_body().unwrap().code;
    let kinds = body
        .instructions()
        .iter()
        .map(|instruction| instruction.kind)
        .collect::<Vec<_>>();
    let entries = kinds
        .iter()
        .filter(|kind| {
            **kind
                == K::Addiu {
                    rt: Reg::SP,
                    rs: Reg::SP,
                    imm: -QUEST_GLYPH_CACHE_FRAME_BYTES,
                }
        })
        .count();
    let mode_initializers = kinds
        .iter()
        .filter(|kind| {
            **kind
                == K::Sw {
                    rt: Reg::ZERO,
                    base: Reg::SP,
                    offset: QUEST_GLYPH_CACHE_ASSIGNMENT_MODE_STACK_OFFSET,
                }
        })
        .count();

    assert_eq!(entries, 5);
    assert_eq!(mode_initializers, entries);
    assert!(!kinds.contains(&K::Addiu {
        rt: Reg::SP,
        rs: Reg::SP,
        imm: -64,
    }));
}

#[test]
fn item_page끼리는_primary_pin을_교체한다() {
    let body = test_item_name_body().unwrap().code;
    let kinds = body
        .instructions()
        .iter()
        .map(|instruction| instruction.kind)
        .collect::<Vec<_>>();
    let replacement = kinds
        .iter()
        .position(|kind| {
            *kind
                == K::And {
                    rd: Reg::T2,
                    rs: Reg::T0,
                    rt: Reg::T1,
                }
        })
        .unwrap();

    assert!(matches!(
        kinds[replacement + 1],
        K::Bne {
            rs: Reg::T2,
            rt: Reg::ZERO,
            offset
        } if offset != 0
    ));
}

#[test]
fn site는_package뒤_typed_body로_직접_jump한다() {
    let site = assemble_site_for_runtime(test_body_runtime()).unwrap();
    assert_eq!(site.len_words(), 2);
    assert_eq!(
        site.instructions()
            .iter()
            .map(|instruction| instruction.kind)
            .collect::<Vec<_>>(),
        vec![
            K::J {
                target: test_body_runtime()
            },
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ]
    );
}

#[test]
fn body는_relocated_target계산과_page_metadata를_읽는다() {
    let body = test_body().unwrap().code;
    let kinds = body
        .instructions()
        .iter()
        .map(|instruction| instruction.kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&K::Lbu {
        rt: Reg::S1,
        base: Reg::S0,
        offset: 1,
    }));
    assert!(kinds.contains(&K::Lhu {
        rt: Reg::T0,
        base: Reg::S0,
        offset: 2,
    }));
    assert!(kinds.contains(&K::Addiu {
        rt: Reg::S0,
        rs: Reg::S0,
        imm: -4,
    }));
}

#[test]
fn body는_개별cell을_upload하고_drawsync뒤에만_state를_commit한다() {
    let body = test_body().unwrap().code;
    let instructions = body.instructions();
    let upload = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Jal {
                    target: QUEST_FONT_UPLOAD_TARGET_RUNTIME,
                }
        })
        .unwrap();
    let sync = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Jal {
                    target: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
                }
        })
        .unwrap();
    let sync_count = instructions
        .iter()
        .filter(|instruction| {
            instruction.kind
                == K::Jal {
                    target: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
                }
        })
        .count();
    let commit = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Sb {
                    rt: Reg::S1,
                    base: Reg::T0,
                    offset: 0,
                }
        })
        .unwrap();
    assert_eq!(sync, upload + 2);
    assert_eq!(sync_count, 1);
    assert_eq!(
        instructions[sync + 1].kind,
        K::Addu {
            rd: Reg::A0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        }
    );
    assert_eq!(
        instructions[sync + 2].kind,
        K::Addiu {
            rt: Reg::S5,
            rs: Reg::S5,
            imm: 1,
        }
    );
    assert!(sync < commit);
}

#[test]
fn body는_bank별_pin을모은뒤_모든bank가pinned일때만_yield한다() {
    let body = test_body().unwrap().code;
    let instructions = body.instructions();
    let marker_read = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Lbu {
                    rt: Reg::T3,
                    base: Reg::T0,
                    offset: QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
                }
        })
        .unwrap();
    let unmarked_target = match instructions[marker_read + 2].flow_kind() {
        crate::isa::FlowKind::Branch { taken } => instructions
            .iter()
            .position(|instruction| instruction.addr == taken)
            .unwrap(),
        other => panic!("unmarked object 분기가 아님: {other:?}"),
    };
    assert_eq!(
        instructions[unmarked_target].kind,
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::T2,
            imm: -1,
        }
    );
    assert!(instructions[marker_read..unmarked_target]
        .iter()
        .any(|instruction| instruction.kind
            == K::Srl {
                rd: Reg::T3,
                rt: Reg::T3,
                sa: 7,
            }));
    for mask in [1, 2] {
        assert!(instructions.iter().any(|instruction| instruction.kind
            == K::Andi {
                rt: Reg::T0,
                rs: Reg::T4,
                imm: mask,
            }));
    }
    let yield_store = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Sw {
                    rt: Reg::V0,
                    base: Reg::GP,
                    offset: QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
                }
        })
        .unwrap();
    assert_eq!(
        instructions[yield_store - 1].kind,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        }
    );
    assert_eq!(instructions[yield_store + 1].kind, K::Jr { rs: Reg::RA });
    assert!(instructions.iter().any(|instruction| instruction.kind
        == K::Addiu {
            rt: Reg::T1,
            rs: Reg::T1,
            imm: (QUEST_GLYPH_CACHE_SECONDARY_VRAM_X - QUEST_GLYPH_CACHE_VRAM_X) as i16,
        }));
    assert!(instructions.iter().any(|instruction| instruction.kind
        == K::Sb {
            rt: Reg::S1,
            base: Reg::T0,
            offset: QuestGlyphAtlasBank::Secondary.state_offset(),
        }));
}

#[test]
fn unpinned_resident_state는_resident복귀대신_upload경로로보낸다() {
    let body = test_body().unwrap().code;
    let instructions = body.instructions();
    let marker_read = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Lbu {
                    rt: Reg::T3,
                    base: Reg::T0,
                    offset: QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
                }
        })
        .unwrap();
    let upload_call = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Jal {
                    target: QUEST_FONT_UPLOAD_TARGET_RUNTIME,
                }
        })
        .unwrap();

    for (state_register, pin_mask) in [(Reg::T1, 1), (Reg::T2, 2)] {
        let decision = instructions[marker_read..]
            .windows(6)
            .position(|window| {
                matches!(
                    window[0].kind,
                    K::Bne {
                        rs,
                        rt: Reg::S1,
                        ..
                    } if rs == state_register
                ) && window[2].kind
                    == K::Andi {
                        rt: Reg::T0,
                        rs: Reg::T4,
                        imm: pin_mask,
                    }
                    && matches!(
                        window[3].kind,
                        K::Bne {
                            rs: Reg::T0,
                            rt: Reg::ZERO,
                            ..
                        }
                    )
                    && matches!(window[5].kind, K::J { .. })
            })
            .map(|index| marker_read + index)
            .unwrap();
        let resident_target = match instructions[decision + 3].flow_kind() {
            crate::isa::FlowKind::Branch { taken } => taken,
            other => panic!("resident pin 분기가 아님: {other:?}"),
        };
        let stale_target = match instructions[decision + 5].flow_kind() {
            crate::isa::FlowKind::Jump { target } => target,
            other => panic!("stale page upload jump가 아님: {other:?}"),
        };
        assert_ne!(resident_target, stale_target);
        let stale_target_index = instructions
            .iter()
            .position(|instruction| instruction.addr == stale_target)
            .unwrap();
        assert!(decision < stale_target_index);
        assert!(stale_target_index < upload_call);
        assert!(!instructions[stale_target_index..upload_call]
            .iter()
            .any(|instruction| instruction.kind == K::Jr { rs: Reg::RA }));
    }
}

#[test]
fn renderer_guard는_op21에만_route_page를전달하고원본prologue를재생한다() {
    let assembled = test_body().unwrap();
    let site =
        assemble_original_font_guard_site_for_runtime(assembled.original_font_guard_entry_runtime)
            .unwrap();
    assert_eq!(
        site.instructions()
            .iter()
            .map(|instruction| instruction.kind)
            .collect::<Vec<_>>(),
        vec![
            K::J {
                target: assembled.original_font_guard_entry_runtime,
            },
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ]
    );

    let instructions = assembled.code.instructions();
    let caller_address_load = instructions
        .windows(2)
        .position(|pair| {
            pair[0].kind
                == K::Lui {
                    rt: Reg::T1,
                    imm: (QUEST_SEQ_TEXT_GLYPH_RENDERER_RETURN_RUNTIME.wrapping_add(0x8000) >> 16)
                        as u16,
                }
                && pair[1].kind
                    == K::Addiu {
                        rt: Reg::T1,
                        rs: Reg::T1,
                        imm: QUEST_SEQ_TEXT_GLYPH_RENDERER_RETURN_RUNTIME as u16 as i16,
                    }
        })
        .unwrap();
    let original_replay = match instructions[caller_address_load + 2].flow_kind() {
        crate::isa::FlowKind::Branch { taken } => instructions
            .iter()
            .position(|instruction| instruction.addr == taken)
            .unwrap(),
        other => panic!("non-OP21 replay 분기가 아님: {other:?}"),
    };
    assert_eq!(
        instructions[original_replay].kind,
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -32,
        }
    );
    assert!(instructions[original_replay..].iter().any(|instruction| {
        instruction.kind
            == K::J {
                target: QUEST_GLYPH_RENDERER_CONTINUATION_RUNTIME,
            }
    }));
    assert!(instructions.iter().any(|instruction| {
        instruction.kind
            == K::Sb {
                rt: Reg::T1,
                base: Reg::T0,
                offset: 0,
            }
    }));
}

#[test]
fn item_name_site는_파생page_entry를_typed_jal로호출한다() {
    let assembled = test_item_name_body().unwrap();
    let entry = assembled.item_name_entry_runtime.unwrap();
    let site = assemble_item_name_residency_site(entry).unwrap();
    assert_eq!(
        site.instructions()
            .iter()
            .map(|instruction| instruction.kind)
            .collect::<Vec<_>>(),
        vec![
            K::Jal { target: entry },
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ]
    );

    let instructions = assembled.code.instructions();
    let entry_index = instructions
        .iter()
        .position(|instruction| instruction.addr == entry)
        .unwrap();
    assert_eq!(
        instructions[entry_index].kind,
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -QUEST_GLYPH_CACHE_FRAME_BYTES,
        }
    );
    assert!(instructions[entry_index..].iter().any(|instruction| {
        instruction.kind
            == K::Addiu {
                rt: Reg::S1,
                rs: Reg::ZERO,
                imm: 4,
            }
    }));
    let page_request = instructions[entry_index..]
        .iter()
        .position(|instruction| matches!(instruction.kind, K::J { .. }))
        .map(|index| entry_index + index)
        .unwrap();
    let page_request_target = match instructions[page_request].flow_kind() {
        crate::isa::FlowKind::Jump { target, .. } => target,
        other => panic!("item page request jump가 아님: {other:?}"),
    };
    let page_request_target = instructions
        .iter()
        .position(|instruction| instruction.addr == page_request_target)
        .unwrap();
    assert_eq!(
        instructions[page_request_target].kind,
        K::Beq {
            rs: Reg::S1,
            rt: Reg::ZERO,
            offset: match instructions[page_request_target].kind {
                K::Beq { offset, .. } => offset,
                _ => unreachable!(),
            },
        }
    );
}

#[test]
fn direct_item_name_calls는_같은_파생page_retry_entry를호출한다() {
    let assembled = test_item_name_body().unwrap();
    let entry = assembled.item_name_direct_glyph_entry_runtime.unwrap();
    let sites = assemble_item_name_direct_glyph_sites(entry).unwrap();
    assert_eq!(
        sites
            .iter()
            .map(VerifiedMachineCode::base_addr)
            .collect::<Vec<_>>(),
        QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES
    );
    assert!(sites.iter().all(|site| {
        site.instructions()
            .iter()
            .map(|instruction| instruction.kind)
            .eq([
                K::Jal { target: entry },
                K::Addiu {
                    rt: Reg::S0,
                    rs: Reg::S0,
                    imm: 2,
                },
            ])
    }));

    let instructions = assembled.code.instructions();
    let entry_index = instructions
        .iter()
        .position(|instruction| instruction.addr == entry)
        .unwrap();
    assert!(instructions[entry_index..entry_index + 18]
        .iter()
        .any(|instruction| {
            instruction.kind
                == K::Addiu {
                    rt: Reg::T0,
                    rs: Reg::ZERO,
                    imm: 2,
                }
        }));
    assert!(instructions[entry_index..entry_index + 18]
        .iter()
        .any(|instruction| {
            instruction.kind
                == K::Addiu {
                    rt: Reg::S1,
                    rs: Reg::ZERO,
                    imm: 4,
                }
        }));
}

#[test]
fn whole_string_item_name_calls는_active_scope안에서만_원본loop를호출한다() {
    let assembled = test_item_name_body().unwrap();
    let entry = assembled.item_name_whole_string_entry_runtime.unwrap();
    let sites = assemble_item_name_whole_string_sites(entry).unwrap();
    assert_eq!(
        sites
            .iter()
            .map(VerifiedMachineCode::base_addr)
            .collect::<Vec<_>>(),
        QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES
    );
    assert!(sites.iter().all(|site| {
        site.instructions()
            .iter()
            .map(|instruction| instruction.kind)
            .eq([
                K::Jal { target: entry },
                K::Addu {
                    rd: Reg::A0,
                    rs: Reg::ZERO,
                    rt: Reg::ZERO,
                },
            ])
    }));

    let instructions = assembled.code.instructions();
    let entry_index = instructions
        .iter()
        .position(|instruction| instruction.addr == entry)
        .unwrap();
    assert!(instructions[entry_index..entry_index + 18]
        .iter()
        .any(|instruction| {
            instruction.kind
                == K::Addiu {
                    rt: Reg::T0,
                    rs: Reg::ZERO,
                    imm: 3,
                }
        }));

    let draw_call = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Jal {
                    target: QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
                }
        })
        .unwrap();
    let active_set = instructions[..draw_call]
        .iter()
        .rposition(|instruction| {
            instruction.kind
                == K::Sb {
                    rt: Reg::T1,
                    base: Reg::T0,
                    offset: 0,
                }
        })
        .unwrap();
    let active_clear = instructions[draw_call + 1..]
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Sb {
                    rt: Reg::ZERO,
                    base: Reg::T0,
                    offset: 0,
                }
        })
        .map(|index| draw_call + 1 + index)
        .unwrap();
    assert!(active_set < draw_call && draw_call < active_clear);
    assert!(instructions[..active_set].windows(2).any(|pair| {
        pair[0].kind
            == K::Lui {
                rt: Reg::T0,
                imm: (QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME.wrapping_add(0x8000) >> 16)
                    as u16,
            }
            && pair[1].kind
                == K::Addiu {
                    rt: Reg::T0,
                    rs: Reg::T0,
                    imm: QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME as u16 as i16,
                }
    }));
}

#[test]
fn item_description_direct_text는_본문앞_metadata를검증한뒤_page를준비한다() {
    let assembled = test_item_name_body().unwrap();
    let entry = assembled.item_text_mode_entry_runtime.unwrap();
    let site = assemble_item_text_mode_residency_site(entry).unwrap();
    assert_eq!(site.base_addr(), QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME);
    assert_eq!(
        site.instructions()
            .iter()
            .map(|instruction| instruction.kind)
            .collect::<Vec<_>>(),
        vec![
            K::Jal { target: entry },
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ]
    );

    let instructions = assembled.code.instructions();
    let entry_index = instructions
        .iter()
        .position(|instruction| instruction.addr == entry)
        .unwrap();
    let metadata_guard = &instructions[entry_index..entry_index + 23];
    assert_eq!(
        metadata_guard[0].kind,
        K::Lw {
            rt: Reg::T0,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        }
    );
    assert_eq!(
        metadata_guard
            .iter()
            .filter_map(|instruction| match instruction.kind {
                K::Lbu {
                    base: Reg::T0,
                    offset,
                    ..
                } if (-4..=-1).contains(&offset) => Some(offset),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![-4, -2, -1, -3]
    );
    let bypass_targets = metadata_guard
        .iter()
        .filter_map(|instruction| match instruction.flow_kind() {
            crate::isa::FlowKind::Branch { taken } => Some(taken),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(bypass_targets.len(), 1);
    let bypass = instructions
        .iter()
        .find(|instruction| Some(instruction.addr) == bypass_targets.iter().next().copied())
        .unwrap();
    assert_eq!(
        bypass.kind,
        K::J {
            target: QUEST_SEQ_TEXT_CONSUMER_RUNTIME,
        }
    );
    assert!(instructions[entry_index + 23..].iter().any(|instruction| {
        instruction.kind
            == K::Addiu {
                rt: Reg::T0,
                rs: Reg::ZERO,
                imm: 4,
            }
    }));
}

#[test]
fn item_description_constructor는_대상page가준비된뒤에만_원본을호출한다() {
    let assembled = test_multi_page_item_body().unwrap();
    let entry = assembled
        .item_description_constructor_entry_runtime
        .unwrap();
    let instructions = assembled.code.instructions();
    let entry_index = instructions
        .iter()
        .position(|instruction| instruction.addr == entry)
        .unwrap();
    let frame_index = instructions[entry_index..]
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Addiu {
                    rt: Reg::SP,
                    rs: Reg::SP,
                    imm: -QUEST_GLYPH_CACHE_FRAME_BYTES,
                }
        })
        .map(|offset| entry_index + offset)
        .unwrap();
    let metadata_guard = &instructions[entry_index..frame_index];

    assert_eq!(
        metadata_guard[0].kind,
        K::Andi {
            rt: Reg::T0,
            rs: Reg::A0,
            imm: 0x00FF,
        }
    );
    let item_id_range_guard = metadata_guard
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Sltiu {
                    rt: Reg::T2,
                    rs: Reg::T0,
                    imm: i16::try_from(BOX_QUEST_ITEM_RECORDS).unwrap(),
                }
        })
        .unwrap();
    let table_address_load = metadata_guard
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Lui {
                    rt: Reg::T1,
                    imm: (QUEST_ITEM_RECORD_TABLE_RUNTIME.wrapping_add(0x8000) >> 16) as u16,
                }
        })
        .unwrap();
    assert!(item_id_range_guard < table_address_load);
    assert!(matches!(
        metadata_guard[item_id_range_guard + 1].kind,
        K::Beq {
            rs: Reg::T2,
            rt: Reg::ZERO,
            ..
        }
    ));
    assert!(metadata_guard.windows(2).any(|pair| {
        pair[0].kind
            == K::Lui {
                rt: Reg::T1,
                imm: (QUEST_ITEM_RECORD_TABLE_RUNTIME.wrapping_add(0x8000) >> 16) as u16,
            }
            && pair[1].kind
                == K::Addiu {
                    rt: Reg::T1,
                    rs: Reg::T1,
                    imm: QUEST_ITEM_RECORD_TABLE_RUNTIME as u16 as i16,
                }
    }));
    assert!(metadata_guard.iter().any(|instruction| {
        instruction.kind
            == K::Lw {
                rt: Reg::T0,
                base: Reg::T0,
                offset: QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET,
            }
    }));
    assert_eq!(
        metadata_guard
            .iter()
            .filter_map(|instruction| match instruction.kind {
                K::Lbu {
                    base: Reg::T0,
                    offset,
                    ..
                } if (-4..=-1).contains(&offset) => Some(offset),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![-4, -2, -1, -3]
    );
    let bypass_targets = metadata_guard
        .iter()
        .filter_map(|instruction| match instruction.flow_kind() {
            crate::isa::FlowKind::Branch { taken } => Some(taken),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(bypass_targets.len(), 1);
    let bypass = instructions
        .iter()
        .position(|instruction| bypass_targets.contains(&instruction.addr))
        .unwrap();
    assert_eq!(
        instructions[bypass].kind,
        K::J {
            target: QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
        }
    );
}

#[test]
fn item_description_constructor는_primary에_같은page가있으면_frame저장없이_원본으로간다() {
    let assembled = test_multi_page_item_body().unwrap();
    let entry = assembled
        .item_description_constructor_entry_runtime
        .unwrap();
    let instructions = assembled.code.instructions();
    let entry_index = instructions
        .iter()
        .position(|instruction| instruction.addr == entry)
        .unwrap();
    let frame_index = instructions[entry_index..]
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Addiu {
                    rt: Reg::SP,
                    rs: Reg::SP,
                    imm: -QUEST_GLYPH_CACHE_FRAME_BYTES,
                }
        })
        .map(|offset| entry_index + offset)
        .unwrap();
    let fast_path = &instructions[entry_index..frame_index];

    assert!(fast_path.windows(2).any(|pair| {
        pair[0].kind
            == K::Lui {
                rt: Reg::T2,
                imm: (QUEST_PAGE_HOOK_STATE_RUNTIME.wrapping_add(0x8000) >> 16) as u16,
            }
            && pair[1].kind
                == K::Addiu {
                    rt: Reg::T2,
                    rs: Reg::T2,
                    imm: QUEST_PAGE_HOOK_STATE_RUNTIME as u16 as i16,
                }
    }));
    assert!(fast_path.windows(2).any(|pair| {
        pair[0].kind
            == K::Lbu {
                rt: Reg::T2,
                base: Reg::T2,
                offset: 0,
            }
            && pair[1].kind
                == K::Sll {
                    rd: Reg::ZERO,
                    rt: Reg::ZERO,
                    sa: 0,
                }
    }));
    let resident_branch = fast_path
        .iter()
        .find(|instruction| {
            matches!(
                instruction.kind,
                K::Beq {
                    rs: Reg::T2,
                    rt: Reg::T1,
                    ..
                }
            )
        })
        .unwrap();
    let target = match resident_branch.flow_kind() {
        crate::isa::FlowKind::Branch { taken } => taken,
        other => panic!("primary resident fast branch가 아님: {other:?}"),
    };
    assert_eq!(
        instructions
            .iter()
            .find(|instruction| instruction.addr == target)
            .unwrap()
            .kind,
        K::J {
            target: QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
        }
    );
    assert!(!fast_path
        .iter()
        .any(|instruction| matches!(instruction.kind, K::Sw { base: Reg::SP, .. })));
}

#[test]
fn item_name_page는_성공때만_draw로복귀하고_yield때state를소비하지않는다() {
    let body = test_item_name_body().unwrap().code;
    let instructions = body.instructions();
    let success_jump = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::J {
                    target: QUEST_ITEM_NAME_RESIDENCY_CONTINUATION_RUNTIME,
                }
        })
        .unwrap();
    assert_eq!(
        instructions[success_jump + 1].kind,
        K::Addu {
            rd: Reg::S2,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        }
    );

    let retry_jump = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::J {
                    target: QUEST_ITEM_NAME_CALLBACK_EPILOGUE_RUNTIME,
                }
        })
        .unwrap();
    assert_eq!(
        instructions[retry_jump + 1].kind,
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        }
    );

    let yield_store = instructions
        .iter()
        .position(|instruction| {
            instruction.kind
                == K::Sw {
                    rt: Reg::V0,
                    base: Reg::GP,
                    offset: QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
                }
        })
        .unwrap();
    let item_retry_branch = instructions[..yield_store]
        .iter()
        .rposition(|instruction| {
            matches!(
                instruction.kind,
                K::Bne {
                    rs: Reg::T0,
                    rt: Reg::ZERO,
                    ..
                }
            )
        })
        .unwrap();
    let retry_target = match instructions[item_retry_branch].flow_kind() {
        crate::isa::FlowKind::Branch { taken } => taken,
        other => panic!("item retry branch가 아님: {other:?}"),
    };
    let retry_dispatch = instructions
        .iter()
        .position(|instruction| instruction.addr == retry_target)
        .unwrap();
    let direct_retry_target = mode_dispatch_target(
        instructions,
        retry_dispatch,
        QUEST_ITEM_NAME_MODE_DIRECT_GLYPH,
    );
    let direct_retry = instructions
        .iter()
        .position(|instruction| instruction.addr == direct_retry_target)
        .unwrap();
    let direct_retry_return = instructions[direct_retry..]
        .iter()
        .position(|instruction| instruction.kind == K::Jr { rs: Reg::RA })
        .map(|index| direct_retry + index)
        .unwrap();
    assert!(!instructions[direct_retry..direct_retry_return]
        .iter()
        .any(|instruction| matches!(instruction.kind, K::Sb { .. } | K::Sh { .. } | K::Sw { .. })));
    let whole_string_retry_target = mode_dispatch_target(
        instructions,
        retry_dispatch,
        QUEST_ITEM_NAME_MODE_WHOLE_STRING,
    );
    let whole_string_retry = instructions
        .iter()
        .position(|instruction| instruction.addr == whole_string_retry_target)
        .unwrap();
    let whole_string_retry_return = instructions[whole_string_retry..]
        .iter()
        .position(|instruction| instruction.kind == K::Jr { rs: Reg::RA })
        .map(|index| whole_string_retry + index)
        .unwrap();
    assert!(!instructions[whole_string_retry..whole_string_retry_return]
        .iter()
        .any(|instruction| instruction.kind
            == K::Jal {
                target: QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
            }));
    let text_mode_retry_target =
        mode_dispatch_target(instructions, retry_dispatch, QUEST_ITEM_TEXT_MODE_RESIDENCY);
    let text_mode_retry = instructions
        .iter()
        .position(|instruction| instruction.addr == text_mode_retry_target)
        .unwrap();
    let text_mode_retry_return = instructions[text_mode_retry..]
        .iter()
        .position(|instruction| instruction.kind == K::Jr { rs: Reg::RA })
        .map(|index| text_mode_retry + index)
        .unwrap();
    assert!(!instructions[text_mode_retry..text_mode_retry_return]
        .iter()
        .any(|instruction| matches!(instruction.kind, K::Sb { .. } | K::Sh { .. } | K::Sw { .. })));
    let description_retry_target = mode_dispatch_target(
        instructions,
        retry_dispatch,
        QUEST_ITEM_DESCRIPTION_MODE_CONSTRUCTOR,
    );
    let description_retry = instructions
        .iter()
        .position(|instruction| instruction.addr == description_retry_target)
        .unwrap();
    assert!(instructions[description_retry..]
        .iter()
        .take(20)
        .any(|instruction| instruction.kind == K::Jr { rs: Reg::RA }));
    assert!(!instructions[description_retry..]
        .iter()
        .take_while(|instruction| instruction.kind != K::Jr { rs: Reg::RA })
        .any(|instruction| instruction.kind
            == K::J {
                target: QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
            }));
    let callback_retry_target = default_dispatch_target(instructions, retry_dispatch);
    let callback_retry = instructions[..retry_jump]
        .iter()
        .rposition(|instruction| {
            instruction.kind
                == K::Lw {
                    rt: Reg::A0,
                    base: Reg::SP,
                    offset: 56,
                }
        })
        .unwrap();
    assert_eq!(callback_retry_target, instructions[callback_retry].addr);

    assert!(instructions.iter().any(|instruction| {
        instruction.kind
            == K::J {
                target: QUEST_GLYPH_RENDERER_RUNTIME,
            }
    }));
}

#[test]
fn item_text_page는_secondary를건너뛰고_primary_pin에서재시도한다() {
    let body = test_item_name_body().unwrap().code;
    let instructions = body.instructions();
    let mode_guard_targets = instructions
        .iter()
        .filter_map(|instruction| {
            if !matches!(
                instruction.kind,
                K::Bne {
                    rs: Reg::T3,
                    rt: Reg::ZERO,
                    ..
                }
            ) {
                return None;
            }
            match instruction.flow_kind() {
                crate::isa::FlowKind::Branch { taken } => Some(taken),
                _ => None,
            }
        })
        .collect::<Vec<_>>();

    assert!(mode_guard_targets.iter().any(|target| {
        instructions
            .iter()
            .find(|instruction| instruction.addr == *target)
            .is_some_and(|instruction| {
                instruction.kind
                    == K::Andi {
                        rt: Reg::T0,
                        rs: Reg::T4,
                        imm: 1,
                    }
            })
    }));
    assert!(mode_guard_targets.iter().any(|target| {
        instructions
            .iter()
            .find(|instruction| instruction.addr == *target)
            .is_some_and(|instruction| {
                instruction.kind
                    == K::Lw {
                        rt: Reg::T0,
                        base: Reg::SP,
                        offset: 12,
                    }
            })
    }));
}

#[test]
fn renderer_guard는_이름과_번역메뉴를_page로보내고_whole_string_scope를_구분한다() {
    let assembled = test_item_name_body().unwrap();
    let instructions = assembled.code.instructions();
    let op21_load = address_load_index(
        instructions,
        Reg::T1,
        QUEST_SEQ_TEXT_GLYPH_RENDERER_RETURN_RUNTIME,
    );
    let item_load = address_load_index(
        instructions,
        Reg::T1,
        QUEST_ITEM_NAME_GLYPH_WRITER_RETURN_RUNTIME,
    );
    let route_load =
        address_load_index(instructions, Reg::T1, QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME);
    let op21_target = match instructions[op21_load + 2].flow_kind() {
        crate::isa::FlowKind::Branch { taken } => taken,
        other => panic!("OP21 route branch가 아님: {other:?}"),
    };
    assert_eq!(op21_target, instructions[route_load].addr);

    let item_target = match instructions[item_load + 2].flow_kind() {
        crate::isa::FlowKind::Branch { taken } => taken,
        other => panic!("item caller filter branch가 아님: {other:?}"),
    };
    assert_eq!(item_target, instructions[route_load].addr);

    let whole_string_load = address_load_index(
        instructions,
        Reg::T1,
        QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_RETURN_RUNTIME,
    );
    // fallthrough와 branch delay의 주소 갱신을 따라 정확한 허용 집합을 복원한다.
    let mut address = 0u32;
    let mut actual = std::collections::BTreeSet::new();
    for instruction in &instructions[item_load + 4..whole_string_load] {
        match instruction.kind {
            K::Lui { rt: Reg::T1, imm } => address = u32::from(imm) << 16,
            K::Addiu {
                rt: Reg::T1,
                rs: Reg::T1,
                imm,
            } => address = address.wrapping_add_signed(i32::from(imm)),
            K::Beq {
                rs: Reg::RA,
                rt: Reg::T1,
                ..
            } => {
                actual.insert(address);
                assert_eq!(
                    instruction.flow_kind(),
                    crate::isa::FlowKind::Branch {
                        taken: instructions[route_load].addr
                    }
                );
            }
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            } => {}
            other => panic!("caller 비교에서 미검증 명령 {other:?}"),
        }
    }
    let expected = QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES
        .into_iter()
        .chain(crate::disasm::QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES)
        .chain(
            crate::disasm::QUEST_MENU_GLYPH_CALLS
                .iter()
                .filter(|c| !c.whole_string)
                .map(|c| c.runtime),
        )
        .map(|runtime| runtime + 8)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected);
    let replay_target = match instructions[whole_string_load + 2].flow_kind() {
        crate::isa::FlowKind::Branch { taken } => taken,
        other => panic!("whole-string caller filter 분기가 아님: {other:?}"),
    };
    let replay = instructions
        .iter()
        .position(|instruction| instruction.addr == replay_target)
        .unwrap();
    assert_eq!(
        instructions[replay].kind,
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -32,
        }
    );
    assert_eq!(
        instructions[whole_string_load + 4].kind,
        K::Lui {
            rt: Reg::T2,
            imm: (QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME.wrapping_add(0x8000) >> 16) as u16,
        }
    );
    assert_eq!(
        instructions[whole_string_load + 5].kind,
        K::Addiu {
            rt: Reg::T2,
            rs: Reg::T2,
            imm: QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME as u16 as i16,
        }
    );
    assert_eq!(
        instructions[whole_string_load + 6].kind,
        K::Lbu {
            rt: Reg::T2,
            base: Reg::T2,
            offset: 0,
        }
    );
    let inactive_target = match instructions[whole_string_load + 8].flow_kind() {
        crate::isa::FlowKind::Branch { taken } => taken,
        other => panic!("whole-string active filter 분기가 아님: {other:?}"),
    };
    assert_eq!(inactive_target, instructions[replay].addr);
    assert_eq!(whole_string_load + 10, route_load);
    // 종료 안내도 번역 메뉴에 포함되어 같은 page로 간다.
    assert!(actual.contains(&crate::disasm::QUEST_ITEM_NAME_FALLBACK_GLYPH_WRITER_RETURN_RUNTIME));
}

#[test]
fn object별_marker_descriptor_texture_page_site를_각각_분리한다() {
    let assembled = test_body().unwrap();
    for (site_runtime, entry_runtime, source_id) in [
        (
            QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME,
            assembled.font_object_marker_entry_runtime,
            QUEST_FONT_OBJECT_MARKER_SITE_SOURCE_ID,
        ),
        (
            QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME,
            assembled.font_draw_descriptor_entry_runtime,
            QUEST_FONT_DRAW_DESCRIPTOR_SITE_SOURCE_ID,
        ),
        (
            QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME,
            assembled.font_draw_texture_page_entry_runtime,
            QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_SOURCE_ID,
        ),
    ] {
        let site =
            assemble_redirect_site_for_runtime(site_runtime, entry_runtime, source_id).unwrap();
        assert_eq!(site.base_addr(), site_runtime);
        assert_eq!(
            site.instructions()[0].kind,
            K::J {
                target: entry_runtime
            }
        );
    }
    assert_eq!(
        assembled.korean_descriptor_runtime,
        QUEST_PAGE_CARRIER_START_RUNTIME + test_layout().korean_descriptor_offset as u32
    );
    assert_eq!(
        korean_texture_page(
            QUEST_GLYPH_CACHE_VRAM_X,
            QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE,
        )
        .unwrap(),
        0x003E
    );
    assert_eq!(
        korean_texture_page(
            QUEST_GLYPH_CACHE_SECONDARY_VRAM_X,
            QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE,
        )
        .unwrap(),
        0x003F
    );
}
