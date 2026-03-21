use bytes::BytesMut;
use proptest::prelude::*;
use tokio_util::codec::Decoder;

use rail_proto::carriage::passengers::http::HttpPassenger;
use rail_proto::carriage::passengers::tcp::TcpPassenger;
use rail_proto::carriage::passengers::tls::TlsPassenger;
use rail_proto::carriage::ticket_pipeline::{PassengerDecoder, TicketField};

fn always_buffer(_: &[u8]) -> bool { true }
fn never_buffer(_: &[u8]) -> bool { false }

// ── HTTP property tests ──

proptest! {
    #[test]
    fn http_never_panics_on_arbitrary_input(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let mut decoder = HttpPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::from(data.as_slice());
        // must not panic, errors are acceptable
        while !buf.is_empty() {
            match decoder.decode(&mut buf) {
                Ok(Some(_)) => {},
                Ok(None) => break,
                Err(_) => break,
            }
        }
    }

    #[test]
    fn http_body_preserved_after_metadata(
        header_count in 1..20usize,
        body_len in 0..1024usize,
    ) {
        let mut request = String::from("GET / HTTP/1.1\r\n");
        for i in 0..header_count {
            request.push_str(&format!("X-Header-{i}: value{i}\r\n"));
        }
        request.push_str("\r\n");
        let body: Vec<u8> = (0..body_len).map(|i| ((i % 94) as u8) + 33).collect();
        let body_str = String::from_utf8_lossy(&body).to_string();
        request.push_str(&body_str);

        let mut decoder = HttpPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::from(request.as_str());

        loop {
            match decoder.decode(&mut buf) {
                Ok(Some(_)) => {},
                Ok(None) => break,
                Err(_) => break,
            }
        }
        // body bytes should remain in the buffer for raw stream
        prop_assert_eq!(buf.len(), body_len);
    }

    #[test]
    fn http_boundary_only_on_empty_line(data in "[a-zA-Z0-9: ]{1,100}\r\n") {
        let mut decoder = HttpPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::from(data.as_str());
        let result = decoder.decode(&mut buf).unwrap();
        // non-empty line should never be a boundary
        prop_assert!(!matches!(result, Some(TicketField::Boundary)));
    }

    #[test]
    fn http_incremental_feed_same_result(
        lines in proptest::collection::vec("[a-zA-Z0-9: -]{1,80}\r\n", 1..10),
    ) {
        let full_input: String = lines.join("");
        let full_input = full_input + "\r\n"; // add boundary

        // decode all at once
        let mut dec_full = HttpPassenger::with_predicate(always_buffer);
        let mut buf_full = BytesMut::from(full_input.as_str());
        let mut results_full = Vec::new();
        while let Ok(Some(field)) = dec_full.decode(&mut buf_full) {
            results_full.push(field_tag(&field));
            if matches!(field, TicketField::Boundary) { break; }
        }

        // decode line by line
        let mut dec_inc = HttpPassenger::with_predicate(always_buffer);
        let mut buf_inc = BytesMut::new();
        let mut results_inc = Vec::new();
        for line in &lines {
            buf_inc.extend_from_slice(line.as_bytes());
            while let Ok(Some(field)) = dec_inc.decode(&mut buf_inc) {
                results_inc.push(field_tag(&field));
                if matches!(field, TicketField::Boundary) { break; }
            }
        }
        buf_inc.extend_from_slice(b"\r\n");
        while let Ok(Some(field)) = dec_inc.decode(&mut buf_inc) {
            results_inc.push(field_tag(&field));
            if matches!(field, TicketField::Boundary) { break; }
        }

        prop_assert_eq!(results_full, results_inc);
    }
}

// ── TCP property tests ──

proptest! {
    #[test]
    fn tcp_never_panics_on_arbitrary_input(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let mut decoder = TcpPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::from(data.as_slice());
        while !buf.is_empty() {
            match decoder.decode(&mut buf) {
                Ok(Some(_)) => {},
                Ok(None) => break,
                Err(_) => break,
            }
        }
    }

    #[test]
    fn tcp_never_produces_boundary(data in proptest::collection::vec(any::<u8>(), 1..4096)) {
        let mut decoder = TcpPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::from(data.as_slice());
        while let Ok(Some(field)) = decoder.decode(&mut buf) {
            prop_assert!(!matches!(field, TicketField::Boundary));
        }
    }

    #[test]
    fn tcp_consumes_all_nonempty_input(data in proptest::collection::vec(any::<u8>(), 1..4096)) {
        let mut decoder = TcpPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::from(data.as_slice());
        let result = decoder.decode(&mut buf).unwrap();
        prop_assert!(result.is_some());
        prop_assert!(buf.is_empty());
    }

    #[test]
    fn tcp_predicate_determines_variant(data in proptest::collection::vec(any::<u8>(), 1..256)) {
        // always_buffer → Buffered
        let mut dec_buf = TcpPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::from(data.as_slice());
        let result = dec_buf.decode(&mut buf).unwrap();
        prop_assert!(matches!(result, Some(TicketField::Buffered(_))));

        // never_buffer → Passthrough
        let mut dec_pass = TcpPassenger::with_predicate(never_buffer);
        let mut buf = BytesMut::from(data.as_slice());
        let result = dec_pass.decode(&mut buf).unwrap();
        prop_assert!(matches!(result, Some(TicketField::Passthrough(_))));
    }
}

