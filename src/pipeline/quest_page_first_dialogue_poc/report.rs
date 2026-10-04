//! QUEST resident page PoC의 build·runtime 결과를 안정된 TSV로 직렬화한다.

use super::{
    QuestPageFirstDialogueReport, QuestPageFirstDialogueRuntimeReport, QuestPageTransitionReport,
    QuestPageTransitionRuntimeReport, QuestPageTwoDialoguesReport,
};

/// report를 결정적 한 행 TSV로 만든다.
pub fn render_quest_page_first_dialogue_tsv(report: &QuestPageFirstDialogueReport) -> String {
    format!(
        "schema_version\ttarget_record_runtime\tsite_runtime\tbody_runtime\tbody_bytes\t\
         body_slack_bytes\tpage_source_runtime\tpage_bytes\t\
         relocated_quest_lba\trelocated_quest_bytes\trelocated_quest_sectors\t\
         relocated_32m_lba\trelocated_32m_bytes\ttown00_cnx_bytes\tmachine_code_writes\t\
         data_writes\tmetadata_writes\trelocation_copy_writes\ttouched_sectors\t\
         final_expected_writes\toriginal_text_except_hook_exact\tresident_page_exact\t\
         extension_outside_page_zero\toriginal_font_archive_unchanged\t\
         original_quest_extent_unchanged\textent_overlaps\truntime_verified\t\
         release_eligible\timage_sha256\n\
         {}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.target_record_runtime,
        report.site_runtime,
        report.body_runtime,
        report.body_bytes,
        report.body_slack_bytes,
        report.page_source_runtime,
        report.page_bytes,
        report.relocated_quest_lba,
        report.relocated_quest_bytes,
        report.relocated_quest_sectors,
        report.relocated_32m_lba,
        report.relocated_32m_bytes,
        report.town00_cnx_bytes,
        report.machine_code_writes,
        report.data_writes,
        report.metadata_writes,
        report.relocation_copy_writes,
        report.touched_sectors,
        report.final_expected_writes,
        report.original_text_except_hook_exact,
        report.resident_page_exact,
        report.extension_outside_page_zero,
        report.original_font_archive_unchanged,
        report.original_quest_extent_unchanged,
        report.extent_overlaps,
        report.runtime_verified,
        report.release_eligible,
        report.image_sha256,
    )
}

/// exact runtime 검증 결과를 결정적 한 행 TSV로 만든다.
pub fn render_quest_page_first_dialogue_runtime_tsv(
    report: &QuestPageFirstDialogueRuntimeReport,
) -> String {
    format!(
        "schema_version\timage_sha256\tram_sha256\tgpu_sha256\ttarget_record_runtime\t\
         page_source_runtime\tpage_bytes\tpage_sha256\tuploaded_page_sha256\t\
         hook_site_exact\thook_body_exact\tresident_page_exact\tuploaded_page_exact\t\
         font_states_inactive\tpage_state\tpage_state_committed\truntime_verified\t\
         release_eligible\n\
         {}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.image_sha256,
        report.ram_sha256,
        report.gpu_sha256,
        report.target_record_runtime,
        report.page_source_runtime,
        report.page_bytes,
        report.page_sha256,
        report.uploaded_page_sha256,
        report.hook_site_exact,
        report.hook_body_exact,
        report.resident_page_exact,
        report.uploaded_page_exact,
        report.font_states_inactive,
        report.page_state,
        report.page_state_committed,
        report.runtime_verified,
        report.release_eligible,
    )
}

