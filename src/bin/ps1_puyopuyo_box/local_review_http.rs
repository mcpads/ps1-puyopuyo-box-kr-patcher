//! 사람이 결정을 남기는 local review server의 공통 HTTP 경계.
//!
//! loopback bind, 제한된 단일 request, form 해석과 session token만 맡는다.
//! route와 domain 저장은 각 검토 server가 소유한다.

use std::collections::BTreeMap;
use std::io::{Read, Write as _};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::process::Command;
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HttpRequest {
    pub method: String,
    pub target: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HttpResponse {
    pub status: &'static str,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub location: Option<String>,
    pub stop_server: bool,
}

impl HttpResponse {
    pub fn html(status: &'static str, body: String) -> Self {
        Self {
            status,
            content_type: "text/html; charset=utf-8",
            body: body.into_bytes(),
            location: None,
            stop_server: false,
        }
    }

    pub fn text(status: &'static str, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body: body.into().into_bytes(),
            location: None,
            stop_server: false,
        }
    }

    pub fn binary(status: &'static str, content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type,
            body,
            location: None,
            stop_server: false,
        }
    }

    pub fn redirect() -> Self {
        Self::redirect_to("/".to_owned())
    }

    pub fn redirect_to(location: String) -> Self {
        Self {
            status: "303 See Other",
            content_type: "text/plain; charset=utf-8",
            body: "저장됨; 검토 화면을 다시 불러옵니다.\n".as_bytes().to_vec(),
            location: Some(location),
            stop_server: false,
        }
    }
}

fn find_bytes(input: &[u8], needle: &[u8]) -> Option<usize> {
    input
        .windows(needle.len())
        .position(|window| window == needle)
}

pub(super) fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("HTTP read timeout 설정 실패: {error}"))?;
    let mut input = Vec::new();
    let header_end = loop {
        if let Some(index) = find_bytes(&input, b"\r\n\r\n") {
            break index + 4;
        }
        if input.len() >= MAX_HEADER_BYTES {
            return Err("HTTP header가 허용 크기를 넘음".to_owned());
        }
        let mut chunk = [0u8; 4096];
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("HTTP request 읽기 실패: {error}"))?;
        if read == 0 {
            return Err("HTTP header가 끝나기 전에 연결이 닫힘".to_owned());
        }
        input.extend_from_slice(&chunk[..read]);
    };
    if header_end > MAX_HEADER_BYTES {
        return Err("HTTP header가 허용 크기를 넘음".to_owned());
    }
    let (method, target, content_length) = {
        let header = std::str::from_utf8(&input[..header_end])
            .map_err(|_| "HTTP header가 UTF-8이 아님".to_owned())?;
        let mut lines = header[..header.len() - 4].split("\r\n");
        let request_line = lines
            .next()
            .ok_or_else(|| "HTTP request line이 없음".to_owned())?;
        let request_parts = request_line.split_ascii_whitespace().collect::<Vec<_>>();
        let [method, target, version] = request_parts.as_slice() else {
            return Err("HTTP request line 형식 오류".to_owned());
        };
        if !matches!(*version, "HTTP/1.0" | "HTTP/1.1") || !target.starts_with('/') {
            return Err("HTTP version 또는 request target 오류".to_owned());
        }

        let mut content_length = None;
        for line in lines {
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| "HTTP header 행 형식 오류".to_owned())?;
            if name.eq_ignore_ascii_case("transfer-encoding") {
                return Err("HTTP Transfer-Encoding은 지원하지 않음".to_owned());
            }
            if name.eq_ignore_ascii_case("content-length") {
                if content_length.is_some() {
                    return Err("HTTP Content-Length가 중복됨".to_owned());
                }
                content_length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| "HTTP Content-Length 형식 오류".to_owned())?,
                );
            }
        }
        (
            (*method).to_owned(),
            (*target).to_owned(),
            content_length.unwrap_or(0),
        )
    };
    if content_length > MAX_BODY_BYTES {
        return Err("HTTP body가 허용 크기를 넘음".to_owned());
    }
    let total_length = header_end
        .checked_add(content_length)
        .ok_or_else(|| "HTTP request 크기 overflow".to_owned())?;
    while input.len() < total_length {
        let mut chunk = [0u8; 4096];
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("HTTP body 읽기 실패: {error}"))?;
        if read == 0 {
            return Err("HTTP body가 끝나기 전에 연결이 닫힘".to_owned());
        }
        input.extend_from_slice(&chunk[..read]);
        if input.len() > total_length {
            return Err("한 연결에 둘 이상의 HTTP request가 들어옴".to_owned());
        }
    }
    if input.len() != total_length {
        return Err("HTTP body 크기 불일치".to_owned());
    }
    Ok(HttpRequest {
        method,
        target,
        body: input[header_end..].to_vec(),
    })
}

