use cgmath::Vector3;
use items::ItemManager;
use serde::{Deserialize, Serialize};

pub mod items;
pub mod packets;

const CHUNK_WIDTH: u8 = 16;

#[derive(Serialize, Deserialize)]
pub struct CompressedSet {
    pub id: i32,
    pub count: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
}

impl QuadVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

pub struct Chunk {
    pub position: Vector3<i32>,
    blocks: [i32; 4096],
    pub vertices: Vec<QuadVertex>,
    pub indices: Vec<u32>,
}

impl Chunk {
    /// Creates a new chunk and fills it with the block of id
    pub fn new(position: Vector3<i32>, id: i32) -> Chunk {
        Chunk {
            position,
            blocks: [id; 4096],
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn xyz_to_i(x: u8, y: u8, z: u8) -> u16 {
        (CHUNK_WIDTH as u16).pow(2) * z as u16 + (CHUNK_WIDTH as u16) * y as u16 + x as u16
    }

    /// Sets the block at position `i` to `id`
    pub fn set_block_i(&mut self, i: u16, id: i32) {
        self.blocks[i as usize] = id;
    }

    /// Sets the block at position (`x`,`y`,`z`) to `id`
    pub fn set_block(&mut self, x: u8, y: u8, z: u8, id: i32) {
        self.blocks[Chunk::xyz_to_i(x, y, z) as usize] = id;
    }

    /// Gets the block at position (`x`,`y`,`z`)
    pub fn get_block(&self, x: u8, y: u8, z: u8) -> i32 {
        self.blocks[Chunk::xyz_to_i(x, y, z) as usize]
    }

    /// Compresses the chunk data using run-length encoding
    pub fn compress(&self) -> Vec<CompressedSet> {
        let mut set = Vec::<CompressedSet>::new();

        let mut number = 0;
        let mut id = -1;

        for i in 0..4096 {
            let block = self.blocks[i];
            if block == id {
                number += 1;
            } else {
                if id != -1 {
                    let new_set = CompressedSet { id, count: number };
                    set.push(new_set);
                }
                id = block;
                number = 1;
            }

            if i == 4095 && id != -1 {
                let new_set = CompressedSet { id, count: number };
                set.push(new_set);
            }
        }

        set
    }

    pub fn build_mesh(&mut self, item_manger: &ItemManager) {
        self.vertices.clear();
        self.indices.clear();

        for z in 0..CHUNK_WIDTH {
            for y in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    let block_offset = Vector3::new(x as f32, y as f32, z as f32);
                    let block_id = self.blocks[Self::xyz_to_i(x, y, z) as usize];
                    let block_info = match item_manger.get_item_by_id(block_id) {
                        Some(item_info) => item_info,
                        None => {
                            eprintln!(
                                "WARNING: Chunk::build_mesh was unable to get item info for id {}",
                                block_id
                            );
                            continue;
                        }
                    };

                    if block_info.is_transparent {
                        continue;
                    }

                    let render_right_face = if z != 15 {
                        item_manger
                            .is_transparent(self.blocks[Self::xyz_to_i(x, y, z + 1) as usize])
                            .unwrap_or(true)
                    } else {
                        true
                    };
                    let render_left_face = if z != 0 {
                        item_manger
                            .is_transparent(self.blocks[Self::xyz_to_i(x, y, z - 1) as usize])
                            .unwrap_or(true)
                    } else {
                        true
                    };
                    let render_front_face = if x != 0 {
                        item_manger
                            .is_transparent(self.blocks[Self::xyz_to_i(x - 1, y, z) as usize])
                            .unwrap_or(true)
                    } else {
                        true
                    };
                    let render_back_face = if x != 15 {
                        item_manger
                            .is_transparent(self.blocks[Self::xyz_to_i(x + 1, y, z) as usize])
                            .unwrap_or(true)
                    } else {
                        true
                    };
                    let render_bottom_face = if y != 0 {
                        item_manger
                            .is_transparent(self.blocks[Self::xyz_to_i(x, y - 1, z) as usize])
                            .unwrap_or(true)
                    } else {
                        true
                    };
                    let render_top_face = if y != 15 {
                        item_manger
                            .is_transparent(self.blocks[Self::xyz_to_i(x, y + 1, z) as usize])
                            .unwrap_or(true)
                    } else {
                        true
                    };

                    // Right Face
                    if render_right_face {
                        // Bottom Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 0.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.side_tex_coords.bl.into(),
                        });
                        let bl_index = self.vertices.len() as u32 - 1;
                        // Bottom Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 0.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.side_tex_coords.br.into(),
                        });
                        let br_index = self.vertices.len() as u32 - 1;
                        // Top Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 1.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.side_tex_coords.tr.into(),
                        });
                        let tr_index = self.vertices.len() as u32 - 1;
                        // Top Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 1.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.side_tex_coords.tl.into(),
                        });
                        let tl_index = self.vertices.len() as u32 - 1;

                        self.indices.push(bl_index);
                        self.indices.push(br_index);
                        self.indices.push(tl_index);

                        self.indices.push(tr_index);
                        self.indices.push(tl_index);
                        self.indices.push(br_index);
                    }
                    // Left Face
                    if render_left_face {
                        // Bottom Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 0.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.side_tex_coords.bl.into(),
                        });
                        let bl_index = self.vertices.len() as u32 - 1;
                        // Bottom Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 0.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.side_tex_coords.br.into(),
                        });
                        let br_index = self.vertices.len() as u32 - 1;
                        // Top Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 1.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.side_tex_coords.tr.into(),
                        });
                        let tr_index = self.vertices.len() as u32 - 1;
                        // Top Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 1.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.side_tex_coords.tl.into(),
                        });
                        let tl_index = self.vertices.len() as u32 - 1;

                        self.indices.push(tl_index);
                        self.indices.push(br_index);
                        self.indices.push(bl_index);

                        self.indices.push(br_index);
                        self.indices.push(tl_index);
                        self.indices.push(tr_index);
                    }
                    // Front Face
                    if render_front_face {
                        // Bottom Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 0.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.side_tex_coords.bl.into(),
                        });
                        let bl_index = self.vertices.len() as u32 - 1;
                        // Bottom Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 0.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.side_tex_coords.br.into(),
                        });
                        let br_index = self.vertices.len() as u32 - 1;
                        // Top Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 1.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.side_tex_coords.tr.into(),
                        });
                        let tr_index = self.vertices.len() as u32 - 1;
                        // Top Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 1.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.side_tex_coords.tl.into(),
                        });
                        let tl_index = self.vertices.len() as u32 - 1;

                        self.indices.push(bl_index);
                        self.indices.push(br_index);
                        self.indices.push(tl_index);

                        self.indices.push(tr_index);
                        self.indices.push(tl_index);
                        self.indices.push(br_index);
                    }
                    // Back Face
                    if render_back_face {
                        // Bottom Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 0.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.side_tex_coords.bl.into(),
                        });
                        let bl_index = self.vertices.len() as u32 - 1;
                        // Bottom Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 0.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.side_tex_coords.br.into(),
                        });
                        let br_index = self.vertices.len() as u32 - 1;
                        // Top Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 1.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.side_tex_coords.tr.into(),
                        });
                        let tr_index = self.vertices.len() as u32 - 1;
                        // Top Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 1.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.side_tex_coords.tl.into(),
                        });
                        let tl_index = self.vertices.len() as u32 - 1;

                        self.indices.push(tl_index);
                        self.indices.push(br_index);
                        self.indices.push(bl_index);

                        self.indices.push(br_index);
                        self.indices.push(tl_index);
                        self.indices.push(tr_index);
                    }
                    // Bottom Face
                    if render_bottom_face {
                        // Bottom Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 0.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.bottom_tex_coords.bl.into(),
                        });
                        let bl_index = self.vertices.len() as u32 - 1;
                        // Bottom Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 0.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.bottom_tex_coords.br.into(),
                        });
                        let br_index = self.vertices.len() as u32 - 1;
                        // Top Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 0.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.bottom_tex_coords.tr.into(),
                        });
                        let tr_index = self.vertices.len() as u32 - 1;
                        // Top Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 0.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.bottom_tex_coords.tl.into(),
                        });
                        let tl_index = self.vertices.len() as u32 - 1;

                        self.indices.push(tl_index);
                        self.indices.push(br_index);
                        self.indices.push(bl_index);

                        self.indices.push(br_index);
                        self.indices.push(tl_index);
                        self.indices.push(tr_index);
                    }
                    // Top Face
                    if render_top_face {
                        // Bottom Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 1.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.top_tex_coords.bl.into(),
                        });
                        let bl_index = self.vertices.len() as u32 - 1;
                        // Bottom Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 1.0,
                                block_offset.z + 1.0,
                            ],
                            tex_coords: block_info.top_tex_coords.br.into(),
                        });
                        let br_index = self.vertices.len() as u32 - 1;
                        // Top Right
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 1.0,
                                block_offset.y + 1.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.top_tex_coords.tr.into(),
                        });
                        let tr_index = self.vertices.len() as u32 - 1;
                        // Top Left
                        self.vertices.push(QuadVertex {
                            position: [
                                block_offset.x + 0.0,
                                block_offset.y + 1.0,
                                block_offset.z + 0.0,
                            ],
                            tex_coords: block_info.top_tex_coords.tl.into(),
                        });
                        let tl_index = self.vertices.len() as u32 - 1;

                        self.indices.push(bl_index);
                        self.indices.push(br_index);
                        self.indices.push(tl_index);

                        self.indices.push(tr_index);
                        self.indices.push(tl_index);
                        self.indices.push(br_index);
                    }
                }
            }
        }
    }
}
