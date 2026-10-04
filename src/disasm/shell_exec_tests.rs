use super::*;
use crate::coords::exe_map;

fn path_and_jump_fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("SLPS_031.14").unwrap();
    let mut text = vec![0u8; map.text_len as usize];
    for spec in SHELL_EXEC_PATHS {
        let offset = map
            .text_offset(PATH_TABLE_RUNTIME + u32::from(spec.path_offset))
            .unwrap();
        text[offset..offset + spec.path.len()].copy_from_slice(spec.path.as_bytes());
    }
    let jump_offset = map.text_offset(JUMP_TABLE_RUNTIME).unwrap();
    for index in 0..=usize::from(SELECTOR_LAST - SELECTOR_FIRST) {
        let selector = SELECTOR_FIRST + index as u8;
        let target = SHELL_EXEC_PATHS
            .iter()
            .find(|spec| spec.selector == selector)
            .map_or(DEFAULT_BLOCK, |spec| spec.block);
        text[jump_offset + index * 4..jump_offset + index * 4 + 4]
            .copy_from_slice(&target.to_le_bytes());
    }
    (map, text)
}

#[test]
fn selector_10부터_80까지_열_경로와_61개_default를_고정한다() {
    let (map, text) = path_and_jump_fixture();
    verify_paths(map, &text).unwrap();
    assert_eq!(verify_jump_table(map, &text).unwrap(), 61);
}

#[test]
fn quest_path와_jump_target_drift를_각각_거부한다() {
    let (map, mut text) = path_and_jump_fixture();
    let quest = SHELL_EXEC_PATHS
        .iter()
        .find(|spec| spec.selector == 70)
        .unwrap();
    let path_offset = map
        .text_offset(PATH_TABLE_RUNTIME + u32::from(quest.path_offset))
        .unwrap();
    text[path_offset] ^= 1;
    assert!(matches!(
        verify_paths(map, &text),
        Err(ShellExecError::PathMismatch { selector: 70, .. })
    ));

    let (map, mut text) = path_and_jump_fixture();
    let jump_offset = map.text_offset(JUMP_TABLE_RUNTIME).unwrap();
    let quest_index = usize::from(70 - SELECTOR_FIRST);
    text[jump_offset + quest_index * 4] ^= 1;
    assert!(matches!(
        verify_jump_table(map, &text),
        Err(ShellExecError::JumpTableMismatch { selector: 70, .. })
    ));
}
