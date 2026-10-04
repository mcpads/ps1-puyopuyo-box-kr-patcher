# 뿌요뿌요 BOX (PlayStation) 한글 패처

PlayStation용 《ぷよぷよBOX》(SLPS-03114) 일본판에 한글 패치를 적용하는 Rust 코드입니다. 디스크·ISO 9660·PSX-EXE 처리, VDD 아카이브와 CNX 압축, BTX·QMP 그래픽, 스크립트 재삽입, 한글 글꼴 합성, R3000A 코드 hook과 Expected Write 기반 디스크 빌드를 제공합니다.

배포용 패치와 적용 방법은 [뿌요뿌요 시리즈 한글 번역 프로젝트](https://github.com/mcpads/puyo-puyo-kr-patch/tree/main/ps1-puyopuyo-box)에서 제공합니다.

## 빌드

번역·글꼴·그래픽 입력(`assets/`)은 이 저장소에 포함하지 않으며, 빌드할 때 소스에 함께 컴파일됩니다. `assets/`를 저장소 루트에 둔 뒤 빌드합니다.

```bash
cargo build --release
cargo test --release
```

## 디스크 생성

```bash
cargo run --release -- build dev current <원본 BIN> <빈 출력 디렉터리>
```

출력 디렉터리에 `current-development.bin`과 `.cue`가 생깁니다. 사용 가능한 명령은 `cargo run --release -- --help`로 확인할 수 있습니다.

## 지원 원본

일본판 SLPS-03114 Mode2/2352 BIN 하나만 지원합니다.

| 항목 | 값 |
| --- | --- |
| 크기 | 141,853,824바이트 |
| SHA-1 | `ca01a98ccc5ca796df082782a6dd1adee856c064` |
| SHA-256 | `8403b1e2354107457063c5c1a8637a1ca706d6e6b19dd59c68885399256085fc` |

## 라이선스

이 저장소의 소스 코드는 [MIT License](LICENSE)로 제공합니다.
