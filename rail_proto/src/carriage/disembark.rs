use tokio::io::{AsyncRead, AsyncWrite};
use crate::carriage::manifest::Manifest;

pub trait DisembarkStrategy {
    type Manifest: Manifest;
    type Body: AsyncRead + Send + Sync + Unpin;
    type Error: std::error::Error;

    fn plan<W: AsyncWrite + Unpin>(writer: W) -> Self;
    async fn send(&mut self, manifest: Self::Manifest, body: Self::Body) -> Result<(), Self::Error>;
    async fn disembark<T: AsyncWrite + Unpin>(self, result_socket: T) -> Result<(), Self::Error>;
}
