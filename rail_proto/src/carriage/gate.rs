use std::collections::HashMap;
use crate::carriage::manifest::Manifest;

pub struct Stamp {
    pub entries: HashMap<String, String>,
}

impl Stamp {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    pub fn add(&mut self, key: String, value: String) {
        self.entries.insert(key, value);
    }
}

pub struct GateVerdict {
    pub stamp: Stamp,
}

#[async_trait::async_trait]
pub trait TicketGate<M: Manifest> {
    type Error: std::error::Error;

    async fn evaluate(&self, manifest: &M) -> Result<GateVerdict, Self::Error>;
}
