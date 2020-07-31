use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use clap::Clap;
use futures::stream::StreamExt;
use parity_tokio_ipc::{Connection, Endpoint, SecurityAttributes};
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::{self, prelude::*};
use uuid::Uuid;

use commons::packet::Packet;
use commons::packet::Packet::Data;

#[allow(irrefutable_let_patterns)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts: Opts = Opts::parse();
    let addr = opts.address + ":" + opts.port.to_string().as_str();

    println!("Connecting to {}", addr);
    let (mut rx, tx) = TcpStream::connect(&addr).await?.into_split();
    println!("Successfully connected to {}", addr);

    let state = Arc::new(Mutex::new(Shared::new(tx)));

    // TCP Connection
    let tcp_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            let mut buf = [0u8; commons::BUFFER_SIZE];
            let length = rx
                .read(&mut buf[..])
                .await
                .expect("Couldn't receive data from Host");
            if length == 0 {
                continue;
            }
            if let Ok(packet) = Packet::deserialize(buf[..length].to_vec()) {
                if let Data(uuid, data) = packet {
                    if let Some(peer) = tcp_state.lock().await.peers.get_mut(&uuid) {
                        let _ = peer.write_all(data.as_slice()).await;
                    }
                }
            }
            // tcp_state.lock().await.broadcast(&buf[..length]).await;
            // println!("{:?}", data.hex_dump());
        }
    });

    // Discord
    let mut endpoint = Endpoint::new(commons::get_path(0)?);
    endpoint.set_security_attributes(SecurityAttributes::allow_everyone_create()?);

    let mut incoming = endpoint.incoming()?;
    println!("Listening for Discord IPC Calls");

    loop {
        let stream: Connection = incoming
            .next()
            .await
            .expect("Couldn't receive connection")?;

        let state = Arc::clone(&state);

        let (mut rx, tx) = tokio::io::split(stream);

        let uuid = Uuid::new_v4();
        state.lock().await.peers.insert(uuid, tx);
        let _ = state.lock().await.send_packet(Packet::Connect(uuid)).await;

        tokio::spawn(async move {
            println!("Accepted connection {}", uuid);

            let mut buf = [0u8; commons::BUFFER_SIZE];
            while let Ok(length) = rx.read(&mut buf[..]).await {
                if length == 0 {
                    break;
                }
                // println!("{}", &buf[..length].to_vec().hex_dump());
                let _ = state
                    .lock()
                    .await
                    .send_packet(Packet::Data(uuid, buf[..length].to_vec()))
                    .await;
            }

            {
                state
                    .lock()
                    .await
                    .send_packet(Packet::Disconnect(uuid))
                    .await;
                state.lock().await.peers.remove(&uuid);
                println!("Removed connection {}", uuid)
            }
        });
    }
}

struct Shared {
    tx: OwnedWriteHalf,
    peers: HashMap<Uuid, WriteHalf<Connection>>,
}

impl Shared {
    fn new(tx: OwnedWriteHalf) -> Self {
        Shared {
            tx,
            peers: HashMap::new(),
        }
    }

    async fn send_packet(&mut self, packet: Packet) {
        let _ = self
            .tx
            .write_all(packet.serialize().unwrap().as_slice())
            .await;
    }
}

/// Enables Discord Rich Presence on Linux Host for games running in a Windows Guest VM
#[derive(Clap)]
#[clap(
name = "VFIO Discord IPC Bridge Guest",
version = commons::VERSION,
author = "Adamaq01 <adamthibert01@gmail.com>"
)]
struct Opts {
    /// The address to connect to
    #[clap(short = "i", long)]
    address: String,
    /// The port to connect to
    #[clap(short, long, default_value = commons::DEFAULT_PORT_STR)]
    port: u16,
}
