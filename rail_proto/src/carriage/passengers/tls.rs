use bytes::BytesMut;
use tokio_util::codec::Decoder;

use crate::carriage::ticket_pipeline::{BufferedField, PassengerDecoder, TicketField};

const TLS_RECORD_HEADER_LEN: usize = 5;

pub struct TlsPassenger {
    past_metadata: bool,
    buffer_predicate: fn(&[u8]) -> bool,
}

impl PassengerDecoder for TlsPassenger {
    fn with_predicate(buffer_predicate: fn(&[u8]) -> bool) -> Self {
        Self {
            past_metadata: false,
            buffer_predicate,
        }
    }
}

impl Decoder for TlsPassenger {
    type Item = TicketField;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.past_metadata {
            return Ok(None);
        }

        if src.len() < TLS_RECORD_HEADER_LEN {
            return Ok(None);
        }

        let record_len = u16::from_be_bytes([src[3], src[4]]) as usize;
        let total_len = TLS_RECORD_HEADER_LEN + record_len;

        if src.len() < total_len {
            return Ok(None);
        }

        let content_type = src[0];
        if content_type == 22 {
            let record = src.split_to(total_len).freeze();
            if (self.buffer_predicate)(&record) {
                return Ok(Some(TicketField::Buffered(BufferedField::Bytes(record))));
            } else {
                return Ok(Some(TicketField::Passthrough(record)));
            }
        }
        self.past_metadata = true;
        Ok(None)
    }
}
