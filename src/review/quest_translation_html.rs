//! QUEST 검토 view model과 원문 표시 구조를 local browser용 HTML로 직렬화한다.
//!
//! 판정은 만들지 않고 Rust가 계산한 상태와 입력 form만 표시한다. 승인 요청은
//! 반드시 server의 domain 저장 경로로 돌아가며 browser 저장소를 쓰지 않는다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use super::{QuestManualComposition, QuestReviewCatalog, QuestReviewQueue, QuestSourceLayout};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestReviewHtmlError(String);

impl fmt::Display for QuestReviewHtmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestReviewHtmlError {}

fn error(message: impl Into<String>) -> QuestReviewHtmlError {
    QuestReviewHtmlError(message.into())
}

fn escape_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
    }
    output
}

fn encode_query_component(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            write!(output, "%{byte:02X}").expect("String write는 실패하지 않음");
        }
    }
    output
}

fn render_source_layout(output: &mut String, layout: &QuestSourceLayout) {
    writeln!(
        output,
        "<section class=source-layout><h3>원문 표시 구조</h3><div class=badges><span>원문 glyph {}자</span><span>control {}개</span><span>window 미확정 span {}개</span><span>line 미확정 span {}개</span></div><p><code>{}</code></p><table><thead><tr><th>순서</th><th>window</th><th>입력 단계</th><th>line</th><th>보호 원문</th><th>뒤 control</th></tr></thead><tbody>",
        layout.glyphs,
        layout.controls,
        layout.unresolved_window_spans,
        layout.unresolved_line_spans,
        escape_html(&layout.source_stream_sha256),
    )
    .expect("String write는 실패하지 않음");
    for span in &layout.spans {
        let window = span
            .window_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "미확정".to_owned());
        let line = span
            .line_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "미확정".to_owned());
        let source = if span.protected_source.is_empty() {
            "(glyph 없음)"
        } else {
            &span.protected_source
        };
        writeln!(
            output,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=source-cell>{}</td><td><code>token {} · FF{:02X} · {:?} · {}</code></td></tr>",
            span.sequence,
            escape_html(&window),
            span.input_phase,
            escape_html(&line),
            escape_html(source),
            span.following_control.token_index,
            span.following_control.code,
            span.following_control.meaning,
            escape_html(&span.following_control.raw_hex),
        )
        .expect("String write는 실패하지 않음");
    }
    output.push_str("</tbody></table><p class=warning>이 표는 원문의 사실만 보여줍니다. FF0F는 input wait 경계이며 visual page clear를 뜻하지 않습니다. 번역 후보는 아직 어느 window·line에도 배치되지 않았으며, 명시적 사람 조판 없이는 정적 또는 실행 미리보기를 만들지 않습니다.</p></section>");
}

fn render_manual_composition(
    output: &mut String,
    composition: &QuestManualComposition,
    selection_target: &str,
) {
    writeln!(
        output,
        "<section class=manual-composition><h3>사람이 작성한 수정안</h3><div class=badges><span>{}</span><span>geometry {}</span><span>layout 승인 {}</span></div><p><strong>선택 문장</strong><br>{}</p><p>{}</p><p><code>{}</code></p>",
        escape_html(composition.status.label()),
        if composition.geometry_verified {
            "검증됨"
        } else {
            "미확정"
        },
        if composition.layout_approved {
            "완료"
        } else {
            "미완"
        },
        escape_html(&composition.selected_korean),
        escape_html(&composition.notes),
        escape_html(&composition.selected_text_sha256),
    )
    .expect("String write는 실패하지 않음");

    let mut cursor = 0usize;
    while cursor < composition.segments.len() {
        let first = &composition.segments[cursor];
        let mut line_text = String::new();
        let mut anchors = Vec::new();
        while cursor < composition.segments.len()
            && composition.segments[cursor].input_phase == first.input_phase
            && composition.segments[cursor].phase_line == first.phase_line
        {
            let segment = &composition.segments[cursor];
            line_text.push_str(&segment.text);
            anchors.push(format!(
                "token {} FF{:02X}",
                segment.following_control_token_index, segment.following_control_code
            ));
            cursor += 1;
        }
        writeln!(
            output,
            "<div class=manual-line><strong>input phase {} · phase line {}</strong><pre>{}</pre><small>{}</small></div>",
            first.input_phase,
            first.phase_line,
            escape_html(&line_text),
            escape_html(&anchors.join(" / ")),
        )
        .expect("String write는 실패하지 않음");
    }
    writeln!(
        output,
        "<p class=warning>입력 단계 안의 줄 번호는 화면의 행 번호가 아닙니다. FF0F는 입력만 기다리며 화면을 지우거나 cursor를 초기화하지 않습니다. 앞선 행이 남는지는 창 생성·줄 이동 등 전후 control과 실제 화면을 함께 확인해야 합니다. 이 수동안은 실제 창 상태와 결속하기 전까지 표시·layout 승인이 아닙니다.</p><button type=button class=select-candidate data-target=\"{}\" data-value=\"{}\">이 수동 수정안 선택</button></section>",
        escape_html(selection_target),
        escape_html(&composition.selected_korean),
    )
    .expect("String write는 실패하지 않음");
}

