use tokio::io::AsyncRead;

pub trait Manifest {
    type Metadata: Send + Sync + Unpin;

    fn get_id(&self) -> &'static str;
}

#[async_trait::async_trait]
pub trait ManifestExtractor<Input: AsyncRead + Send + Sync + Unpin> {
    type Metadata: Send;
    type Body: AsyncRead + Send + Sync + Unpin;
    type Error: std::error::Error;

    fn new() -> Self;
    async fn extract(&mut self, input: &mut Input) -> Result<Option<(Self::Metadata, Self::Body)>, Self::Error>;
}
