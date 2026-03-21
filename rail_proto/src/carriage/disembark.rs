use tokio::io::AsyncWrite;
use crate::carriage::dataframe::DataFrame;

pub trait DisembarkStrategy {
    type DataFrame: DataFrame;
    type Error: std::error::Error;
    fn plan<W: AsyncWrite + Unpin>(writer: W) -> Self;
    async fn out_dataframe(&mut self, df: Self::DataFrame) -> Result<Option<()>, Self::Error>;

    async fn disembark<T: AsyncWrite + Unpin>(self, result_socket: T) -> Result<(), Self::Error>;
}