// ── TLS property tests ──

fn make_tls_record(content_type: u8, payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u16;
    let mut record = vec![content_type, 0x03, 0x03];
    record.extend_from_slice(&len.to_be_bytes());
    record.extend_from_slice(payload);
    record
}

proptest! {
    #[test]
    fn tls_never_panics_on_arbitrary_input(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let mut decoder = TlsPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::from(data.as_slice());
        while !buf.is_empty() {
            match decoder.decode(&mut buf) {
                Ok(Some(_)) => {},
                Ok(None) => break,
                Err(_) => break,
            }
        }
    }

    #[test]
    fn tls_never_produces_boundary(
        payloads in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..256), 1..10),
    ) {
        let mut decoder = TlsPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::new();
        // only use handshake records — app data stops decoding
        for payload in payloads.iter() {
            buf.extend_from_slice(&make_tls_record(22, payload));
        }

        while let Ok(Some(field)) = decoder.decode(&mut buf) {
            prop_assert!(!matches!(field, TicketField::Boundary));
        }
    }

    #[test]
    fn tls_handshake_records_buffered_then_app_data_preserved(
        hs_count in 1..5usize,
        hs_payload_len in 1..128usize,
    ) {
        let mut decoder = TlsPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::new();

        let hs_payload: Vec<u8> = (0..hs_payload_len).map(|i| (i % 256) as u8).collect();
        for _ in 0..hs_count {
            buf.extend_from_slice(&make_tls_record(22, &hs_payload));
        }
        let app_record = make_tls_record(23, b"app");
        let app_len = app_record.len();
        buf.extend_from_slice(&app_record);

        let mut buffered_count = 0;
        while let Ok(Some(field)) = decoder.decode(&mut buf) {
            match field {
                TicketField::Buffered(_) => buffered_count += 1,
                TicketField::Passthrough(_) => {},
                TicketField::Boundary => panic!("TLS should never produce boundary"),
            }
        }
        prop_assert_eq!(buffered_count, hs_count);
        // app data record preserved in buffer for raw stream
        prop_assert_eq!(buf.len(), app_len);
    }

    #[test]
    fn tls_record_length_respected(payload_len in 0..1024usize) {
        let mut decoder = TlsPassenger::with_predicate(always_buffer);
        let payload: Vec<u8> = (0..payload_len).map(|i| (i % 256) as u8).collect();
        let record = make_tls_record(22, &payload);
        let total_len = record.len();

        let mut buf = BytesMut::from(record.as_slice());
        let result = decoder.decode(&mut buf).unwrap();
        prop_assert!(result.is_some());
        prop_assert!(buf.is_empty());

        // verify record was the right size
        if let Some(TicketField::Buffered(rail_proto::carriage::ticket_pipeline::BufferedField::Bytes(b))) = result {
            prop_assert_eq!(b.len(), total_len);
        }
    }

    #[test]
    fn tls_incremental_never_loses_data(payload in proptest::collection::vec(any::<u8>(), 1..256)) {
        let record = make_tls_record(22, &payload);
        let mut decoder = TlsPassenger::with_predicate(always_buffer);
        let mut buf = BytesMut::new();

        // feed one byte at a time
        let mut got_result = false;
        for &byte in &record {
            buf.extend_from_slice(&[byte]);
            if let Ok(Some(_)) = decoder.decode(&mut buf) {
                got_result = true;
                break;
            }
        }
        prop_assert!(got_result);
        prop_assert!(buf.is_empty());
    }
}

// ── Helpers ──

fn field_tag(field: &TicketField) -> &'static str {
    match field {
        TicketField::Buffered(_) => "buffered",
        TicketField::Passthrough(_) => "passthrough",
        TicketField::Boundary => "boundary",
    }
}
