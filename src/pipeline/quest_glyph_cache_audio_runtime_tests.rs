use super::*;

fn state(
    pc: u32,
    spu_control: u32,
    start_addr: u32,
    adsr_control: u32,
    adsr_level: u32,
    read_addr: u32,
) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "g0.PC": pc,
        "g2.MainVolL": 24574,
        "g2.MainVolR": 24574,
        "g2.CDVolL": 24575,
        "g2.CDVolR": 24575,
        "g2.SPUCTRL": spu_control,
        "g2.NoiseOn": 0,
        "g2.FMOn": 0,
        "g2.ReverbOn": 0,
        "g2.RvbVolL": 16512,
        "g2.RvbVolR": 16512,
        "g2.VoiceOn": 0,
        "g2.VoiceOff": 0,
        "g3.SAddr": start_addr,
        "g3.ADSRCTRL": adsr_control,
        "g3.Pitch": 4096,
        "g3.ADSRLev": adsr_level,
        "g3.VolL": 3858,
        "g3.VolR": 0,
        "g3.RAddr": read_addr,
    }))
    .expect("state JSON")
}

#[test]
fn 활성_voice는_같은_program으로_진행해야한다() {
    let entry =
        parse_state(&state(0, 49281, 163848, 3459088551, 11854, 173011)).expect("entry state");
    let returned =
        parse_state(&state(0, 49281, 163848, 3459088551, 11590, 173613)).expect("return state");
    assert!(same_voice_identity(&entry, &returned).expect("identity"));
    assert!(voice_progressed(&entry, &returned).expect("progress"));
}

#[test]
fn 활성_voice의_진행이_없으면_연속성으로_세지않는다() {
    let raw = state(0, 49281, 163848, 3459088551, 11854, 173011);
    let entry = parse_state(&raw).expect("entry state");
    let returned = parse_state(&raw).expect("return state");
    assert!(!voice_progressed(&entry, &returned).expect("no progress"));
}

#[test]
fn spu_control_변경은_checkpoint에서_거부한다() {
    let entry = state(0x8011_87D4, 49281, 163848, 3459088551, 11854, 173011);
    let returned = state(
        QUEST_BODY_RETURN_RUNTIME,
        0,
        163848,
        3459088551,
        11590,
        173613,
    );
    let spu = vec![0; PSX_SPU_RAM_BYTES];
    let result = verify_checkpoint(
        "changed-control",
        QuestGlyphCacheAudioRuntimeCheckpoint {
            entry_state: &entry,
            return_state: &returned,
            entry_spu: &spu,
            return_spu: &spu,
        },
        0x8011_87D4,
    );
    assert!(result
        .expect_err("SPUCTRL 변경은 실패")
        .to_string()
        .contains("g2.SPUCTRL"));
}

#[test]
fn spu_ram_dump의_정확한_크기를_요구한다() {
    let raw = state(0, 49281, 163848, 3459088551, 11854, 173011);
    let short_spu = vec![0; PSX_SPU_RAM_BYTES - 1];
    let result = verify_checkpoint(
        "short-spu",
        QuestGlyphCacheAudioRuntimeCheckpoint {
            entry_state: &raw,
            return_state: &raw,
            entry_spu: &short_spu,
            return_spu: &short_spu,
        },
        0,
    );
    assert!(result
        .expect_err("잘린 SPU RAM은 실패")
        .to_string()
        .contains("SPU RAM 길이 불일치"));
}

#[test]
fn page_전환에서_활성_voice만_진행을_요구한다() {
    let checkpoints = [
        VerifiedAudioCheckpoint {
            entry_state_sha256: String::new(),
            return_state_sha256: String::new(),
            entry_spu_sha256: String::new(),
            return_spu_sha256: String::new(),
            entry_state: serde_json::Value::Null,
            active_voice_at_entry: true,
            voice_progress_observed: true,
        },
        VerifiedAudioCheckpoint {
            entry_state_sha256: String::new(),
            return_state_sha256: String::new(),
            entry_spu_sha256: String::new(),
            return_spu_sha256: String::new(),
            entry_state: serde_json::Value::Null,
            active_voice_at_entry: false,
            voice_progress_observed: false,
        },
    ];
    assert!(active_voices_progressed(&checkpoints, &[0, 1]));
    assert!(!active_voices_progressed(&checkpoints, &[1]));

    let mut stalled = checkpoints;
    stalled[0].voice_progress_observed = false;
    assert!(!active_voices_progressed(&stalled, &[0, 1]));
}
