use std::net::Ipv4Addr;

const HEADER_LEN: usize = 12;

#[derive(Debug, Clone)]
pub struct DnsHeader {
    pub id: u16,
    pub flags: u16,
    pub qd_count: u16,
    pub an_count: u16,
    pub ns_count: u16,
    pub ar_count: u16,
}

impl DnsHeader {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < HEADER_LEN {
            return None;
        }
        Some(Self {
            id: u16::from_be_bytes([data[0], data[1]]),
            flags: u16::from_be_bytes([data[2], data[3]]),
            qd_count: u16::from_be_bytes([data[4], data[5]]),
            an_count: u16::from_be_bytes([data[6], data[7]]),
            ns_count: u16::from_be_bytes([data[8], data[9]]),
            ar_count: u16::from_be_bytes([data[10], data[11]]),
        })
    }

    pub fn write(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.id.to_be_bytes());
        buf.extend_from_slice(&self.flags.to_be_bytes());
        buf.extend_from_slice(&self.qd_count.to_be_bytes());
        buf.extend_from_slice(&self.an_count.to_be_bytes());
        buf.extend_from_slice(&self.ns_count.to_be_bytes());
        buf.extend_from_slice(&self.ar_count.to_be_bytes());
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DnsQuestion {
    pub name: String,
    pub qtype: u16,
    pub qclass: u16,
}

pub const QTYPE_A: u16 = 1;
pub const QCLASS_IN: u16 = 1;

pub fn parse_name(data: &[u8], mut offset: usize) -> Option<(String, usize)> {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut end_offset = 0;

    loop {
        if offset >= data.len() {
            return None;
        }

        let len = data[offset] as usize;

        if len == 0 {
            if !jumped {
                end_offset = offset + 1;
            }
            break;
        }

        if len & 0xC0 == 0xC0 {
            if offset + 1 >= data.len() {
                return None;
            }
            let ptr = ((len & 0x3F) << 8) | data[offset + 1] as usize;
            if !jumped {
                end_offset = offset + 2;
            }
            offset = ptr;
            jumped = true;
            continue;
        }

        offset += 1;
        if offset + len > data.len() {
            return None;
        }
        labels.push(String::from_utf8_lossy(&data[offset..offset + len]).into_owned());
        offset += len;
    }

    Some((labels.join("."), end_offset))
}

pub fn write_name(name: &str, buf: &mut Vec<u8>) {
    for label in name.split('.') {
        buf.push(label.len() as u8);
        buf.extend_from_slice(label.as_bytes());
    }
    buf.push(0);
}

impl DnsQuestion {
    pub fn parse(data: &[u8], offset: usize) -> Option<(Self, usize)> {
        let (name, offset) = parse_name(data, offset)?;
        if offset + 4 > data.len() {
            return None;
        }
        let qtype = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let qclass = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
        Some((Self { name, qtype, qclass }, offset + 4))
    }

    pub fn write(&self, buf: &mut Vec<u8>) {
        write_name(&self.name, buf);
        buf.extend_from_slice(&self.qtype.to_be_bytes());
        buf.extend_from_slice(&self.qclass.to_be_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct DnsQuery {
    pub header: DnsHeader,
    pub questions: Vec<DnsQuestion>,
    pub raw_len: usize,
}

impl DnsQuery {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let header = DnsHeader::parse(data)?;
        let mut offset = HEADER_LEN;
        let mut questions = Vec::with_capacity(header.qd_count as usize);

        for _ in 0..header.qd_count {
            let (q, new_offset) = DnsQuestion::parse(data, offset)?;
            questions.push(q);
            offset = new_offset;
        }

        Some(Self {
            header,
            questions,
            raw_len: offset,
        })
    }
}

pub struct DnsResponse;

impl DnsResponse {
    pub fn a_record(query: &DnsQuery, question: &DnsQuestion, addr: Ipv4Addr, ttl: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);

        let flags = 0x8180; // response, recursion desired + available, no error
        let header = DnsHeader {
            id: query.header.id,
            flags,
            qd_count: 1,
            an_count: 1,
            ns_count: 0,
            ar_count: 0,
        };
        header.write(&mut buf);
        question.write(&mut buf);

        write_name(&question.name, &mut buf);
        buf.extend_from_slice(&QTYPE_A.to_be_bytes());
        buf.extend_from_slice(&QCLASS_IN.to_be_bytes());
        buf.extend_from_slice(&ttl.to_be_bytes());
        buf.extend_from_slice(&4u16.to_be_bytes()); // rdlength
        buf.extend_from_slice(&addr.octets());

        buf
    }

    pub fn nxdomain(query: &DnsQuery, question: &DnsQuestion) -> Vec<u8> {
        let mut buf = Vec::with_capacity(32);

        let flags = 0x8183; // response, recursion desired + available, NXDOMAIN
        let header = DnsHeader {
            id: query.header.id,
            flags,
            qd_count: 1,
            an_count: 0,
            ns_count: 0,
            ar_count: 0,
        };
        header.write(&mut buf);
        question.write(&mut buf);

        buf
    }
}
