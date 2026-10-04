use super::*;
use crate::script::{QuestSeqTranslationUnit, QuestSeqTranslationUnitPolicy};

const HEADER: &str = "term_id\tcategory\tjp\tcandidate_ko\tapproved_ko\tevidence_project\t\
     evidence_path\tevidence_state\tnotes\n";

fn analysis_with_sources(sources: &[&str]) -> QuestSeqTranslationUnitAnalysis {
    QuestSeqTranslationUnitAnalysis {
        units: sources
            .iter()
            .enumerate()
            .map(|(index, source)| QuestSeqTranslationUnit {
                id: format!("TEST:{index}"),
                sequence: "TEST.SEQ".to_owned(),
                sequence_sha256: [0; 32],
                record_offset: index,
                end_offset: index + 1,
                stream_sha256: [0; 32],
                policy: QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate,
                protected_source: (*source).to_owned(),
                glyph_occurrences: source.chars().count(),
                japanese_glyph_occurrences: source.chars().count(),
                protected_raw_glyph_occurrences: 0,
                control_occurrences: 0,
                control_signature: String::new(),
                runtime_evidence: None,
            })
            .collect(),
        japanese_translation_candidates: sources.len(),
        non_japanese_reference_units: 0,
        protected_raw_review_units: 0,
        control_only_reference_units: 0,
        runtime_observed_units: 0,
    }
}

#[test]
fn 현재_원문_횟수와_자매표기_충돌을_분리한다() {
    let ledger = format!(
        "{HEADER}\
         quest.character.arle\tcharacter\tアルル\t아르르\t\tone\tterms.tsv\t\
         series-approved\t승인 표기\n\
         quest.honorific.car_kun\thonorific\tカーくん\t카 군\t\ttwo\tglossary.md\t\
         same-title-reference\t띄어쓰기\n\
         quest.honorific.car_kun\thonorific\tカーくん\t카군\t\tthree\tterms.json\t\
         project-decision\t붙여쓰기\n"
    );
    let units = analysis_with_sources(&["アルルとアルル、カーくん", "カーくん"]);
    let analysis = audit_from_ledger(&units, &ledger).unwrap();

    assert_eq!(analysis.terms.len(), 2);
    assert_eq!(analysis.evidence_rows, 3);
    assert_eq!(analysis.pending_terms, 1);
    assert_eq!(analysis.conflicting_terms, 1);
    assert_eq!(analysis.approved_terms, 0);
    assert_eq!(analysis.terms[0].source_occurrences, 2);
    assert_eq!(analysis.terms[0].matched_units, 1);
    assert_eq!(
        analysis.terms[1].decision,
        QuestTerminologyDecision::ConflictingSiblingCandidates
    );
    assert_eq!(analysis.terms[1].candidate_ko, ["카 군", "카군"]);

    let report = render_quest_terminology_candidates_tsv(&analysis);
    assert!(report.contains("quest-terminology-candidates-v1"));
    assert!(report.contains("conflict-needs-human-decision"));
    assert!(report.contains("two:glossary.md[same-title-reference]=>카 군"));
}

#[test]
fn 자매근거가_없는_현재작품_지명은_별도_후보로_보존한다() {
    let ledger = format!(
        "{HEADER}quest.location.happiness_tower\tlocation\tしあわせの塔\t행복의 탑\t\t\
         current\tassets/translations/batches.tsv\tcurrent-source-candidate\t\
         현재 작품의 반복 목적지 후보\n"
    );
    let units = analysis_with_sources(&["しあわせの塔へ行こう"]);
    let analysis = audit_from_ledger(&units, &ledger).unwrap();

    assert_eq!(analysis.terms.len(), 1);
    assert_eq!(analysis.pending_terms, 1);
    assert_eq!(
        analysis.terms[0].category,
        QuestTerminologyCategory::Location
    );
    assert_eq!(
        analysis.terms[0].evidence[0].state,
        QuestTerminologyEvidenceState::CurrentSourceCandidate
    );
}

#[test]
fn 사람승인은_후보중_하나만_허용한다() {
    let units = analysis_with_sources(&["アルル"]);
    let invalid = format!(
        "{HEADER}quest.character.arle\tcharacter\tアルル\t아르르\t알르\tone\t\
         terms.tsv\tseries-approved\t승인 표기\n"
    );
    assert!(audit_from_ledger(&units, &invalid)
        .unwrap_err()
        .to_string()
        .contains("승인 표기가 후보 집합에 없음"));

    let valid = format!(
        "{HEADER}quest.character.arle\tcharacter\tアルル\t아르르\t아르르\tone\t\
         terms.tsv\tseries-approved\t승인 표기\n"
    );
    let analysis = audit_from_ledger(&units, &valid).unwrap();
    assert_eq!(analysis.approved_terms, 1);
    assert_eq!(
        analysis.terms[0].decision,
        QuestTerminologyDecision::HumanApproved
    );
}

#[test]
fn 현재_원문에_없는_용어와_중복근거를_거부한다() {
    let units = analysis_with_sources(&["アルル"]);
    let missing = format!(
        "{HEADER}quest.character.rulue\tcharacter\tルルー\t루루\t\tone\t\
         terms.tsv\tseries-approved\t승인 표기\n"
    );
    assert!(audit_from_ledger(&units, &missing)
        .unwrap_err()
        .to_string()
        .contains("등장하지 않음"));

    let duplicate = format!(
        "{HEADER}\
         quest.character.arle\tcharacter\tアルル\t아르르\t\tone\tterms.tsv\t\
         series-approved\t승인 표기\n\
         quest.character.arle\tcharacter\tアルル\t아르르\t\tone\tterms.tsv\t\
         series-approved\t승인 표기\n"
    );
    assert!(audit_from_ledger(&units, &duplicate)
        .unwrap_err()
        .to_string()
        .contains("근거 중복"));
}

#[test]
fn stream_순서와_무관하게_report가_결정적이다() {
    let ledger = format!(
        "{HEADER}\
         quest.character.arle\tcharacter\tアルル\t아르르\t\tone\tterms.tsv\t\
         series-approved\t승인 표기\n"
    );
    let left = analysis_with_sources(&["アルル", "알파アルル"]);
    let right = analysis_with_sources(&["알파アルル", "アルル"]);
    let left = audit_from_ledger(&left, &ledger).unwrap();
    let right = audit_from_ledger(&right, &ledger).unwrap();
    assert_eq!(
        render_quest_terminology_candidates_tsv(&left),
        render_quest_terminology_candidates_tsv(&right)
    );
}
