use std::io;
use std::net::{Ipv4Addr, SocketAddr};

use tokio::net::UdpSocket;
use tracing::debug;

use crate::dns::message::{DnsQuery, DnsResponse, QCLASS_IN, QTYPE_A};

pub struct ResolveRule {
    pub prefix: String,
    pub addr: Ipv4Addr,
}

pub struct DnsResolver {
    domain: String,
    rules: Vec<ResolveRule>,
    upstream: SocketAddr,
    ttl: u32,
}

impl DnsResolver {
    pub fn new(domain: String, upstream: SocketAddr) -> Self {
        Self {
            domain,
            rules: Vec::new(),
            upstream,
            ttl: 60,
        }
    }

    pub fn add_rule(&mut self, prefix: &str, addr: Ipv4Addr) {
        self.rules.push(ResolveRule {
            prefix: prefix.to_string(),
            addr,
        });
    }

    pub fn with_ttl(mut self, ttl: u32) -> Self {
        self.ttl = ttl;
        self
    }

    pub async fn resolve(&self, data: &[u8]) -> io::Result<Vec<u8>> {
        let query = DnsQuery::parse(data).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "malformed DNS query")
        })?;

        let question = query.questions.first().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "no questions in query")
        })?;

        if question.qtype != QTYPE_A || question.qclass != QCLASS_IN {
            return self.forward_upstream(data).await;
        }

        let name = question.name.to_lowercase();
        let suffix = format!(".{}", self.domain);

        if !name.ends_with(&suffix) && name != self.domain {
            debug!(name = %name, "not our domain, forwarding upstream");
            return self.forward_upstream(data).await;
        }

        let prefix = if name == self.domain {
            ""
        } else {
            &name[..name.len() - suffix.len()]
        };

        for rule in &self.rules {
            if rule.prefix == prefix {
                debug!(name = %name, addr = %rule.addr, "resolved via prefix rule");
                return Ok(DnsResponse::a_record(&query, question, rule.addr, self.ttl));
            }
        }

        debug!(name = %name, "no matching rule, NXDOMAIN");
        Ok(DnsResponse::nxdomain(&query, question))
    }

    async fn forward_upstream(&self, data: &[u8]) -> io::Result<Vec<u8>> {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        sock.send_to(data, self.upstream).await?;

        let mut buf = vec![0u8; 512];
        let (len, _) = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            sock.recv_from(&mut buf),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "upstream DNS timeout"))??;

        buf.truncate(len);
        Ok(buf)
    }
}