/// 두 page 전환 빌드 결과를 결정적 한 행 TSV로 만든다.
pub fn render_quest_page_transition_tsv(report: &QuestPageTransitionReport) -> String {
    format!(
        "schema_version\tfirst_record_runtime\tsite_runtime\t\
         body_runtime\tbody_bytes\tbody_slack_bytes\tpage_count\t\
         korean_page_source_runtime\toriginal_page_source_runtime\tpage_bytes\t\
         relocated_quest_lba\trelocated_quest_bytes\trelocated_quest_sectors\t\
         relocated_32m_lba\trelocated_32m_bytes\ttown00_cnx_bytes\tmachine_code_writes\t\
         data_writes\tmetadata_writes\trelocation_copy_writes\ttouched_sectors\t\
         final_expected_writes\toriginal_text_except_hook_exact\tkorean_page_exact\t\
         original_page_exact\tpages_differ\textension_outside_pages_zero\t\
         original_font_archive_unchanged\toriginal_quest_extent_unchanged\t\
         extent_overlaps\truntime_verified\trelease_eligible\timage_sha256\n\
         {}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t\
         0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
         {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.first_record_runtime,
        report.site_runtime,
        report.body_runtime,
        report.body_bytes,
        report.body_slack_bytes,
        report.page_count,
        report.korean_page_source_runtime,
        report.original_page_source_runtime,
        report.page_bytes,
        report.relocated_quest_lba,
        report.relocated_quest_bytes,
        report.relocated_quest_sectors,
        report.relocated_32m_lba,
        report.relocated_32m_bytes,
        report.town00_cnx_bytes,
        report.machine_code_writes,
        report.data_writes,
        report.metadata_writes,
        report.relocation_copy_writes,
        report.touched_sectors,
        report.final_expected_writes,
        report.original_text_except_hook_exact,
        report.korean_page_exact,
        report.original_page_exact,
        report.pages_differ,
        report.extension_outside_pages_zero,
        report.original_font_archive_unchanged,
        report.original_quest_extent_unchanged,
        report.extent_overlaps,
        report.runtime_verified,
        report.release_eligible,
        report.image_sha256,
    )
}

/// 두 대사 공용 page 빌드 결과를 결정적 한 행 TSV로 만든다.
pub fn render_quest_page_two_dialogues_tsv(report: &QuestPageTwoDialoguesReport) -> String {
    format!(
        "schema_version\tfirst_record_runtime\tsecond_record_runtime\tbody_runtime\t\
         body_bytes\tbody_slack_bytes\tpage_count\tkorean_page_source_runtime\t\
         original_page_source_runtime\tpage_bytes\tkorean_page_sha256\t\
         second_stream_sha256\tsecond_glyph_mappings\tkorean_page_glyph_mappings\t\
         relocated_quest_lba\t\
         relocated_quest_bytes\trelocated_quest_sectors\trelocated_32m_lba\t\
         relocated_32m_bytes\ttown00_cnx_bytes\tmachine_code_writes\tdata_writes\t\
         metadata_writes\trelocation_copy_writes\ttouched_sectors\tfinal_expected_writes\t\
         original_text_except_hook_exact\tkorean_page_exact\toriginal_page_exact\t\
         pages_differ\textension_outside_pages_zero\toriginal_font_archive_unchanged\t\
         original_quest_extent_unchanged\textent_overlaps\ttranslation_approved\t\
         layout_approved\truntime_verified\trelease_eligible\timage_sha256\n\
         {}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t\
         {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
         {}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.first_record_runtime,
        report.second_record_runtime,
        report.body_runtime,
        report.body_bytes,
        report.body_slack_bytes,
        report.page_count,
        report.korean_page_source_runtime,
        report.original_page_source_runtime,
        report.page_bytes,
        report.korean_page_sha256,
        report.second_stream_sha256,
        report.second_glyph_mappings,
        report.korean_page_glyph_mappings,
        report.relocated_quest_lba,
        report.relocated_quest_bytes,
        report.relocated_quest_sectors,
        report.relocated_32m_lba,
        report.relocated_32m_bytes,
        report.town00_cnx_bytes,
        report.machine_code_writes,
        report.data_writes,
        report.metadata_writes,
        report.relocation_copy_writes,
        report.touched_sectors,
        report.final_expected_writes,
        report.original_text_except_hook_exact,
        report.korean_page_exact,
        report.original_page_exact,
        report.pages_differ,
        report.extension_outside_pages_zero,
        report.original_font_archive_unchanged,
        report.original_quest_extent_unchanged,
        report.extent_overlaps,
        report.translation_approved,
        report.layout_approved,
        report.runtime_verified,
        report.release_eligible,
        report.image_sha256,
    )
}

/// 두 page 전환 runtime 결과를 결정적 한 행 TSV로 만든다.
pub fn render_quest_page_transition_runtime_tsv(
    report: &QuestPageTransitionRuntimeReport,
) -> String {
    format!(
        "schema_version\tphase\timage_sha256\tram_sha256\tgpu_sha256\t\
         expected_page_source_runtime\tpage_bytes\t\
         korean_page_sha256\toriginal_page_sha256\tuploaded_page_sha256\t\
         hook_site_exact\thook_body_exact\tresident_pages_exact\tuploaded_page_exact\t\
         font_states_inactive\tpage_state\tpage_state_committed\truntime_verified\t\
         release_eligible\n\
         {}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.phase.label(),
        report.image_sha256,
        report.ram_sha256,
        report.gpu_sha256,
        report.expected_page_source_runtime,
        report.page_bytes,
        report.korean_page_sha256,
        report.original_page_sha256,
        report.uploaded_page_sha256,
        report.hook_site_exact,
        report.hook_body_exact,
        report.resident_pages_exact,
        report.uploaded_page_exact,
        report.font_states_inactive,
        report.page_state,
        report.page_state_committed,
        report.runtime_verified,
        report.release_eligible,
    )
}
