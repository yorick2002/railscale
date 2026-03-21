use std::collections::HashMap;
use tokio::io::AsyncRead;
use crate::carriage::dataframe::DataFrame;

pub struct FrameInspection {
    pass: bool,
    detail: HashMap<&'static str, String>,
}

#[async_trait::async_trait]
pub trait FrameConductor<F: DataFrame + AsyncRead> {
    type Error: std::error::Error;
    fn for_frame() -> Self;
    async fn inspect(&self, frame: F) -> Result<FrameInspection, Self::Error>;
}