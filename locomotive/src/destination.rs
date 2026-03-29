use std::net::SocketAddr;
use bytes::Bytes;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use train_track::{Frame, StreamDestination};
use crate::HttpFrame;

pub struct TcpDestination {
    upstream: Option<TcpStream>,
}

impl TcpDestination {
    pub fn new() -> Self {
        Self { upstream: None }
    }

    pub async fn provide_with_addr(&mut self, addr: &SocketAddr) -> Result<(), std::io::Error> {
        let stream = TcpStream::connect(addr).await?;
        self.upstream = Some(stream);
        Ok(())
    }
}

impl StreamDestination for TcpDestination {
    type Frame = HttpFrame;
    type Error = std::io::Error;

    async fn provide(&mut self, routing_frame: &Self::Frame) -> Result<(), Self::Error> {
        let line = routing_frame.as_bytes();
        let host = extract_host(line).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "no host in routing frame")
        })?;
        let stream = TcpStream::connect(host).await?;
        self.upstream = Some(stream);
        Ok(())
    }

    async fn write(&mut self, frame: Self::Frame) -> Result<(), Self::Error> {
        let upstream = self.upstream.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotConnected, "not routed")
        })?;
        upstream.write_all(frame.as_bytes()).await
    }

    async fn write_raw(&mut self, bytes: Bytes) -> Result<(), Self::Error> {
        let upstream = self.upstream.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotConnected, "not routed")
        })?;
        upstream.write_all(&bytes).await
    }
}

fn extract_host(request_line: &[u8]) -> Option<String> {
    let line = std::str::from_utf8(request_line).ok()?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        let uri = parts[1];
        if uri.contains("://") {
            uri.split("://").nth(1).map(|h| {
                h.split('/').next().unwrap_or(h).to_string()
            })
        } else {
            Some(uri.trim_start_matches('/').to_string())
        }
    } else {
        None
    }
}
