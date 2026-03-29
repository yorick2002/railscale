use tokio::io::AsyncRead;
use crate::RailscaleError;

pub trait StreamSource: Send {
    type Stream: AsyncRead + Send + Unpin;
    type Error: Into<RailscaleError>;

    fn accept(&self) -> impl std::future::Future<Output = Result<Self::Stream, Self::Error>> + Send;
}
