use std::io;

use bytes::BytesMut;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Decoder;
use tracing::{debug, error, info};

use rail_carriage::passengers::http::HttpPassenger;
use rail_carriage::ticket_pipeline::{BufferedField, PassengerDecoder, TicketField};

pub struct DevListener {
    listener: TcpListener,
}

impl DevListener {
    pub async fn bind(addr: &str) -> io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        info!(addr = %listener.local_addr()?, "listening");
        Ok(Self { listener })
    }

    pub async fn run(&self) -> io::Result<()> {
        loop {
            let (stream, peer) = self.listener.accept().await?;
            debug!(%peer, "connection");
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream).await {
                    error!(%peer, %e, "connection error");
                }
            });
        }
    }
}

async fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    let mut raw = BytesMut::with_capacity(4096);
    let mut decoder = HttpPassenger::with_predicate(|_| true);

    loop {
        let n = stream.read_buf(&mut raw).await?;
        if n == 0 {
            return Ok(());
        }

        loop {
            match decoder.decode(&mut raw)? {
                Some(TicketField::Boundary) => {
                    let body = "railscale dev server\n";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body,
                    );
                    tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes()).await?;
                    return Ok(());
                }
                Some(field) => log_field(&field),
                None => break,
            }
        }
    }
}

fn log_field(field: &TicketField) {
    match field {
        TicketField::Buffered(bf) => match bf {
            BufferedField::Attribute(a) => debug!(request_line = %a),
            BufferedField::Header(k, v) => debug!(header_key = %k, header_value = %v),
            BufferedField::KeyValue(k, v) => debug!(%k, %v),
            BufferedField::Bytes(b) => debug!(len = b.len(), "raw bytes"),
        },
        TicketField::Passthrough(b) => debug!(len = b.len(), "passthrough"),
        TicketField::Boundary => {}
    }
}
