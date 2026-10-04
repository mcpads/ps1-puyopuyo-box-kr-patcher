use super::*;

#[test]
fn work_보호원문은_현재_rom_추출값과_같아야_한다() {
    verify_work_protected_source("stream", "원본", "원본").unwrap();
    assert!(
        verify_work_protected_source("stream", "변경된 원문", "원본")
            .unwrap_err()
            .to_string()
            .contains("현재 ROM 추출 원문과 불일치")
    );
}

#[test]
fn 같은_stream의_중복배정과_scene_order_gap은_거부한다() {
    let header = format!("{MANIFEST_HEADER}\n");
    let row = "batch\tscene\ttitle\tcontext\tevidence\t1\tvictory\tSTREAM\ttranslate\t\
               speaker\t화자\taddressee\t상대\tconfirmed\tnote\n";
    let duplicate = format!("{header}{row}{row}");
    assert!(parse_manifest(&duplicate)
        .unwrap_err()
        .to_string()
        .contains("stream 중복 배정"));

    let gap = format!(
        "{header}batch\tscene\ttitle\tcontext\tevidence\t2\tvictory\tSTREAM\t\
         translate\tspeaker\t화자\taddressee\t상대\tconfirmed\tnote\n"
    );
    assert!(parse_manifest(&gap)
        .unwrap_err()
        .to_string()
        .contains("scene_order 비연속"));
}

#[test]
fn 문체후보의_미지원상태와_중복id는_거부한다() {
    let invalid_status = format!("{STYLE_HEADER}\nstyle\tscope\trule\tapproved\tevidence\tnotes\n");
    assert!(verify_style_candidates(&invalid_status)
        .unwrap_err()
        .to_string()
        .contains("status 미지원"));

    let duplicate = format!(
        "{STYLE_HEADER}\n\
         style\tscope\trule\tcandidate\tevidence\tnotes\n\
         style\tscope\trule\tprotected\tevidence\tnotes\n"
    );
    assert!(verify_style_candidates(&duplicate)
        .unwrap_err()
        .to_string()
        .contains("ID 중복"));
}
