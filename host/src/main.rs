extern crate interfaces;

use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

use clap::Clap;
use futures::future::{AbortHandle, Abortable};
use interfaces::{Interface, Kind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use uuid::Uuid;

use commons::packet::Packet;

use crate::discord::{Discord, DiscordRead, DiscordWrite, DiscordWriteHalf};

mod discord;

#[allow(irrefutable_let_patterns)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts: Opts = Opts::parse();
    let addr = opts
        .interface
        .map(|interface| {
            get_address(
                &Interface::get_by_name(interface.as_str())
                    .expect("Couldn't get interface")
                    .expect("Couldn't get interface"),
            )
            .expect("Couldn't get interface address")
        })
        .unwrap_or_else(|| "0.0.0.0".to_string())
        + ":"
        + opts.port.to_string().as_str();

    let state = Arc::new(Mutex::new(Shared::new()));

    // TCP Connections
    let mut listener = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);
    loop {
        let (stream, addr) = listener.accept().await?;

        let state = Arc::clone(&state);

        let (mut rx, tx) = stream.into_split();

        let peer = Peer::new(addr, tx);
        state.lock().await.peers.insert(addr, peer);

        tokio::spawn(async move {
            println!("Accepted connection {}", addr);

            let mut buf = [0u8; commons::BUFFER_SIZE];
            while let Ok(length) = rx.read(&mut buf[..]).await {
                if length == 0 {
                    break;
                }
                // println!("{}", &buf[..length].to_vec().hex_dump());
                let packet = Packet::deserialize(buf[..length].to_vec()).unwrap();
                match packet {
                    Packet::Connect(uuid) => {
                        let (mut discord_rx, discord_tx) =
                            Discord::connect().await.unwrap().split();
                        let (abort_handle, abort_registration) = AbortHandle::new_pair();
                        state.lock().await[addr]
                            .abort_handles
                            .insert(uuid, abort_handle);
                        state.lock().await[addr].discords.insert(uuid, discord_tx);
                        let discord_state = Arc::clone(&state);
                        let _ = tokio::spawn(Abortable::new(
                            async move {
                                loop {
                                    let data = discord_rx.receive_data().await;
                                    if let Ok(data) = data {
                                        let _ = discord_state.lock().await[addr]
                                            .tx
                                            .write_all(
                                                Packet::Data(uuid, data)
                                                    .serialize()
                                                    .unwrap()
                                                    .as_slice(),
                                            )
                                            .await;
                                    } else {
                                        return;
                                    }
                                }
                            },
                            abort_registration,
                        ));
                    }
                    Packet::Data(uuid, data) => {
                        let _ = state.lock().await[addr]
                            .discords
                            .get_mut(&uuid)
                            .unwrap()
                            .send_data(data)
                            .await;
                    }
                    Packet::Disconnect(uuid) => {
                        let _ = state.lock().await[addr]
                            .abort_handles
                            .remove(&uuid)
                            .unwrap()
                            .abort();
                        let _ = state.lock().await[addr]
                            .discords
                            .remove(&uuid)
                            .unwrap()
                            .shutdown()
                            .await;
                    }
                }
            }

            {
                state.lock().await.peers.remove(&addr);
                println!("Removed connection {}", addr)
            }
        });
    }
}

pub fn get_address(interface: &Interface) -> Option<String> {
    for addr in &interface.addresses {
        if addr.kind == Kind::Ipv4 && addr.addr.is_some() {
            return Some(addr.addr?.ip().to_string());
        }
    }
    None
}

struct Peer {
    address: SocketAddr,
    tx: OwnedWriteHalf,
    abort_handles: HashMap<Uuid, AbortHandle>,
    discords: HashMap<Uuid, DiscordWriteHalf>,
}

impl Peer {
    fn new(address: SocketAddr, tx: OwnedWriteHalf) -> Peer {
        Peer {
            address,
            tx,
            abort_handles: HashMap::new(),
            discords: HashMap::new(),
        }
    }
}

struct Shared {
    peers: HashMap<SocketAddr, Peer>,
}

impl Shared {
    fn new() -> Self {
        Shared {
            peers: HashMap::new(),
        }
    }
}

impl Index<SocketAddr> for Shared {
    type Output = Peer;

    fn index(&self, index: SocketAddr) -> &Self::Output {
        self.peers.get(&index).unwrap()
    }
}

impl IndexMut<SocketAddr> for Shared {
    fn index_mut(&mut self, index: SocketAddr) -> &mut Self::Output {
        self.peers.get_mut(&index).unwrap()
    }
}

/// Enables Discord Rich Presence on Linux Host for games running in a Windows Guest VM
#[derive(Clap)]
#[clap(
name = "VFIO Discord IPC Bridge Host",
version = commons::VERSION,
author = "Adamaq01 <adamthibert01@gmail.com>"
)]
struct Opts {
    /// The network interface to use
    #[clap(short, long)]
    interface: Option<String>,
    /// The port to listen to
    #[clap(short, long, default_value = commons::DEFAULT_PORT_STR)]
    port: u16,
}
