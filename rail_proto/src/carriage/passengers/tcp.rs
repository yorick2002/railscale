use bytes::BytesMut;
use tokio_util::codec::Decoder;

use crate::carriage::ticket_pipeline::{BufferedField, PassengerDecoder, TicketField};

pub struct TcpPassenger {
    buffer_predicate: fn(&[u8]) -> bool,
}

impl PassengerDecoder for TcpPassenger {
    fn with_predicate(buffer_predicate: fn(&[u8]) -> bool) -> Self {
        Self { buffer_predicate }
    }
}

impl Decoder for TcpPassenger {
    type Item = TicketField;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }

        let chunk = src.split().freeze();
        if (self.buffer_predicate)(&chunk) {
            Ok(Some(TicketField::Buffered(BufferedField::Bytes(chunk))))
        } else {
            Ok(Some(TicketField::Passthrough(chunk)))
        }
    }
}
