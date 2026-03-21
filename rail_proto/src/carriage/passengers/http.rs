use bytes::BytesMut;
use tokio_util::codec::Decoder;

use crate::carriage::ticket_pipeline::{BufferedField, PassengerDecoder, TicketField};

pub struct HttpPassenger {
    past_metadata: bool,
    buffer_predicate: fn(&[u8]) -> bool,
}

impl PassengerDecoder for HttpPassenger {
    fn with_predicate(buffer_predicate: fn(&[u8]) -> bool) -> Self {
        Self {
            past_metadata: false,
            buffer_predicate,
        }
    }
}

impl Decoder for HttpPassenger {
    type Item = TicketField;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.past_metadata {
            return Ok(None);
        }

        let newline_pos = src.iter().position(|&b| b == b'\n');
        match newline_pos {
            Some(pos) => {
                let line_bytes = src.split_to(pos + 1).freeze();

                let trimmed = line_bytes.iter().filter(|&&b| b != b'\r' && b != b'\n').count();
                if trimmed == 0 {
                    self.past_metadata = true;
                    return Ok(Some(TicketField::Boundary));
                }

                if (self.buffer_predicate)(&line_bytes) {
                    Ok(Some(TicketField::Buffered(BufferedField::Bytes(line_bytes))))
                } else {
                    Ok(Some(TicketField::Passthrough(line_bytes)))
                }
            }
            None => Ok(None),
        }
    }
}
