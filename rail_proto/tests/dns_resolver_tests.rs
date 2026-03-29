use std::net::Ipv4Addr;

use rail_turnout::dns::message::*;
use rail_turnout::dns::resolver::DnsResolver;

fn build_query(name: &str, qtype: u16) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&0x1234u16.to_be_bytes());
    buf.extend_from_slice(&0x0100u16.to_be_bytes());
    buf.extend_from_slice(&1u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    write_name(name, &mut buf);
    buf.extend_from_slice(&qtype.to_be_bytes());
    buf.extend_from_slice(&QCLASS_IN.to_be_bytes());
    buf
}

fn test_resolver() -> DnsResolver {
    let mut r = DnsResolver::new(
        "intranet.levandor.io".into(),
        "8.8.8.8:53".parse().unwrap(),
    );
    r.add_rule("postgres", Ipv4Addr::new(192, 168, 1, 13));
    r.add_rule("datalake", Ipv4Addr::new(192, 168, 1, 20));
    r
}

#[tokio::test]
async fn resolves_prefix_match() {
    let resolver = test_resolver();
    let query = build_query("postgres.intranet.levandor.io", QTYPE_A);
    let response = resolver.resolve(&query).await.unwrap();

    let header = DnsHeader::parse(&response).unwrap();
    assert_eq!(header.an_count, 1);
    assert_eq!(header.flags & 0x000F, 0); // no error

    let ip_offset = response.len() - 4;
    assert_eq!(&response[ip_offset..], &[192, 168, 1, 13]);
}

#[tokio::test]
async fn resolves_second_rule() {
    let resolver = test_resolver();
    let query = build_query("datalake.intranet.levandor.io", QTYPE_A);
    let response = resolver.resolve(&query).await.unwrap();

    let ip_offset = response.len() - 4;
    assert_eq!(&response[ip_offset..], &[192, 168, 1, 20]);
}

#[tokio::test]
async fn nxdomain_for_unknown_prefix() {
    let resolver = test_resolver();
    let query = build_query("unknown.intranet.levandor.io", QTYPE_A);
    let response = resolver.resolve(&query).await.unwrap();

    let header = DnsHeader::parse(&response).unwrap();
    assert_eq!(header.flags & 0x000F, 3); // NXDOMAIN
    assert_eq!(header.an_count, 0);
}

#[tokio::test]
async fn case_insensitive_match() {
    let resolver = test_resolver();
    let query = build_query("POSTGRES.INTRANET.LEVANDOR.IO", QTYPE_A);
    let response = resolver.resolve(&query).await.unwrap();

    let header = DnsHeader::parse(&response).unwrap();
    assert_eq!(header.an_count, 1);
}

#[tokio::test]
async fn preserves_query_id() {
    let resolver = test_resolver();
    let query = build_query("postgres.intranet.levandor.io", QTYPE_A);
    let response = resolver.resolve(&query).await.unwrap();

    let header = DnsHeader::parse(&response).unwrap();
    assert_eq!(header.id, 0x1234);
}

#[tokio::test]
async fn malformed_query_returns_error() {
    let resolver = test_resolver();
    let result = resolver.resolve(&[0; 5]).await;
    assert!(result.is_err());
}
