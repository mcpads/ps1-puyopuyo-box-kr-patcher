use super::*;

fn synthetic_chain() -> Vec<u8> {
    let mut raw = vec![0; MMENU_ROOT_OFFSET + LAZY_INIT_SPECS.len() * LAZY_INIT_BLOCK_BYTES];
    for (index, spec) in LAZY_INIT_SPECS.iter().enumerate() {
        let offset = MMENU_ROOT_OFFSET + index * LAZY_INIT_BLOCK_BYTES;
        raw[offset..offset + 4].copy_from_slice(&[0x12, 0x46, GLOBAL_WORD_INDEX, spec.mask]);
        raw[offset + 4..offset + 8].copy_from_slice(
            &runtime_at(offset + LAZY_INIT_BLOCK_BYTES)
                .unwrap()
                .to_le_bytes(),
        );
        raw[offset + 8..offset + 12].copy_from_slice(&[0x0F, 0x47, GLOBAL_WORD_INDEX, spec.mask]);
        raw[offset + 0x18..offset + 0x1C].copy_from_slice(&[0x28, 0x12, 0x97, 0]);
        raw[offset + 0x1C..offset + 0x1E].copy_from_slice(&spec.resource_id.to_le_bytes());
    }
    raw
}

#[test]
fn 여덟_lazy_init_block의_mask_resource_skip을_함께_요구한다() {
    let blocks = verify_lazy_init_chain(&synthetic_chain()).unwrap();
    assert_eq!(blocks.len(), 8);
    assert_eq!(blocks[1].mask, 2);
    assert_eq!(blocks[1].resource_id, 165);
    assert_eq!(blocks[1].setup_runtime, MM_MES_SETUP_RUNTIME);

    let mut drift = synthetic_chain();
    drift[MMENU_ROOT_OFFSET + LAZY_INIT_BLOCK_BYTES + 3] = 4;
    assert!(verify_lazy_init_chain(&drift).is_err());

    let mut drift = synthetic_chain();
    drift[MMENU_ROOT_OFFSET + 4] ^= 1;
    assert!(verify_lazy_init_chain(&drift).is_err());
}

fn synthetic_module_entry() -> Vec<u8> {
    let mut raw = vec![0; MMENU_INITIALIZED_LEN as usize];
    raw[..4].copy_from_slice(&MMENU_MODULE_KIND.to_le_bytes());
    raw[MMENU_MODULE_ROOT_POINTER_FIELD_OFFSET..MMENU_MODULE_ROOT_POINTER_FIELD_OFFSET + 4]
        .copy_from_slice(&MMENU_MODULE_ROOT_RUNTIME.to_le_bytes());
    raw[MMENU_MODULE_ROOT_OFFSET..MMENU_MODULE_ROOT_OFFSET + 8].copy_from_slice(&[
        0x0F,
        0x40,
        GLOBAL_WORD_INDEX,
        0,
        0xE8,
        0,
        0,
        0,
    ]);
    raw[MMENU_ROOT_CALL_RECORD_OFFSET..MMENU_ROOT_CALL_RECORD_OFFSET + 4]
        .copy_from_slice(&[0x04, 0, 0, 0]);
    raw[MMENU_ROOT_CALL_RECORD_OFFSET + 4..MMENU_ROOT_CALL_RECORD_OFFSET + 8]
        .copy_from_slice(&MMENU_ROOT_RUNTIME.to_le_bytes());
    raw
}

#[test]
fn module_header_root가_op04로_lazy_init_root를_호출한다() {
    let entry = verify_module_entry(&synthetic_module_entry()).unwrap();
    assert_eq!(
        entry,
        MmMesModuleEntry {
            pointer_field_offset: MMENU_MODULE_ROOT_POINTER_FIELD_OFFSET,
            module_root_runtime: MMENU_MODULE_ROOT_RUNTIME,
            root_call_record_offset: MMENU_ROOT_CALL_RECORD_OFFSET,
            root_call_record_runtime: MMENU_ROOT_CALL_RECORD_RUNTIME,
        }
    );

    let mut drift = synthetic_module_entry();
    drift[0] = 4;
    assert!(verify_module_entry(&drift).is_err());

    let mut drift = synthetic_module_entry();
    drift[MMENU_ROOT_CALL_RECORD_OFFSET] = 3;
    assert!(verify_module_entry(&drift).is_err());

    let mut drift = synthetic_module_entry();
    drift[MMENU_MODULE_ROOT_OFFSET + 4] = 0xE7;
    assert!(verify_module_entry(&drift).is_err());
}

#[test]
fn resource_direct_selection_shape는_flags와_정렬에_무관하게_센다() {
    let mut raw = vec![0; 24];
    raw[1..9].copy_from_slice(&[0x28, 0x10, 0x97, 1, 62, 0, 0, 0]);
    raw[12..20].copy_from_slice(&[0x28, 0x10, 0x55, 1, 63, 0, 1, 0]);
    assert_eq!(
        direct_type1_selector_records(&raw),
        vec![
            DirectType1SelectorRecord {
                offset: 1,
                resource_id: 62,
                reserved: 0,
            },
            DirectType1SelectorRecord {
                offset: 12,
                resource_id: 63,
                reserved: 1,
            },
        ]
    );
}
