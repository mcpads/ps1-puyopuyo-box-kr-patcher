//! 명령행 사용법 문자열.

pub(super) fn usage() -> &'static str {
    "사용법:\n  \
     ps1-puyopuyo-box review serve btx-families [--no-open] <원본 BIN>\n  \
     ps1-puyopuyo-box review serve btx-layout-review [--no-open] <원본 BIN>\n  \
     ps1-puyopuyo-box review serve quest [--no-open] <원본 BIN>\n  \
     ps1-puyopuyo-box verify roundtrip vdd [--require-cnx-byte-identical] <VDD>...\n  \
     ps1-puyopuyo-box verify inventory btx <GRP.VDD>...\n  \
     ps1-puyopuyo-box verify source disc <BIN>\n  \
     ps1-puyopuyo-box verify runtime btx-layout <원본 BIN> <GRP ISO 경로> <BTX 이름> <GPU dump>\n  \
     ps1-puyopuyo-box verify runtime quest-exe-relocation-control <원본 BIN> <RAM dump> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-glyph-cache-control <원본 BIN> <RAM dump> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-glyph-cache-upload <원본 BIN> <RAM dump> <GPU dump> <event JSON> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-resident-text <원본 BIN> <증거 디렉터리> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-glyph-cache-transition <원본 BIN> <증거 디렉터리> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-glyph-cache-residency <예상 page ID> <원본 BIN> <Mednafen state> <화면 PNG> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-development-disc-residency [--expect-initial-scratch] <예상 page ID> <원본 BIN> <Mednafen state> <화면 PNG> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-development-carrier-scene <예상 page ID> <원본 BIN> <Mednafen state> <화면 PNG> <descriptor event JSON> <write observation JSON> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-glyph-cache-bank <stream ID> <primary page ID> <원본 BIN> <증거 디렉터리> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-vram-bank-ownership <원본 BIN> <RAM dump> <GPU dump> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-development-disc-synthetic-route <stream ID> <원본 BIN> <entry state JSON> <entry RAM> <화면 PNG> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-development-disc-max-page <stream ID> <원본 BIN> <증거 디렉터리> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-glyph-cache-audio <원본 BIN> <증거 디렉터리> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-spu-voice-source <원본 BIN> <증거 디렉터리> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-sound-sequence <원본 BIN> <증거 디렉터리> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-glyph-cache-collision <원본 BIN> <정상 entry RAM> <강제 active RAM> <yield RAM> <retry RAM> <entry event JSON> <yield event JSON> <retry event JSON> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-glyph-cache-transition-collision <원본 BIN> <정상 entry RAM> <강제 active RAM> <yield RAM> <retry RAM> <entry event JSON> <yield event JSON> <retry event JSON> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-font-active-lifetime <패치 BIN> <store 직전 RAM> <store 직후 RAM> <안정화 RAM> <producer event JSON> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-font-active-natural <패치 BIN> <checkpoint state> <store 직전 RAM> <store 직후 RAM> <안정화 RAM> <producer event JSON> <route evidence JSON> <message PNG> <choice PNG> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-save-card <패치 BIN> <빈 MCR> <저장 MCR> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-page-first-dialogue <원본 BIN> <RAM dump> <GPU dump> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-page-transition <korean|restored> <원본 BIN> <RAM dump> <GPU dump> <출력 TSV>\n  \
     ps1-puyopuyo-box verify runtime quest-page-two-dialogues <원본 BIN> <증거 디렉터리> <출력 TSV>\n  \
     ps1-puyopuyo-box build control cnx-all <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc quest-first-dialogue [--control] <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc quest-hook-noop <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc quest-exe-relocation-control <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc quest-page-first-dialogue <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc quest-page-transition <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc quest-page-two-dialogues <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc quest-glyph-cache-control <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc quest-glyph-cache-upload <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc quest-glyph-cache-transition <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build dev quest <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build dev current <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc menu06-draft <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc p2-info-page-stress <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc drac-single-glyph <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc puyo2-scene-glyph-population <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc puyo2-full-glyph-population <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc puyo2-e6-single-glyph <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build patch puyo2-e6 <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc shell-first-description [--control] <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box build poc shell-approved-descriptions [--control] <원본 BIN> <출력 디렉터리>\n  \
     ps1-puyopuyo-box report translation coverage\n  \
     ps1-puyopuyo-box report translation mm-spr <출력 TSV>\n  \
     ps1-puyopuyo-box report translation btx-catalog <출력 TSV>\n  \
     ps1-puyopuyo-box report translation btx-pending <출력 TSV>\n  \
     ps1-puyopuyo-box report translation menu06 <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation menu06-manifest <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-dialogue <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-item-text <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-battle-names <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-town00-candidates <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-seq-candidates <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-seq-summary <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-seq-rooted-runs <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-seq-token-ledger <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-seq-review <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-terms <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-batches <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-reviews <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-translation-selection <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-translation-selection-routing <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-translation-glyph-cache-plan <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-translation-glyph-cache-package <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-translation-glyph-cache-hook <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-batch-decisions <batch ID> <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-decisions <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation quest-approval-template <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation puyo2-e6 <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report translation scene-e0 <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-seq-text-documents <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime quest-seq-text-evidence <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis exe-trace <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis exe-strings <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-text-candidates <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-text-summary <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-text-roots <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-min-text <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-e6-text <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-e6-glyphs <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime subgame-seq-e6 <원본 BIN> <RAM dump> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-layouts <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-drac-text <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-drac-consumers <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-drac-glyphs <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-scene-glyphs <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-scene-messages <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-scene-relinks <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-scene-translation-plan <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-scene-glyph-patches <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-drac-relinks <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime subgame-seq-drac <원본 BIN> <RAM dump> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-descriptors <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-registrations <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-selections <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-loads <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-control-targets <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-text-bridges <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-overlay-call-arguments <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime subgame-seq-text-bridge <원본 BIN> <RAM dump> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime subgame-seq-text-bridge-calls <원본 BIN> (<RAM dump> <event JSON> <화면 PNG>)+ <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-text-consumers <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis subgame-seq-controls <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis btx-names <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis common-font-text <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis btx-pending-consumers <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis btx-pending-static-cells <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis btx-profile-candidates <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis btx-profile-candidate-members <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis btx-family-candidates <원본 BIN> <특징 TSV> <family TSV>\n  \
     ps1-puyopuyo-box report analysis btx-family-review-template <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis btx-review-layout-evidence <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime btx-pending <원본 BIN> <RAM dump> <GPU dump> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis btx-cells <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis common-font-draw-sites <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis common-font-cell-sources <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis common-font-fixed-draws <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis bios-font <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis town00-addresses <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-seq-addresses <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-seq-text-streams <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-seq-text-roots <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-seq-text-paths <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-seq-text-frontiers <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-seq-flag-branches <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-seq-resource-transitions <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-font-plan <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-source-repertoire <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-capacity <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-font-alternatives <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-page-supply <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-second-dialogue-plan <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report analysis quest-hook-space <원본 BIN> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime quest-font-boundary <원본 BIN> <Mednafen state>\n  \
     ps1-puyopuyo-box report runtime quest-font-windows <원본 BIN> <RAM dump> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime quest-hook-space <원본 BIN> <Mednafen state>\n  \
     ps1-puyopuyo-box report runtime quest-map-collision <원본 BIN> <관측 BIN> <Mednafen state>\n  \
     ps1-puyopuyo-box report runtime quest-map-route <원본 BIN> <관측 BIN> <Mednafen state> <목표 x pixel> <목표 y pixel> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime quest-item-container-contact <원본 BIN> <관측 BIN> <Mednafen state> <item container anchor x pixel> <item container anchor y pixel> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime town00-root <원본 BIN> <Mednafen state>\n  \
     ps1-puyopuyo-box report runtime common-font-draw <원본 BIN> <EXE> <descriptor runtime> <record count> <RAM dump> <event JSON> <화면 PNG> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime common-font-writer-draw <원본 BIN> <EXE> <descriptor runtime> <record count> <RAM dump> <writer event JSON> <draw event JSON> <화면 PNG> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime common-font-direct-draw <원본 BIN> <RAM dump> <writer event JSON> <post-store event JSON> <handoff event JSON> <commit event JSON> <화면 PNG> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime cell-table <RAM dump> <descriptor runtime> <record count> <출력 TSV>\n  \
     ps1-puyopuyo-box report runtime cell-selection <RAM dump> <descriptor runtime> <record count> <index CSV> <출력 TSV>"
}
