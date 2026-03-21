use bytes::BytesMut;
use tokio_util::codec::Decoder;

use rail_proto::carriage::passengers::http::HttpPassenger;
use rail_proto::carriage::ticket_pipeline::{PassengerDecoder, TicketField};

fn always_buffer(_: &[u8]) -> bool { true }
fn never_buffer(_: &[u8]) -> bool { false }
fn buffer_user_agent(line: &[u8]) -> bool {
    line.windows(10).any(|w| w.eq_ignore_ascii_case(b"user-agent"))
}

#[test]
fn decodes_single_header_line() {
    let mut decoder = HttpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::from("Host: example.com\r\n");
    let result = decoder.decode(&mut buf).unwrap();
    assert!(matches!(result, Some(TicketField::Buffered(_))));
    assert!(buf.is_empty());
}

#[test]
fn passthrough_when_predicate_false() {
    let mut decoder = HttpPassenger::with_predicate(never_buffer);
    let mut buf = BytesMut::from("Host: example.com\r\n");
    let result = decoder.decode(&mut buf).unwrap();
    assert!(matches!(result, Some(TicketField::Passthrough(_))));
}

#[test]
fn detects_boundary_crlf() {
    let mut decoder = HttpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::from("\r\n");
    let result = decoder.decode(&mut buf).unwrap();
    assert!(matches!(result, Some(TicketField::Boundary)));
}

#[test]
fn detects_boundary_lf_only() {
    let mut decoder = HttpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::from("\n");
    let result = decoder.decode(&mut buf).unwrap();
    assert!(matches!(result, Some(TicketField::Boundary)));
}

#[test]
fn body_after_boundary_returns_none() {
    let mut decoder = HttpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::from("\r\n");
    decoder.decode(&mut buf).unwrap(); // consume boundary

    buf.extend_from_slice(b"body content here");
    let result = decoder.decode(&mut buf).unwrap();
    assert!(result.is_none());
    // body bytes stay in the buffer for raw stream consumption
    assert_eq!(&buf[..], b"body content here");
}

#[test]
fn returns_none_on_incomplete_line() {
    let mut decoder = HttpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::from("Host: example");
    let result = decoder.decode(&mut buf).unwrap();
    assert!(result.is_none());
    assert_eq!(buf.len(), 13); // data preserved
}

#[test]
fn selective_buffering_by_header_name() {
    let mut decoder = HttpPassenger::with_predicate(buffer_user_agent);
    let mut buf = BytesMut::from("Host: example.com\r\nUser-Agent: curl/7.x\r\nAccept: */*\r\n\r\n");

    let r1 = decoder.decode(&mut buf).unwrap();
    assert!(matches!(r1, Some(TicketField::Passthrough(_)))); // Host

    let r2 = decoder.decode(&mut buf).unwrap();
    assert!(matches!(r2, Some(TicketField::Buffered(_)))); // User-Agent

    let r3 = decoder.decode(&mut buf).unwrap();
    assert!(matches!(r3, Some(TicketField::Passthrough(_)))); // Accept

    let r4 = decoder.decode(&mut buf).unwrap();
    assert!(matches!(r4, Some(TicketField::Boundary))); // empty line
}

#[test]
fn full_http_request_flow() {
    let mut decoder = HttpPassenger::with_predicate(buffer_user_agent);
    let request = "GET /path HTTP/1.1\r\n\
                   Host: example.com\r\n\
                   User-Agent: test/1.0\r\n\
                   Content-Length: 5\r\n\
                   \r\n\
                   hello";
    let mut buf = BytesMut::from(request);

    // request line — passthrough
    assert!(matches!(decoder.decode(&mut buf).unwrap(), Some(TicketField::Passthrough(_))));
    // Host — passthrough
    assert!(matches!(decoder.decode(&mut buf).unwrap(), Some(TicketField::Passthrough(_))));
    // User-Agent — buffered
    assert!(matches!(decoder.decode(&mut buf).unwrap(), Some(TicketField::Buffered(_))));
    // Content-Length — passthrough
    assert!(matches!(decoder.decode(&mut buf).unwrap(), Some(TicketField::Passthrough(_))));
    // boundary
    assert!(matches!(decoder.decode(&mut buf).unwrap(), Some(TicketField::Boundary)));
    // body — decoder returns None, bytes remain in buffer
    assert!(decoder.decode(&mut buf).unwrap().is_none());
    assert!(!buf.is_empty()); // body bytes preserved for raw stream
}

#[test]
fn empty_body_after_boundary() {
    let mut decoder = HttpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::from("Host: x\r\n\r\n");

    decoder.decode(&mut buf).unwrap(); // header
    decoder.decode(&mut buf).unwrap(); // boundary

    let result = decoder.decode(&mut buf).unwrap();
    assert!(result.is_none()); // empty body = None
}

#[test]
fn incremental_feed() {
    let mut decoder = HttpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::new();

    // feed partial line
    buf.extend_from_slice(b"Host: exa");
    assert!(decoder.decode(&mut buf).unwrap().is_none());

    // complete the line
    buf.extend_from_slice(b"mple.com\r\n");
    assert!(matches!(decoder.decode(&mut buf).unwrap(), Some(TicketField::Buffered(_))));
}

#[test]
fn body_bytes_preserved_for_raw_stream() {
    let mut decoder = HttpPassenger::with_predicate(never_buffer);
    let mut buf = BytesMut::from("\r\n");
    decoder.decode(&mut buf).unwrap(); // boundary

    buf.extend_from_slice(b"chunk1chunk2");
    assert!(decoder.decode(&mut buf).unwrap().is_none());
    assert_eq!(&buf[..], b"chunk1chunk2"); // all body bytes preserved
}
