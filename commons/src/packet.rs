use bincode::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub enum Packet {
    Connect(Uuid),
    Disconnect(Uuid),
    Data(Uuid, Vec<u8>),
}

impl Packet {
    pub fn deserialize(data: Vec<u8>) -> Result<Packet> {
        bincode::deserialize(data.as_slice())
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(&self)
    }
}
