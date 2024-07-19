use anyhow::Result;
use std::{
    net::Ipv4Addr,
    sync::mpsc::{Receiver, Sender},
};

use enet::{Address, BandwidthLimit, ChannelLimit, Enet, Packet, PacketMode};
use packets::{assemble_player_connect_info, assemble_player_info_request};

use crate::packets;

#[derive(Debug)]
pub enum NetworkingMessage {
    ConnectionEstablished,
}

pub fn run_networking(
    to_main: Sender<NetworkingMessage>,
    from_main: Receiver<NetworkingMessage>,
) -> Result<()> {
    let enet = Enet::new().unwrap();
    let mut host = enet
        .create_host::<()>(
            None,
            10,
            ChannelLimit::Maximum,
            BandwidthLimit::Unlimited,
            BandwidthLimit::Unlimited,
        )
        .expect("Unable to start networking host");
    host.connect(&Address::new(Ipv4Addr::LOCALHOST, 1234), 10, 0)
        .expect("Could not connect to server");

    let mut peer = loop {
        let e = host.service(1000).expect("service failed");

        let e = match e {
            Some(e) => e,
            _ => continue,
        };

        // println!("[client] event: {:#?}", e);

        match e {
            enet::Event::Connect(ref p) => {
                // println!("Connected!");
                break p.clone();
            }
            enet::Event::Disconnect(ref p, r) => {
                println!("connection NOT successful, peer: {:?}, reason: {}", p, r);
                std::process::exit(0);
            }
            enet::Event::Receive { .. } => {
                panic!("unexpected Receive-event while waiting for connection")
            }
        };
    };
    to_main.send(NetworkingMessage::ConnectionEstablished)?;

    peer.send_packet(
        Packet::new(
            &assemble_player_connect_info("ethan"),
            PacketMode::ReliableSequenced,
        )
        .unwrap(),
        1,
    )
    .expect("Sending packet failed");

    peer.send_packet(
        Packet::new(
            &&assemble_player_info_request("ethan"),
            PacketMode::ReliableSequenced,
        )
        .unwrap(),
        1,
    )
    .expect("Sending packet failed");

    loop {
        match host.service(1000) {
            Ok(e) => match e {
                Some(e) => match &e {
                    enet::Event::Connect(_) => {
                        eprintln!("Someone trying to connect with the client?")
                    }
                    enet::Event::Disconnect(_, _) => {
                        eprintln!("Disconnected while waiting for user info!");
                        break;
                    }
                    enet::Event::Receive {
                        sender: _,
                        channel_id: _,
                        packet: _,
                    } => {
                        println!("Got a message from the server!");
                    }
                },
                None => continue,
            },
            Err(_) => eprintln!("Service failed!"),
        }
    }

    Ok(())
}
