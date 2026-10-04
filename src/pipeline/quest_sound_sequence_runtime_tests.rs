use super::*;

fn write_u16(ram: &mut [u8], runtime: u32, value: u16) {
    let offset = psx_main_ram_offset(runtime, 2).unwrap();
    ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_i16(ram: &mut [u8], runtime: u32, value: i16) {
    let offset = psx_main_ram_offset(runtime, 2).unwrap();
    ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(ram: &mut [u8], runtime: u32, value: u32) {
    let offset = psx_main_ram_offset(runtime, 4).unwrap();
    ram[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_bytes(ram: &mut [u8], runtime: u32, raw: &[u8]) {
    let offset = psx_main_ram_offset(runtime, raw.len()).unwrap();
    ram[offset..offset + raw.len()].copy_from_slice(raw);
}

fn write_descriptor(ram: &mut [u8], destination: u32) {
    let base = QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME;
    write_u16(ram, base, 1);
    write_u16(ram, base + 2, 1);
    write_u32(ram, base + 8, QUEST_WAKEUP_SEQUENCE_BYTES as u32);
    write_u32(ram, base + 12, QUEST_WAKEUP_SEQUENCE_BYTES as u32);
    write_u32(ram, base + 16, destination);
}

fn fixture(event_cursors: [u32; 3]) -> (Vec<u8>, [Vec<u8>; 4]) {
    let sequence = vec![0xA5; QUEST_WAKEUP_SEQUENCE_BYTES];
    let mut rams: [Vec<u8>; 4] = std::array::from_fn(|_| vec![0u8; PSX_MAIN_RAM_LEN]);
    let loader = QUEST_SOUND_LOADER_STATE_RUNTIME;
    write_u16(&mut rams[0], loader, 1);
    write_u16(&mut rams[0], loader + 2, REQUEST_ID);
    write_i16(&mut rams[0], loader + 4, APPLICATION_SLOT);
    write_u32(
        &mut rams[0],
        loader + 8,
        QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME,
    );
    write_u32(&mut rams[0], loader + 12, QUEST_SOUND_STAGING_RUNTIME);
    write_i16(&mut rams[0], loader + 16, -1);
    write_descriptor(&mut rams[0], 0);

    for (index, cursor) in event_cursors.into_iter().enumerate() {
        let ram = &mut rams[index + 1];
        write_i16(ram, loader + 2, -1);
        write_i16(ram, loader + 16, -1);
        write_descriptor(ram, QUEST_SOUND_LOAD_DESTINATION_RUNTIME);
        let slot = QUEST_SOUND_APPLICATION_SLOT_RUNTIME;
        write_u16(ram, slot, 1);
        write_i16(ram, slot + 2, SEQUENCE_HANDLE);
        write_bytes(ram, slot + 6, &[127, 127]);
        write_u32(ram, slot + 12, QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME);
        write_u16(ram, slot + 16, REQUEST_ID);
        write_bytes(ram, slot + 18, &[PLAYING_STATUS, 127]);
        write_u32(ram, slot + 28, 0xAA);
        write_u32(
            ram,
            QUEST_SOUND_SEQUENCE_POINTER_TABLE_RUNTIME,
            QUEST_SOUND_SEQUENCE_STATE_RUNTIME,
        );
        write_u32(ram, QUEST_SOUND_SEQUENCE_STATE_RUNTIME, cursor);
        write_bytes(ram, QUEST_SOUND_LOAD_DESTINATION_RUNTIME, &sequence);
        write_bytes(ram, QUEST_SOUND_STAGING_RUNTIME, &sequence);
    }
    (sequence, rams)
}

#[test]
fn exact_sequence_slot과_event_cursor_진행을_고정한다() {
    let offsets = [0x600_usize, 0x900, 0x1200];
    let event_cursors =
        offsets.map(|offset| QUEST_SOUND_STAGING_RUNTIME + u32::try_from(offset).unwrap());
    let (sequence, rams) = fixture(event_cursors);
    let (copies, cursors) = verify_runtime_memory(
        &sequence,
        QuestSoundSequenceRuntimeEvidence {
            first: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[0] },
            second: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[1] },
            third: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[2] },
            restored: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[3] },
        },
    )
    .unwrap();
    assert_eq!(copies.len(), 6);
    assert_eq!(
        cursors
            .iter()
            .map(|cursor| cursor.sequence_offset)
            .collect::<Vec<_>>(),
        offsets.to_vec()
    );
}

#[test]
fn staging_밖의_event_cursor를_거부한다() {
    let (sequence, mut rams) = fixture([
        QUEST_SOUND_STAGING_RUNTIME + 0x600,
        QUEST_SOUND_STAGING_RUNTIME + 0x900,
        QUEST_SOUND_STAGING_RUNTIME + 0x1200,
    ]);
    write_u32(
        &mut rams[2],
        QUEST_SOUND_SEQUENCE_STATE_RUNTIME,
        QUEST_SOUND_STAGING_RUNTIME - 4,
    );
    assert!(verify_runtime_memory(
        &sequence,
        QuestSoundSequenceRuntimeEvidence {
            first: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[0] },
            second: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[1] },
            third: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[2] },
            restored: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[3] },
        },
    )
    .is_err());
}

#[test]
fn staging_안에서_역행하는_event_cursor를_거부한다() {
    let (sequence, rams) = fixture([
        QUEST_SOUND_STAGING_RUNTIME + 0x900,
        QUEST_SOUND_STAGING_RUNTIME + 0x600,
        QUEST_SOUND_STAGING_RUNTIME + 0x1200,
    ]);
    assert!(verify_runtime_memory(
        &sequence,
        QuestSoundSequenceRuntimeEvidence {
            first: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[0] },
            second: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[1] },
            third: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[2] },
            restored: QuestSoundSequenceRuntimeCheckpoint { ram: &rams[3] },
        },
    )
    .is_err());
}

#[test]
fn report는_consumer_start_progress와_release를_분리한다() {
    let report = QuestSoundSequenceRuntimeReport {
        schema_version: QUEST_SOUND_SEQUENCE_RUNTIME_SCHEMA_VERSION,
        image_sha256: "image".to_string(),
        sequence_sha256: "sequence".to_string(),
        ram_sha256: vec![
            "first".into(),
            "second".into(),
            "third".into(),
            "restored".into(),
        ],
        copies: Vec::new(),
        cursors: Vec::new(),
        request_id: 31,
        application_slot: 0,
        sequence_handle: 0,
        playing_status: 2,
        sequence_consumer_verified: true,
        sequence_start_verified: true,
        sequence_progress_verified: true,
        bgm_track_identity_verified: true,
        runtime_verified: true,
        release_eligible: false,
    };
    let rendered = render_quest_sound_sequence_runtime_tsv(&report);
    assert!(rendered.contains("sequence_progress_verified=true"));
    assert!(rendered.contains("bgm_track_identity_verified=true"));
    assert!(rendered.contains("release_eligible=false"));
    assert!(rendered.ends_with("\timage\n"));
}
