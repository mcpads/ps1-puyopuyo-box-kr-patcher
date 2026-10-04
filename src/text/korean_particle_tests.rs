use super::*;

#[test]
fn 완성형_끝음절의_받침으로_주격조사를_고른다() {
    assert_eq!(select_subject_particle('자').unwrap(), '가');
    assert_eq!(select_subject_particle('클').unwrap(), '이');
}

#[test]
fn 영문과_숫자는_발음_끝음절로_추측하지_않는다() {
    assert!(select_subject_particle('T').is_err());
    assert!(select_subject_particle('４').is_err());
}

#[test]
fn 목적격_조사는_책과_카레의_받침을_구분하고_비한글은_추정하지_않는다() {
    assert_eq!(select_object_particle('책').unwrap(), '을');
    assert_eq!(select_object_particle('레').unwrap(), '를');
    assert_eq!(select_object_particle('링').unwrap(), '을');
    assert!(select_object_particle('T').is_err());
}
