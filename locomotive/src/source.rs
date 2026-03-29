use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use train_track::{StreamSource, RailscaleError};

pub struct TcpSource {
    listener: TcpListener,
}

impl TcpSource {
    pub async fn bind(addr: &str) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self { listener })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }
}

impl StreamSource for TcpSource {
    type Stream = TcpStream;
    type Error = std::io::Error;

    async fn accept(&self) -> Result<Self::Stream, Self::Error> {
        let (stream, _addr) = self.listener.accept().await?;
        Ok(stream)
    }
}
