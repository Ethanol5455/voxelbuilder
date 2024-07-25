use core::str;

use crate::player_data::Player;

#[derive(Copy, Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum PacketType {
    PlayerConnect,
    PlayerDisconnect,
    PlayerInfoRequest, // Get saved player data from file (if available)
    PlayerInfoData,    // Data about a player to save, sent at a fixed interval from the client
    ChunkRequest,      // Request from the client to send data about a chunk
    ChunkUpdate,       // Request from the client to update a chunk
    ChunkContents,     // The contents of a chunk as requested by the client
                       // TODO: Add server message to client // Send a message from the server to the client
                       // TODO: Add client command to server // Send a command from the client to the server
}

impl PacketType {
    pub fn fromu8(value: u8) -> Result<PacketType, ()> {
        Ok(match value {
            0 => PacketType::PlayerConnect,
            1 => PacketType::PlayerDisconnect,
            2 => PacketType::PlayerInfoRequest,
            3 => PacketType::PlayerInfoData,
            4 => PacketType::ChunkRequest,
            5 => PacketType::ChunkUpdate,
            6 => PacketType::ChunkContents,
            _ => return Err(()),
        })
    }
}

pub enum ChunkUpdateType {
    PlaceBlockEvent,
    DestroyBlockEvent,
}

pub fn assemble_player_info_data(player: &Player) -> Vec<u8> {
    let mut packet_data = Vec::<u8>::new();
    // [0: Type][1-12: position][13-20: rotation][21-: username]

    packet_data.push(PacketType::PlayerInfoData as u8);

    // position
    let mut pos = bincode::serialize(&player.position).unwrap();
    packet_data.append(&mut pos);

    // rotation
    let mut rot = bincode::serialize(&player.rotation).unwrap();
    packet_data.append(&mut rot);

    // username
    for c in player.username.as_bytes() {
        packet_data.push(*c);
    }

    packet_data
}

pub fn parse_player_info_data(data: &[u8]) -> Player {
    assert_eq!(
        PacketType::fromu8(data[0])
            .expect("First byte of packet should be PacketType::PlayerInfoData"),
        PacketType::PlayerInfoData
    );

    // [0: Type][1-12: position][13-20: rotation][21-: username]
    let position = bincode::deserialize(&data[1..13]).unwrap();
    let rotation = bincode::deserialize(&data[13..21]).unwrap();
    let username = str::from_utf8(&data[21..(data.len() - 1)])
        .unwrap()
        .to_string();

    Player {
        username,
        position,
        rotation,
    }
}
