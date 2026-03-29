use std::net::Ipv4Addr;

use rail_turnout::dns::message::*;

fn build_query(name: &str, qtype: u16) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&0xABCDu16.to_be_bytes()); // id
    buf.extend_from_slice(&0x0100u16.to_be_bytes()); // flags: standard query, recursion desired
    buf.extend_from_slice(&1u16.to_be_bytes());      // qd_count
    buf.extend_from_slice(&0u16.to_be_bytes());      // an_count
    buf.extend_from_slice(&0u16.to_be_bytes());      // ns_count
    buf.extend_from_slice(&0u16.to_be_bytes());      // ar_count
    write_name(name, &mut buf);
    buf.extend_from_slice(&qtype.to_be_bytes());
    buf.extend_from_slice(&QCLASS_IN.to_be_bytes());
    buf
}

#[test]
fn parse_header() {
    let data = build_query("example.com", QTYPE_A);
    let header = DnsHeader::parse(&data).unwrap();
    assert_eq!(header.id, 0xABCD);
    assert_eq!(header.qd_count, 1);
    assert_eq!(header.an_count, 0);
}

#[test]
fn parse_header_too_short() {
    assert!(DnsHeader::parse(&[0; 11]).is_none());
}

#[test]
fn parse_question() {
    let data = build_query("postgres.intranet.levandor.io", QTYPE_A);
    let query = DnsQuery::parse(&data).unwrap();
    assert_eq!(query.questions.len(), 1);
    assert_eq!(query.questions[0].name, "postgres.intranet.levandor.io");
    assert_eq!(query.questions[0].qtype, QTYPE_A);
    assert_eq!(query.questions[0].qclass, QCLASS_IN);
}

#[test]
fn parse_name_single_label() {
    let mut buf = Vec::new();
    write_name("localhost", &mut buf);
    let (name, end) = parse_name(&buf, 0).unwrap();
    assert_eq!(name, "localhost");
    assert_eq!(end, buf.len());
}

#[test]
fn parse_name_multi_label() {
    let mut buf = Vec::new();
    write_name("a.b.c.d", &mut buf);
    let (name, _) = parse_name(&buf, 0).unwrap();
    assert_eq!(name, "a.b.c.d");
}

#[test]
fn parse_name_with_pointer() {
    let mut buf = Vec::new();
    write_name("example.com", &mut buf);
    let ptr_offset = buf.len();
    buf.push(0xC0);
    buf.push(0x00);
    let (name, end) = parse_name(&buf, ptr_offset).unwrap();
    assert_eq!(name, "example.com");
    assert_eq!(end, ptr_offset + 2);
}

#[test]
fn roundtrip_name() {
    let mut buf = Vec::new();
    write_name("postgres.intranet.levandor.io", &mut buf);
    let (name, _) = parse_name(&buf, 0).unwrap();
    assert_eq!(name, "postgres.intranet.levandor.io");
}

#[test]
fn a_record_response() {
    let data = build_query("test.example.com", QTYPE_A);
    let query = DnsQuery::parse(&data).unwrap();
    let question = &query.questions[0];

    let response = DnsResponse::a_record(&query, question, Ipv4Addr::new(10, 0, 0, 1), 300);

    let resp_header = DnsHeader::parse(&response).unwrap();
    assert_eq!(resp_header.id, 0xABCD);
    assert_eq!(resp_header.flags & 0x8000, 0x8000); // QR bit set (response)
    assert_eq!(resp_header.flags & 0x000F, 0); // no error
    assert_eq!(resp_header.qd_count, 1);
    assert_eq!(resp_header.an_count, 1);

    let ip_offset = response.len() - 4;
    assert_eq!(&response[ip_offset..], &[10, 0, 0, 1]);
}

#[test]
fn nxdomain_response() {
    let data = build_query("nope.example.com", QTYPE_A);
    let query = DnsQuery::parse(&data).unwrap();
    let question = &query.questions[0];

    let response = DnsResponse::nxdomain(&query, question);

    let resp_header = DnsHeader::parse(&response).unwrap();
    assert_eq!(resp_header.id, 0xABCD);
    assert_eq!(resp_header.flags & 0x000F, 3); // NXDOMAIN rcode
    assert_eq!(resp_header.an_count, 0);
}

#[test]
fn parse_malformed_returns_none() {
    assert!(DnsQuery::parse(&[0; 5]).is_none());
    assert!(DnsQuery::parse(&[]).is_none());
}

#[test]
fn parse_truncated_question() {
    let mut data = build_query("test.com", QTYPE_A);
    data.truncate(data.len() - 2);
    assert!(DnsQuery::parse(&data).is_none());
}
