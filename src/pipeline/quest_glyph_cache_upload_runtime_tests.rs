use super::*;
use serde_json::json;

fn place_cell(gpu: &mut [u8], cell: &[u8], x_words: usize, y: usize) {
    for row in 0..QUEST_GLYPH_CACHE_CELL_ROWS {
        let source = row * QUEST_GLYPH_CACHE_CELL_ROW_BYTES;
        let target = (y + row) * PSX_GPU_ROW_BYTES + x_words * 2;
        gpu[target..target + QUEST_GLYPH_CACHE_CELL_ROW_BYTES]
            .copy_from_slice(&cell[source..source + QUEST_GLYPH_CACHE_CELL_ROW_BYTES]);
    }
}

#[test]
fn indexed4_cell은_vram_word좌표에서_행단위로_복원된다() {
    let mut gpu = vec![0_u8; PSX_GPU_RAM_BYTES];
    let cell = (0..QUEST_GLYPH_CACHE_CELL_BYTES)
        .map(|value| value as u8)
        .collect::<Vec<_>>();
    place_cell(&mut gpu, &cell, 583, 31);

    assert_eq!(uploaded_cell(&gpu, 583, 31).unwrap(), cell);
}

#[test]
fn gpu길이와_cell범위가_맞지_않으면_실패한다() {
    assert!(uploaded_cell(&[0; 4], 0, 0).is_err());
    assert!(uploaded_cell(&vec![0; PSX_GPU_RAM_BYTES], 1023, 511).is_err());
}

#[test]
fn runtime_slice는_kseg주소만_main_ram으로_환산한다() {
    let mut ram = vec![0_u8; PSX_MAIN_RAM_LEN];
    let offset = psx_main_ram_offset(QUEST_PAGE_CARRIER_START_RUNTIME, 4).unwrap();
    ram[offset..offset + 4].copy_from_slice(&[1, 2, 3, 4]);

    assert_eq!(
        runtime_slice(&ram, QUEST_PAGE_CARRIER_START_RUNTIME, 4, "test").unwrap(),
        [1, 2, 3, 4]
    );
    assert!(runtime_slice(&ram, 0x8020_0000, 1, "test").is_err());
}

fn exact_upload_events(expected_calls: usize, dropped: u64) -> Vec<u8> {
    let contract = UploadEventContract {
        upload_pc: 10,
        upload_return: 11,
        sync_pc: 20,
        sync_return: 21,
        commit_pc: 30,
    };
    let mut events = Vec::new();
    for index in 0..expected_calls {
        events.push(json!({"pc": contract.upload_pc, "registers": {
            "ra": contract.upload_return, "s5": index,
        }}));
        events.push(json!({"pc": contract.sync_pc, "registers": {
            "ra": contract.sync_return, "s5": index,
        }}));
    }
    events.push(json!({"pc": contract.commit_pc}));
    serde_json::to_vec(&json!({"dropped": dropped, "events": events})).unwrap()
}

#[test]
fn upload_event는_현재_page호출수와_commit의_순서를_검증한다() {
    let contract = UploadEventContract {
        upload_pc: 10,
        upload_return: 11,
        sync_pc: 20,
        sync_return: 21,
        commit_pc: 30,
    };
    let expected_calls = 3;
    let evidence = verify_upload_events_with_contract(
        contract,
        expected_calls,
        &exact_upload_events(expected_calls, 0),
    )
    .unwrap();

    assert_eq!(evidence.load_image_calls, expected_calls);
    assert_eq!(evidence.draw_sync_calls, expected_calls);
    assert_eq!(evidence.page_commit_events, 1);
}

#[test]
fn upload_event_drop은_정확한_sequence로_승격하지_않는다() {
    let contract = UploadEventContract {
        upload_pc: 10,
        upload_return: 11,
        sync_pc: 20,
        sync_return: 21,
        commit_pc: 30,
    };

    assert!(verify_upload_events_with_contract(contract, 2, &exact_upload_events(2, 1)).is_err());
}

#[test]
fn upload과_sync의_순서가_바뀌면_실패한다() {
    let contract = UploadEventContract {
        upload_pc: 10,
        upload_return: 11,
        sync_pc: 20,
        sync_return: 21,
        commit_pc: 30,
    };
    let mut document: Value = serde_json::from_slice(&exact_upload_events(2, 0)).unwrap();
    document["events"].as_array_mut().unwrap().swap(0, 1);
    let raw = serde_json::to_vec(&document).unwrap();

    assert!(verify_upload_events_with_contract(contract, 2, &raw).is_err());
}
