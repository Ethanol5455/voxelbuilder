use cgmath::{Vector2, Vector3};
use common::Chunk;

pub struct ChunkColumn {
    chunks: Vec<Chunk>,
}

impl ChunkColumn {
    // Creates a new ChunkColumn filled with `id` (16 chunks tall)
    pub fn new(position: &Vector2<i32>, id: i32) -> ChunkColumn {
        let mut col = ChunkColumn { chunks: Vec::new() };

        for y in 0..16 {
            col.chunks
                .push(Chunk::new(Vector3::new(position.x, y, position.y), id));
        }

        col
    }

    pub fn get_chunks(&self) -> &Vec<Chunk> {
        &self.chunks
    }

    pub fn get_chunk<'a>(&'a mut self, i: u8) -> &'a mut Chunk {
        &mut self.chunks[i as usize]
    }

    pub fn set_block(&mut self, position: &Vector3<i32>, id: i32) {
        self.chunks
            .get_mut((position.y / 16) as usize)
            .unwrap()
            .set_block(
                position.x as u8,
                (position.y % 16) as u8,
                position.z as u8,
                id,
            );
    }

    pub fn set_layers(&mut self, lower: u32, upper: u32, id: i32) {
        for y in lower..(upper + 1) {
            for x in 0..16 {
                for z in 0..16 {
                    self.chunks.get_mut((y / 16) as usize).unwrap().set_block(
                        x,
                        (y % 16) as u8,
                        z,
                        id,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xyz_to_i() {
        let mut i = 0;
        for z in 0..16 {
            for y in 0..16 {
                for x in 0..16 {
                    assert_eq!(Chunk::xyz_to_i(x, y, z), i);
                    i += 1;
                }
            }
        }
    }

    #[test]
    fn test_chunk_compression() {
        let mut chunk = Chunk::new(Vector3::new(0, 0, 0), 0);
        // All air
        let set = chunk.compress();
        assert_eq!(set.len(), 1);
        assert_eq!(set[0].count, 4096);
        assert_eq!(set[0].id, 0);

        // Single block
        chunk.set_block_i(0, 1);
        let set = chunk.compress();
        assert_eq!(set.len(), 2);
        assert_eq!(set[0].count, 1);
        assert_eq!(set[0].id, 1);
        assert_eq!(set[1].count, 4095);
        assert_eq!(set[1].id, 0);

        // Alternating double
        for i in 0..4096 {
            if i % 4 == 0 || i % 4 == 1 {
                chunk.set_block_i(i, 1);
            }
        }

        let set = chunk.compress();
        assert_eq!(set.len(), 2048);
        for i in 0..2048 {
            assert_eq!(set[i].count, 2);
            if i % 2 == 1 {
                assert_eq!(set[i].id, 0);
            } else {
                assert_eq!(set[i].id, 1);
            }
        }
    }
}
