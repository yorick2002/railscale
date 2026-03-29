use std::io;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tracing::{debug, error, info};

use rail_turnout::dns::resolver::DnsResolver;

pub struct DnsServer {
    socket: Arc<UdpSocket>,
    resolver: Arc<DnsResolver>,
}

impl DnsServer {
    pub async fn bind(addr: &str, resolver: DnsResolver) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        info!(addr = %socket.local_addr()?, "dns listening");
        Ok(Self {
            socket: Arc::new(socket),
            resolver: Arc::new(resolver),
        })
    }

    pub async fn run(&self) -> io::Result<()> {
        let mut buf = vec![0u8; 512];

        loop {
            let (len, src) = self.socket.recv_from(&mut buf).await?;
            let data = buf[..len].to_vec();
            let resolver = Arc::clone(&self.resolver);
            let socket = Arc::clone(&self.socket);

            debug!(%src, len, "dns query");

            tokio::spawn(async move {
                match resolver.resolve(&data).await {
                    Ok(response) => {
                        if let Err(e) = socket.send_to(&response, src).await {
                            error!(%src, %e, "failed to send dns response");
                        }
                    }
                    Err(e) => {
                        error!(%src, %e, "dns resolve error");
                    }
                }
            });
        }
    }
}
