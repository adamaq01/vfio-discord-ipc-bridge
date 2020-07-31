use std::io::Error;

use async_trait::async_trait;
use bytes::BufMut;
use parity_tokio_ipc::{Connection, Endpoint};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ErrorKind, ReadHalf, WriteHalf};

#[async_trait]
pub trait DiscordRead {
    async fn receive_data(&mut self) -> Result<Vec<u8>, Error>;
    async fn receive_data_exact(&mut self, length: usize) -> Result<Vec<u8>, Error>;
}

#[async_trait]
pub trait DiscordWrite {
    async fn send_packet(&mut self, op_code: u32, json: String) -> Result<(), Error>;
    async fn send_data(&mut self, data: Vec<u8>) -> Result<(), Error>;
    async fn shutdown(&mut self) -> Result<(), Error>;
}

#[async_trait]
impl DiscordRead for Discord {
    async fn receive_data(&mut self) -> Result<Vec<u8>, Error> {
        let mut buf = [0u8; commons::BUFFER_SIZE];
        self.connection
            .read(&mut buf[..])
            .await
            .map(|length| buf[..length].to_vec())
    }

    async fn receive_data_exact(&mut self, length: usize) -> Result<Vec<u8>, Error> {
        let mut buf = [0u8; commons::BUFFER_SIZE];
        self.connection
            .read_exact(&mut buf[..length])
            .await
            .map(|length| buf[..length].to_vec())
    }
}

#[async_trait]
impl DiscordWrite for Discord {
    async fn send_packet(&mut self, op_code: u32, json: String) -> Result<(), Error> {
        let mut packet = vec![0u8; 0];
        packet.put_u32_le(op_code);
        packet.put_u32_le(json.len() as u32);
        packet.put(json.as_bytes());
        self.connection.write_all(packet.as_slice()).await
    }

    async fn send_data(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.connection.write_all(data.as_slice()).await
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        self.connection.shutdown().await
    }
}

pub struct DiscordReadHalf {
    rx: ReadHalf<Connection>,
}

pub struct DiscordWriteHalf {
    tx: WriteHalf<Connection>,
}

#[allow(dead_code)]
impl DiscordReadHalf {
    pub fn merge(self, tx: DiscordWriteHalf) -> Discord {
        Discord {
            connection: self.rx.unsplit(tx.tx),
        }
    }
}

#[allow(dead_code)]
impl DiscordWriteHalf {
    pub fn merge(self, rx: DiscordReadHalf) -> Discord {
        Discord {
            connection: rx.rx.unsplit(self.tx),
        }
    }
}

#[async_trait]
impl DiscordRead for DiscordReadHalf {
    async fn receive_data(&mut self) -> Result<Vec<u8>, Error> {
        let mut buf = [0u8; commons::BUFFER_SIZE];
        self.rx
            .read(&mut buf[..])
            .await
            .map(|length| buf[..length].to_vec())
    }

    async fn receive_data_exact(&mut self, length: usize) -> Result<Vec<u8>, Error> {
        let mut buf = [0u8; commons::BUFFER_SIZE];
        self.rx
            .read_exact(&mut buf[..length])
            .await
            .map(|length| buf[..length].to_vec())
    }
}

#[async_trait]
impl DiscordWrite for DiscordWriteHalf {
    async fn send_packet(&mut self, op_code: u32, json: String) -> Result<(), Error> {
        let mut packet = vec![0u8; 0];
        packet.put_u32_le(op_code);
        packet.put_u32_le(json.len() as u32);
        packet.put(json.as_bytes());
        self.tx.write_all(packet.as_slice()).await
    }

    async fn send_data(&mut self, data: Vec<u8>) -> Result<(), Error> {
        self.tx.write_all(data.as_slice()).await
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        self.tx.shutdown().await
    }
}

pub struct Discord {
    connection: Connection,
}

impl Discord {
    pub async fn connect() -> Result<Discord, Error> {
        for i in 0..10 {
            let path = commons::get_path(i).map_err(|err| Error::new(ErrorKind::Other, err))?;
            println!("Attempting to connect to {}", path);
            let client = Endpoint::connect(path.clone()).await;
            if client.is_ok() {
                println!("Connected to {}", path);
                return client.map(|connection| Discord { connection });
            } else {
                println!("Failed to connect to {}", path);
            }
        }
        Err(Error::new(
            ErrorKind::Other,
            "Couldn't connect to the Discord Client",
        ))
    }

    pub fn split(self) -> (DiscordReadHalf, DiscordWriteHalf) {
        let (rx, tx) = tokio::io::split(self.connection);
        (DiscordReadHalf { rx }, DiscordWriteHalf { tx })
    }
}
