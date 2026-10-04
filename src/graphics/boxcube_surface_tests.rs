use super::*;

#[test]
fn 다른원본은_픽셀쓰기전에_거부한다() {
    assert!(patch_boxcube_surface(&vec![0; 128 * 192]).is_err());
}

#[test]
fn 트레이닝의_옛일본어_돌출영역을_투명하게_지운다() {
    let line = PIXELS
        .lines()
        .find(|line| line.starts_with("BTXB-0027\t"))
        .unwrap();
    let hex = line.split('\t').nth(3).unwrap();
    let pixels: Vec<_> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    // 새 그림은 첫 64px 안에 있다. 옛 とれーにんぐ의 오른쪽 돌출은 남기지 않는다.
    assert!((0..48).all(|y| pixels[y * 80 + 64..y * 80 + 80].iter().all(|&v| v == 0)));
    // 바깥 제거를 검은 글자 외곽선 전체의 투명화로 대체하지 않는다.
    assert!(pixels.contains(&1));
}
