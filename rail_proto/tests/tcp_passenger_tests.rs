use bytes::BytesMut;
use tokio_util::codec::Decoder;

use rail_carriage::passengers::tcp::TcpPassenger;
use rail_carriage::ticket_pipeline::{PassengerDecoder, TicketField};

fn always_buffer(_: &[u8]) -> bool { true }
fn never_buffer(_: &[u8]) -> bool { false }
fn buffer_short(data: &[u8]) -> bool { data.len() < 10 }

#[test]
fn empty_buffer_returns_none() {
    let mut decoder = TcpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::new();
    assert!(decoder.decode(&mut buf).unwrap().is_none());
}

#[test]
fn buffers_when_predicate_true() {
    let mut decoder = TcpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::from("hello");
    let result = decoder.decode(&mut buf).unwrap();
    assert!(matches!(result, Some(TicketField::Buffered(_))));
    assert!(buf.is_empty());
}

#[test]
fn passthrough_when_predicate_false() {
    let mut decoder = TcpPassenger::with_predicate(never_buffer);
    let mut buf = BytesMut::from("hello");
    let result = decoder.decode(&mut buf).unwrap();
    assert!(matches!(result, Some(TicketField::Passthrough(ref b)) if b == "hello"));
}

#[test]
fn predicate_receives_full_chunk() {
    let mut decoder = TcpPassenger::with_predicate(buffer_short);

    let mut buf = BytesMut::from("short");
    assert!(matches!(decoder.decode(&mut buf).unwrap(), Some(TicketField::Buffered(_))));

    let mut buf = BytesMut::from("this is a long chunk of data");
    assert!(matches!(decoder.decode(&mut buf).unwrap(), Some(TicketField::Passthrough(_))));
}

#[test]
fn consecutive_decodes_drain_buffer() {
    let mut decoder = TcpPassenger::with_predicate(always_buffer);
    let mut buf = BytesMut::from("data");

    decoder.decode(&mut buf).unwrap();
    assert!(buf.is_empty());

    assert!(decoder.decode(&mut buf).unwrap().is_none());
}

#[test]
fn never_produces_boundary() {
    let mut decoder = TcpPassenger::with_predicate(always_buffer);
    // feed various data patterns — TCP should never emit Boundary
    for input in ["\r\n", "\n", "\r\n\r\n", "", "data\r\nmore\r\n"] {
        let mut buf = BytesMut::from(input);
        while let Some(field) = decoder.decode(&mut buf).unwrap() {
            assert!(!matches!(field, TicketField::Boundary));
        }
    }
}

#[test]
fn binary_data_handled() {
    let mut decoder = TcpPassenger::with_predicate(always_buffer);
    let binary: &[u8] = &[0x00, 0xFF, 0xFE, 0x01, 0x80];
    let mut buf = BytesMut::from(binary);
    let result = decoder.decode(&mut buf).unwrap();
    assert!(matches!(result, Some(TicketField::Buffered(_))));
}
