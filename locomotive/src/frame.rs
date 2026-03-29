use bytes::Bytes;
use train_track::Frame;

pub struct HttpFrame {
    data: Bytes,
    routing: bool,
}

impl HttpFrame {
    pub fn header(data: Bytes, is_request_line: bool) -> Self {
        Self { data, routing: is_request_line }
    }
}

impl Frame for HttpFrame {
    fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    fn into_bytes(self) -> Bytes {
        self.data
    }

    fn is_routing_frame(&self) -> bool {
        self.routing
    }
}
