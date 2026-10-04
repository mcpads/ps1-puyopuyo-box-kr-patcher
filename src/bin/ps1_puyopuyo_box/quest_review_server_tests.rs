use super::*;

use std::io::Write as _;
use std::net::{IpAddr, Ipv4Addr, TcpStream};
use std::thread;

#[test]
fn form은_utf8_percent_escape와_space를_복원한다() {
    let fields =
        parse_form(b"selected_ko=%EA%B0%80%EB%82%98+%EB%8B%A4&notes=ok").expect("form 해석");

    assert_eq!(
        fields.get("selected_ko").map(String::as_str),
        Some("가나 다")
    );
    assert_eq!(fields.get("notes").map(String::as_str), Some("ok"));
}

#[test]
fn form은_중복_field와_깨진_escape를_거부한다() {
    assert!(parse_form(b"token=a&token=b").is_err());
    assert!(parse_form(b"token=%Q0").is_err());
}

#[test]
fn session은_정확한_server_token만_받는다() {
    let fields = parse_form(b"token=expected").expect("form 해석");

    assert!(verify_session(&fields, "expected").is_ok());
    assert!(verify_session(&fields, "different").is_err());
}

#[test]
fn get_query는_batch_하나만_복원한다() {
    assert_eq!(requested_batch_id("/").unwrap(), None);
    assert_eq!(
        requested_batch_id("/?batch=quest-town00%2Dtest").unwrap(),
        Some("quest-town00-test".to_owned())
    );
    assert!(requested_batch_id("/?batch=a&batch=b").is_err());
    assert!(requested_batch_id("/?unknown=a").is_err());
}

#[test]
fn batch_redirect_query는_비ascii도_왕복한다() {
    let batch_id = "시험 batch";
    let target = format!("/?batch={}", encode_query_component(batch_id));

    assert_eq!(
        requested_batch_id(&target).unwrap().as_deref(),
        Some(batch_id)
    );
}

#[test]
fn 명시적조판이_없으면_옛preview_route도_실패한다() {
    let fields = parse_form(b"token=expected").expect("form 해석");

    let error = reject_unlaid_preview(&fields, "expected").expect_err("미조판 preview 거부");
    assert!(error.contains("명시적 사람 조판"));
}

#[test]
fn listener는_ipv4_loopback에만_bind한다() {
    let listener = bind_loopback().expect("loopback bind");
    let address = listener.local_addr().expect("local 주소");

    assert_eq!(address.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
}

#[test]
fn http_reader는_content_length에_맞는_한_request만_받는다() {
    let listener = bind_loopback().expect("loopback bind");
    let address = listener.local_addr().expect("local 주소");
    let writer = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).expect("test server 연결");
        stream
            .write_all(
                b"POST /approve HTTP/1.1\r\nHost: localhost\r\nContent-Length: 7\r\n\r\ntoken=x",
            )
            .expect("request 쓰기");
    });
    let (mut stream, _) = listener.accept().expect("test request 수락");

    let request = read_http_request(&mut stream).expect("HTTP request 해석");
    writer.join().expect("request writer");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/approve");
    assert_eq!(request.body, b"token=x");
}

#[test]
fn http_reader는_transfer_encoding을_거부한다() {
    let listener = bind_loopback().expect("loopback bind");
    let address = listener.local_addr().expect("local 주소");
    let writer = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).expect("test server 연결");
        stream
            .write_all(
                b"POST /approve HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n",
            )
            .expect("request 쓰기");
    });
    let (mut stream, _) = listener.accept().expect("test request 수락");

    let error = read_http_request(&mut stream).expect_err("chunked request 거부");
    writer.join().expect("request writer");
    assert!(error.contains("Transfer-Encoding"));
}
