use common::packets::PacketType;

pub fn assemble_player_connect_info(username: &str) -> Vec<u8> {
    let mut packet_data = Vec::<u8>::new();

    packet_data.push(PacketType::PlayerConnect as u8);

    // username
    for c in username.as_bytes() {
        packet_data.push(*c);
    }
    packet_data.push('\0' as u8);

    packet_data
}

pub fn assemble_player_info_request(username: &str) -> Vec<u8> {
    let mut packet_data = Vec::<u8>::new();

    packet_data.push(PacketType::PlayerInfoRequest as u8);

    // username
    for c in username.as_bytes() {
        packet_data.push(*c);
    }
    packet_data.push('\0' as u8);

    packet_data
}
