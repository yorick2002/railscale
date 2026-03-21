use tokio::io::AsyncRead;

pub trait DataFrame {
    type BufferedFrame: Send + Sync + Unpin;

    fn is_derived(&self) -> bool;
    fn get_id(&self) -> &'static str;
}

pub trait DerivedDataFrame {
    type D: DataFrame;
    fn derived(self) -> Self::D;
}



#[async_trait::async_trait]
pub trait DataFrameProducer<Input: AsyncRead + Send + Sync + Unpin> {
    type DataFrame: Send;
    type Error: std::error::Error;
    fn alloc_with_fresh_store() -> Self;
    async fn poll_read(&mut self, input: &mut Input) -> Result<Option<Self::DataFrame>, Self::Error>;
    async fn write_back_original(mut self) -> Result<Input, Self::Error>;
}