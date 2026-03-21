use std::error::Error;
use tokio_stream::Stream;
use tokio::io::{AsyncRead, AsyncWrite};

pub enum IngressSource {
    TerminatedTcpPacket,
    TerminatedHttpPacket,
    TlsTcpPacket,
    TlsHttpPacket,
}


#[async_trait::async_trait]
pub trait CarriageListener {
    type Ingress: AsyncWrite + AsyncRead + Send + Sync + Unpin;
    type IngressStream: Stream<Item = Self::Ingress>;
    type Error: Error;
    async fn create() -> Result<Self::IngressStream, Self::Error>;
    async fn accept(&self) -> Result<Self::Ingress, Self::Error>;
}