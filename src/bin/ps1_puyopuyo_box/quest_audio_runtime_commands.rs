//! QUEST glyph cache 전환 중 audio/SPU/sequence 보존 증거를 검증하는 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::disasm::verify_box_quest_sound_sequence_consumer;
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::pipeline::{
    render_quest_glyph_cache_audio_runtime_tsv, render_quest_sound_sequence_runtime_tsv,
    render_quest_spu_voice_source_runtime_tsv, verify_box_quest_glyph_cache_transition_poc,
    verify_quest_glyph_cache_audio_runtime as verify_quest_glyph_cache_audio_runtime_state,
    verify_quest_sound_sequence_runtime as verify_quest_sound_sequence_runtime_state,
    verify_quest_spu_voice_source_runtime as verify_quest_spu_voice_source_runtime_state,
    QuestGlyphCacheAudioRuntimeCheckpoint, QuestGlyphCacheAudioRuntimeEvidence,
    QuestSoundSequenceRuntimeCheckpoint, QuestSoundSequenceRuntimeEvidence,
    QuestSpuVoiceSourceRuntimeCheckpoint, QuestSpuVoiceSourceRuntimeEvidence,
};

use super::disc_commands::read_box_executables;
use super::quest_analysis_inputs::read_quest_glyph_cache_control_inputs;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_glyph_cache_audio_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, evidence_dir, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_transition_poc(
        &source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    let read = |relative: &str| {
        let path = evidence_dir.join(relative);
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let first_entry_state = read("first-entry-dump/state.json")?;
    let first_return_state = read("first-return-dump/state.json")?;
    let first_entry_spu = read("first-entry-dump/spu.bin")?;
    let first_return_spu = read("first-return-dump/spu.bin")?;
    let second_entry_state = read("second-entry-dump/state.json")?;
    let second_return_state = read("second-return-dump/state.json")?;
    let second_entry_spu = read("second-entry-dump/spu.bin")?;
    let second_return_spu = read("second-return-dump/spu.bin")?;
    let third_entry_state = read("third-entry-dump/state.json")?;
    let third_return_state = read("third-return-dump/state.json")?;
    let third_entry_spu = read("third-entry-dump/spu.bin")?;
    let third_return_spu = read("third-return-dump/spu.bin")?;
    let restored_entry_state = read("restored-entry-dump/state.json")?;
    let restored_return_state = read("restored-return-dump/state.json")?;
    let restored_entry_spu = read("restored-entry-dump/spu.bin")?;
    let restored_return_spu = read("restored-return-dump/spu.bin")?;
    let report = verify_quest_glyph_cache_audio_runtime_state(
        &build,
        QuestGlyphCacheAudioRuntimeEvidence {
            first: QuestGlyphCacheAudioRuntimeCheckpoint {
                entry_state: &first_entry_state,
                return_state: &first_return_state,
                entry_spu: &first_entry_spu,
                return_spu: &first_return_spu,
            },
            second: QuestGlyphCacheAudioRuntimeCheckpoint {
                entry_state: &second_entry_state,
                return_state: &second_return_state,
                entry_spu: &second_entry_spu,
                return_spu: &second_return_spu,
            },
            third: QuestGlyphCacheAudioRuntimeCheckpoint {
                entry_state: &third_entry_state,
                return_state: &third_return_state,
                entry_spu: &third_entry_spu,
                return_spu: &third_return_spu,
            },
            restored: QuestGlyphCacheAudioRuntimeCheckpoint {
                entry_state: &restored_entry_state,
                return_state: &restored_return_state,
                entry_spu: &restored_entry_spu,
                return_spu: &restored_return_spu,
            },
        },
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_glyph_cache_audio_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST sparse glyph cache audio runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST sparse glyph cache audio runtime: pages {}, upload {}, active voice {}, \
         progress {}, preserved SPU fields {}, control {}, long-lived identity {}, page-change \
         progress {}, BGM track identity {}, release {}, image SHA-256 {}",
        report
            .page_states
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        report
            .load_image_calls
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        report
            .active_voice_at_entry
            .iter()
            .map(bool::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        report
            .voice_progress_observed
            .iter()
            .map(bool::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        report.preserved_spu_fields,
        report.spu_control_preserved,
        report.long_lived_voice_identity_preserved,
        report.page_change_voice_progress_exact,
        report.bgm_track_identity_verified,
        report.release_eligible,
        report.image_sha256,
    );
    Ok(())
}

pub(super) fn verify_quest_spu_voice_source_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, evidence_dir, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_transition_poc(
        &source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let sound_vdd = iso
        .read_file("/QUEST/RPG_SND.VDD;1")
        .map_err(|error| error.to_string())?;
    let read = |relative: &str| {
        let path = evidence_dir.join(relative);
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let first_state = read("first-entry-dump/state.json")?;
    let first_ram = read("first-entry-dump/ram.bin")?;
    let first_spu = read("first-entry-dump/spu.bin")?;
    let second_state = read("second-entry-dump/state.json")?;
    let second_ram = read("second-entry-dump/ram.bin")?;
    let second_spu = read("second-entry-dump/spu.bin")?;
    let third_state = read("third-entry-dump/state.json")?;
    let third_ram = read("third-entry-dump/ram.bin")?;
    let third_spu = read("third-entry-dump/spu.bin")?;
    let restored_state = read("restored-entry-dump/state.json")?;
    let restored_ram = read("restored-entry-dump/ram.bin")?;
    let restored_spu = read("restored-entry-dump/spu.bin")?;
    let report = verify_quest_spu_voice_source_runtime_state(
        &build,
        &sound_vdd,
        QuestSpuVoiceSourceRuntimeEvidence {
            first: QuestSpuVoiceSourceRuntimeCheckpoint {
                state: &first_state,
                ram: &first_ram,
                spu: &first_spu,
            },
            second: QuestSpuVoiceSourceRuntimeCheckpoint {
                state: &second_state,
                ram: &second_ram,
                spu: &second_spu,
            },
            third: QuestSpuVoiceSourceRuntimeCheckpoint {
                state: &third_state,
                ram: &third_ram,
                spu: &third_spu,
            },
            restored: QuestSpuVoiceSourceRuntimeCheckpoint {
                state: &restored_state,
                ram: &restored_ram,
                spu: &restored_spu,
            },
        },
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_spu_voice_source_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST SPU voice source runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST SPU voice source runtime: resident VAB {}, resident SEQ {}, voice sources {}, \
         long-lived source {}, _01WAKU VAB/SEQ {}/{}, sequence consumer {}, BGM identity {}, \
         release {}, image SHA-256 {}",
        report.resident_vab_bodies.len(),
        report.resident_sequences.len(),
        report.voice_sources.len(),
        report.long_lived_voice_source_exact,
        report.wakeup_vab_resident,
        report.wakeup_sequence_resident,
        report.sequence_consumer_verified,
        report.bgm_track_identity_verified,
        report.release_eligible,
        report.image_sha256,
    );
    Ok(())
}

pub(super) fn verify_quest_sound_sequence_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, evidence_dir, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_transition_poc(
        &source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|executable| executable.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let consumer = verify_box_quest_sound_sequence_consumer(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;
    let sound_vdd = iso
        .read_file("/QUEST/RPG_SND.VDD;1")
        .map_err(|error| error.to_string())?;
    let read_ram = |relative: &str| {
        let path = evidence_dir.join(relative);
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let first_ram = read_ram("first-entry-dump/ram.bin")?;
    let second_ram = read_ram("second-entry-dump/ram.bin")?;
    let third_ram = read_ram("third-entry-dump/ram.bin")?;
    let restored_ram = read_ram("restored-entry-dump/ram.bin")?;
    let report = verify_quest_sound_sequence_runtime_state(
        &build,
        &consumer,
        &sound_vdd,
        QuestSoundSequenceRuntimeEvidence {
            first: QuestSoundSequenceRuntimeCheckpoint { ram: &first_ram },
            second: QuestSoundSequenceRuntimeCheckpoint { ram: &second_ram },
            third: QuestSoundSequenceRuntimeCheckpoint { ram: &third_ram },
            restored: QuestSoundSequenceRuntimeCheckpoint { ram: &restored_ram },
        },
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_sound_sequence_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST sound sequence runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST sound sequence runtime: copies {}, cursors {}, request/slot/handle \
         {}/{}/{}, status {}, consumer {}, start {}, progress {}, BGM identity {}, release {}, \
         image SHA-256 {}",
        report.copies.len(),
        report.cursors.len(),
        report.request_id,
        report.application_slot,
        report.sequence_handle,
        report.playing_status,
        report.sequence_consumer_verified,
        report.sequence_start_verified,
        report.sequence_progress_verified,
        report.bgm_track_identity_verified,
        report.release_eligible,
        report.image_sha256,
    );
    Ok(())
}