fn page_start(output: &mut String, title: &str) {
    write!(
        output,
        "<!doctype html><html lang=ko><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>{}</title><style>
        :root{{color-scheme:dark;--bg:#11141a;--panel:#1a1f29;--line:#343d4d;--accent:#7fc8ff;--ok:#99e2b4;--warn:#ffd166}}
        *{{box-sizing:border-box}}body{{margin:0;background:var(--bg);color:#edf2f7;font:15px/1.5 system-ui,sans-serif}}a{{color:var(--accent)}}
        header{{position:sticky;top:0;z-index:3;padding:12px 18px;background:#0c0f14;border-bottom:1px solid var(--line)}}
        .layout{{display:grid;grid-template-columns:260px minmax(520px,1fr) 330px;gap:12px;padding:12px}}nav,.item,.actions,.dependency{{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:14px}}
        nav{{position:sticky;top:78px;max-height:calc(100vh - 90px);overflow:auto}}nav a{{display:block;padding:5px 0}}nav a.current-batch{{color:#fff;background:#253247;border-left:3px solid var(--accent);padding-left:7px}}main{{display:grid;gap:14px}}aside{{display:grid;align-content:start;gap:12px}}
        h1,h2,h3,h4{{line-height:1.25}}h1{{font-size:20px;margin:0}}h2{{font-size:19px}}h3{{font-size:16px;margin-top:22px}}
        .source,.candidate,pre,textarea{{white-space:pre-wrap;overflow-wrap:anywhere}}.source{{padding:10px;background:#2a1d25;border-left:4px solid #db7f9f}}
        .candidate{{padding:10px;background:#142838;border-left:4px solid var(--accent);margin:8px 0}}.manual-composition{{margin-top:12px;padding:10px;background:#18251f;border-left:4px solid var(--ok)}}.manual-line{{margin:8px 0;padding:8px;background:#101914;border:1px solid #345141;border-radius:6px}}.manual-line pre{{color:#d7f3df;font-size:16px}}.badges{{display:flex;flex-wrap:wrap;gap:6px;margin:6px 0}}
        .badges span{{border:1px solid var(--line);border-radius:999px;padding:2px 8px;font-size:12px}}
        table{{width:100%;border-collapse:collapse;margin:10px 0}}th,td{{border:1px solid var(--line);padding:6px;text-align:left;vertical-align:top}}.source-cell{{white-space:pre-wrap}}.source-layout{{overflow:auto}}
        pre{{margin:7px 0 0;color:#bbc7d6}}textarea{{width:100%;min-height:95px;background:#0d1117;color:#fff;border:1px solid #596579;border-radius:6px;padding:8px}}
        button{{border:1px solid #637086;background:#253247;color:#fff;border-radius:6px;padding:7px 10px;margin:3px;cursor:pointer}}button.primary{{background:#155a87;border-color:#4ea8de}}.notice{{color:var(--ok)}}.warning{{color:var(--warn)}}
        @media(max-width:1100px){{.layout{{grid-template-columns:220px 1fr}}aside{{grid-column:2}}}}@media(max-width:760px){{.layout{{display:block}}nav{{position:static;max-height:none;margin-bottom:12px}}}}
        </style></head><body><header><h1>{}</h1></header>",
        escape_html(title),
        escape_html(title),
    )
    .expect("String write는 실패하지 않음");
}

/// 전역 QUEST 색인에서 고른 한 batch의 3-column local UI를 만든다.
pub fn render_quest_review_page(
    catalog: &QuestReviewCatalog,
    queue: &QuestReviewQueue,
    source_layouts: &[QuestSourceLayout],
    manual_compositions: &[QuestManualComposition],
    session_token: &str,
    notice: Option<&str>,
) -> Result<String, QuestReviewHtmlError> {
    if queue.entries.len() != source_layouts.len()
        || queue
            .entries
            .iter()
            .zip(source_layouts)
            .any(|(entry, layout)| entry.stream_id != layout.stream_id)
    {
        return Err(error("QUEST 검토 대기열과 원문 표시 구조 분모 불일치"));
    }
    let matching_batches = catalog
        .batches
        .iter()
        .filter(|batch| batch.batch_id == queue.batch_id)
        .collect::<Vec<_>>();
    let [current_batch] = matching_batches.as_slice() else {
        return Err(error("QUEST 검토 색인과 현재 batch 분모 불일치"));
    };
    if current_batch.entries != queue.entries.len()
        || current_batch.approved_entries != queue.approved_entries
        || current_batch.approval_build_eligible_entries != queue.approval_build_eligible_entries
    {
        return Err(error("QUEST 검토 색인과 현재 batch 집계 불일치"));
    }
    let mut manual_by_stream = BTreeMap::new();
    for composition in manual_compositions {
        if manual_by_stream
            .insert(composition.stream_id.as_str(), composition)
            .is_some()
        {
            return Err(error(format!(
                "QUEST 수동 composition stream이 중복됨: {}",
                composition.stream_id
            )));
        }
    }
    let current_compositions = queue
        .entries
        .iter()
        .filter_map(|entry| manual_by_stream.get(entry.stream_id.as_str()).copied())
        .collect::<Vec<_>>();
    let geometry_verified = current_compositions
        .iter()
        .filter(|composition| composition.geometry_verified)
        .count();
    let layout_approved = current_compositions
        .iter()
        .filter(|composition| composition.layout_approved)
        .count();
    let product_eligible = current_compositions
        .iter()
        .filter(|composition| {
            composition.geometry_verified
                && composition.layout_approved
                && queue.entries.iter().any(|entry| {
                    entry.stream_id == composition.stream_id && entry.approval_build_eligible
                })
        })
        .count();
    let mut output = String::new();
    page_start(&mut output, "QUEST 번역 승인 검토");
    output.push_str("<div class=layout><nav><strong>전체 batch</strong>");
    writeln!(
        output,
        "<p>총 {} batch · {}문장<br>문장 승인 {} · 제품 적격 {}</p>",
        catalog.batches.len(),
        catalog.entries,
        catalog.approved_entries,
        catalog.approval_build_eligible_entries,
    )
    .expect("String write는 실패하지 않음");
    for batch in &catalog.batches {
        writeln!(
            output,
            "<a{} href=\"/?batch={}\">{}<br><small>{} · {}/{} 승인</small></a>",
            if batch.batch_id == queue.batch_id {
                " class=current-batch"
            } else {
                ""
            },
            encode_query_component(&batch.batch_id),
            escape_html(&batch.scene_title),
            escape_html(&batch.batch_id),
            batch.approved_entries,
            batch.entries,
        )
        .expect("String write는 실패하지 않음");
    }
    output.push_str("<hr><strong>현재 장면 순서</strong>");
    for (index, entry) in queue.entries.iter().enumerate() {
        writeln!(
            output,
            "<a href=\"#entry-{index}\">{}. {} · {}</a>",
            index + 1,
            escape_html(&entry.speaker),
            if entry.selected_korean.is_some() {
                "승인됨"
            } else {
                "미승인"
            },
        )
        .expect("String write는 실패하지 않음");
    }
    output.push_str("</nav><main>");
    writeln!(
        output,
        "<section class=item><h2>{}</h2><div class=badges><span>전체 {}개</span><span>문장 승인 {}개</span><span>수동 작성 {}개</span><span>geometry 검증 {}개</span><span>layout 승인 {}개</span><span>제품 build-eligible {}개</span><span>공통 의존성 {}개</span></div>{}</section>",
        escape_html(&queue.batch_id),
        queue.entries.len(),
        queue.approved_entries,
        current_compositions.len(),
        geometry_verified,
        layout_approved,
        product_eligible,
        queue.dependencies.len(),
        notice
            .map(|notice| format!("<p class=notice>{}</p>", escape_html(notice)))
            .unwrap_or_default(),
    )
    .expect("String write는 실패하지 않음");

    for (index, (entry, source_layout)) in queue.entries.iter().zip(source_layouts).enumerate() {
        writeln!(
            output,
            "<article class=item id=\"entry-{index}\"><h2>{}. {}</h2><div class=badges><span>{}</span><span>{} → {}</span><span>{}</span><span>{}</span></div><p><strong>장면 문맥:</strong> {}</p><p><strong>상세 문맥:</strong> {}</p><p><strong>runtime evidence:</strong> {}</p><p><code>{}</code></p><p><strong>기준선:</strong> <code>{}</code></p><div class=source><strong>보호 원문</strong><br>{}</div>",
            index + 1,
            escape_html(&entry.scene_title),
            escape_html(&entry.independent_review_status),
            escape_html(&entry.speaker),
            escape_html(&entry.addressee),
            escape_html(&entry.branch),
            escape_html(&entry.review_action),
            escape_html(&entry.sequence_context),
            escape_html(&entry.context_note),
            escape_html(entry.runtime_evidence.as_deref().unwrap_or("관측 증거 없음")),
            escape_html(&entry.stream_id),
            escape_html(&entry.baseline_sha256),
            escape_html(&entry.protected_source),
        )
        .expect("String write는 실패하지 않음");
        render_source_layout(&mut output, source_layout);
        output.push_str("<h3>번역 후보</h3><p class=warning>아래 문장은 아직 조판되지 않았습니다. 문장 선택은 window·input phase·phase line 배치 승인과 별개입니다.</p>");
        for candidate in &entry.candidates {
            let sources = candidate
                .sources
                .iter()
                .map(|source| source.label())
                .collect::<Vec<_>>()
                .join(" + ");
            writeln!(
                output,
                "<div class=candidate><strong>{}</strong><br>{}<br><button type=button class=select-candidate data-target=\"selection-{index}\" data-value=\"{}\">이 후보 선택</button></div>",
                escape_html(&sources),
                escape_html(&candidate.korean),
                escape_html(&candidate.korean),
            )
                .expect("String write는 실패하지 않음");
        }
        if let Some(manual_composition) = manual_by_stream.get(entry.stream_id.as_str()) {
            render_manual_composition(
                &mut output,
                manual_composition,
                &format!("selection-{index}"),
            );
        } else {
            output.push_str("<section class=manual-composition><h3>사람이 작성한 수정안</h3><p class=warning>이 항목에는 아직 사람이 직접 작성한 문장·input phase·phase line 조판이 없습니다. 후보 문장을 자동 배치하거나 선택하지 않습니다.</p></section>");
        }
        writeln!(
            output,
            "<h3>근거</h3><p>{}</p><p>{}</p><p>control: <code>{}</code></p>",
            escape_html(&entry.work_notes),
            escape_html(&entry.review_notes),
            escape_html(&entry.control_signature),
        )
        .expect("String write는 실패하지 않음");
        if !entry.terms.is_empty() {
            output.push_str("<details><summary>용어 근거</summary><ul>");
            for term in &entry.terms {
                writeln!(
                    output,
                    "<li>{}: {} → {} ({})</li>",
                    escape_html(&term.id),
                    escape_html(&term.protected_source),
                    escape_html(term.approved_korean.as_deref().unwrap_or("미승인")),
                    escape_html(&term.decision),
                )
                .expect("String write는 실패하지 않음");
                if !term.evidence.is_empty() {
                    output.push_str("<ul>");
                    for evidence in &term.evidence {
                        writeln!(output, "<li>{}</li>", escape_html(evidence))
                            .expect("String write는 실패하지 않음");
                    }
                    output.push_str("</ul>");
                }
            }
            output.push_str("</ul></details>");
        }
        if !entry.dependency_ids.is_empty() {
            writeln!(
                output,
                "<details><summary>이 문장의 공통 의존성</summary><pre>{}</pre></details>",
                escape_html(&entry.dependency_ids.join("\n")),
            )
            .expect("String write는 실패하지 않음");
        }
        output.push_str("</article>");
    }
    output.push_str("</main><aside>");
    for dependency in &queue.dependencies {
        writeln!(
            output,
            "<section class=dependency><h3>공통 의존성</h3><p><code>{}</code></p><p>영향 {}회 / batch {}개 / stream {}개</p><details><summary>영향 batch</summary><pre>{}</pre></details><details><summary>영향 stream</summary><pre>{}</pre></details><form method=post action=/approve class=review-form><input type=hidden name=token value=\"{}\"><input type=hidden name=batch_id value=\"{}\"><input type=hidden name=expected_manifest_sha256 value=\"{}\"><input type=hidden name=row_kind value=shared-dependency-decision><input type=hidden name=decision_id value=\"{}\"><input type=hidden name=baseline_sha256 value=\"{}\"><label>사람 결정<textarea name=selected_ko required>{}</textarea></label><label>승인 근거<textarea name=notes required>{}</textarea></label><button class=primary type=submit>의존성 저장</button></form></section>",
            escape_html(&dependency.id),
            dependency.occurrences,
            dependency.batch_ids.len(),
            dependency.stream_ids.len(),
            escape_html(&dependency.batch_ids.join("\n")),
            escape_html(&dependency.stream_ids.join("\n")),
            escape_html(session_token),
            escape_html(&queue.batch_id),
            escape_html(&queue.approval_manifest_sha256),
            escape_html(&dependency.id),
            escape_html(&dependency.baseline_sha256),
            escape_html(dependency.selected_value.as_deref().unwrap_or("")),
            escape_html(dependency.approval_notes.as_deref().unwrap_or("")),
        )
        .expect("String write는 실패하지 않음");
    }
    for (index, entry) in queue.entries.iter().enumerate() {
        let default_selection = entry.selected_korean.as_deref().unwrap_or("");
        writeln!(
            output,
            "<section class=actions><h3>{}. {} 선택</h3><p class=warning>{} / 문장 저장은 조판·표시 승인이 아님</p><form method=post action=/approve class=review-form><input type=hidden name=token value=\"{}\"><input type=hidden name=batch_id value=\"{}\"><input type=hidden name=expected_manifest_sha256 value=\"{}\"><input type=hidden name=row_kind value=translation-entry-decision><input type=hidden name=decision_id value=\"{}\"><input type=hidden name=baseline_sha256 value=\"{}\"><label>선택/직접 수정<textarea id=\"selection-{index}\" name=selected_ko required>{}</textarea></label><label>승인 근거<textarea name=notes required>{}</textarea></label><button class=primary type=submit>문장 저장</button><button type=button class=hold data-target=\"selection-{index}\">보류</button></form></section>",
            index + 1,
            escape_html(&entry.speaker),
            escape_html(&entry.waiting_reasons.join(" / ")),
            escape_html(session_token),
            escape_html(&queue.batch_id),
            escape_html(&queue.approval_manifest_sha256),
            escape_html(&entry.stream_id),
            escape_html(&entry.baseline_sha256),
            escape_html(default_selection),
            escape_html(entry.approval_notes.as_deref().unwrap_or("")),
        )
        .expect("String write는 실패하지 않음");
    }
    writeln!(
        output,
        "<form method=post action=/stop><input type=hidden name=token value=\"{}\"><button type=submit>검토 server 종료</button></form></aside></div><script>
        let dirty=false;document.querySelectorAll('.review-form').forEach(f=>{{f.addEventListener('input',()=>dirty=true);f.addEventListener('submit',()=>dirty=false)}});
        document.querySelectorAll('.select-candidate').forEach(b=>b.addEventListener('click',()=>{{document.getElementById(b.dataset.target).value=b.dataset.value;dirty=true}}));
        document.querySelectorAll('.hold').forEach(b=>b.addEventListener('click',()=>{{document.getElementById(b.dataset.target).value='';dirty=true}}));
        addEventListener('beforeunload',e=>{{if(dirty){{e.preventDefault();e.returnValue=''}}}});
        </script></body></html>",
        escape_html(session_token),
    )
    .expect("String write는 실패하지 않음");
    Ok(output)
}

#[cfg(test)]
#[path = "quest_translation_html_tests.rs"]
mod tests;