pub(super) fn write_http_response(
    stream: &mut TcpStream,
    response: &HttpResponse,
) -> Result<(), String> {
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("HTTP write timeout 설정 실패: {error}"))?;
    let location = response
        .location
        .as_deref()
        .map(|location| format!("Location: {location}\r\n"))
        .unwrap_or_default();
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Connection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; img-src 'self' data:; style-src 'unsafe-inline'; script-src 'unsafe-inline'; form-action 'self'\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len(),
        location,
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|()| stream.write_all(&response.body))
        .and_then(|()| stream.flush())
        .map_err(|error| format!("HTTP response 쓰기 실패: {error}"))
}

fn decode_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_form_component(input: &[u8]) -> Result<String, String> {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'+' => output.push(b' '),
            b'%' => {
                let high = input
                    .get(index + 1)
                    .and_then(|byte| decode_hex_digit(*byte))
                    .ok_or_else(|| "form percent escape 오류".to_owned())?;
                let low = input
                    .get(index + 2)
                    .and_then(|byte| decode_hex_digit(*byte))
                    .ok_or_else(|| "form percent escape 오류".to_owned())?;
                output.push((high << 4) | low);
                index += 2;
            }
            byte => output.push(byte),
        }
        index += 1;
    }
    String::from_utf8(output).map_err(|_| "form 값이 UTF-8이 아님".to_owned())
}

pub(super) fn parse_form(body: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let mut fields = BTreeMap::new();
    for pair in body.split(|byte| *byte == b'&') {
        let separator = pair
            .iter()
            .position(|byte| *byte == b'=')
            .ok_or_else(|| "form field 형식 오류".to_owned())?;
        let (name, value_with_separator) = pair.split_at(separator);
        let value = &value_with_separator[1..];
        let name = decode_form_component(name)?;
        let value = decode_form_component(value)?;
        if name.is_empty() || fields.insert(name.clone(), value).is_some() {
            return Err(format!("form field가 비었거나 중복됨: {name}"));
        }
    }
    Ok(fields)
}

pub(super) fn required_field<'a>(
    fields: &'a BTreeMap<String, String>,
    name: &str,
) -> Result<&'a str, String> {
    fields
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("form 필수값이 없음: {name}"))
}

pub(super) fn verify_session(
    fields: &BTreeMap<String, String>,
    expected: &str,
) -> Result<(), String> {
    if required_field(fields, "token")? != expected {
        return Err("검토 session token 불일치".to_owned());
    }
    Ok(())
}

pub(super) fn session_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| format!("검토 session token 생성 실패: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(super) fn bind_loopback() -> Result<TcpListener, String> {
    TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .map_err(|error| format!("검토 loopback bind 실패: {error}"))
}

pub(super) fn open_browser(context: &str, url: &str) -> Result<(), String> {
    let status = Command::new("open")
        .arg(url)
        .status()
        .map_err(|error| format!("{context} browser 열기 실패: {error}"))?;
    if !status.success() {
        return Err(format!("{context} browser 열기 명령 실패: status={status}"));
    }
    Ok(())
}
