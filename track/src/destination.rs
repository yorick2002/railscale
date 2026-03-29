use bytes::Bytes;
use crate::frame::Frame;
use crate::RailscaleError;

pub trait StreamDestination: Send {
    type Frame: Frame;
    type Error: Into<RailscaleError>;

    fn provide(&mut self, routing_frame: &Self::Frame) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
    fn write(&mut self, frame: Self::Frame) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
    fn write_raw(&mut self, bytes: Bytes) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
}
