use crate::world::ChunkColumn;
use common::packets::PacketType;

pub fn assemble_chunk_contents_packet(col: &mut ChunkColumn) -> Vec<u8> {
    let mut packet_data = Vec::<u8>::new();

    packet_data.push(PacketType::ChunkContents as u8);

    let chunks = col.get_chunks();
    for chunk in chunks {
        let mut pos = bincode::serialize(&chunk.position).unwrap();
        packet_data.append(&mut pos);

        let compressed_data = chunk.compress();

        for set in compressed_data {
            let mut id = bincode::serialize(&set.id).unwrap();
            packet_data.append(&mut id);
            let mut count = bincode::serialize(&set.count).unwrap();
            packet_data.append(&mut count);
        }

        let mut end_indicator = bincode::serialize(&(-1)).unwrap();
        packet_data.append(&mut end_indicator);
    }

    packet_data
}
