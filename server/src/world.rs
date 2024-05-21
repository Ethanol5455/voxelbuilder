pub mod chunk_column;
use cgmath::{Vector2, Vector3};
pub use chunk_column::ChunkColumn;
use common::items::ItemManager;

use fast_noise_lite_rs::{FastNoiseLite, NoiseType};
use rlua::Lua;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;

use crate::save_file::SaveFile;

#[derive(Serialize, Deserialize)]
pub struct BlockToPlace {
    pub column_position: Vector2<i32>,
    pub position_in_column: Vector3<i32>,
    pub block_id: i32,
}

pub struct World {
    save_file: SaveFile,
    column_map: BTreeMap<i32, BTreeMap<i32, ChunkColumn>>,
    item_manager: ItemManager,
    lua: Lua,
    column_script: String,
    noise_functions: HashMap<String, FastNoiseLite>,
}

impl World {
    /// Creates a new world with no chunks
    pub fn new(item_manager: ItemManager, save: SaveFile) -> World {
        let seed = save.world_seed;
        let mut noise_functions = HashMap::new();

        let mut noise = FastNoiseLite::new(seed);
        noise.set_noise_type(NoiseType::OpenSimplex2);
        noise_functions.insert("OpenSimplex2".to_string(), noise);
        let mut noise = FastNoiseLite::new(seed);
        noise.set_noise_type(NoiseType::OpenSimplex2S);
        noise_functions.insert("OpenSimplex2S".to_string(), noise);
        let mut noise = FastNoiseLite::new(seed);
        noise.set_noise_type(NoiseType::Cellular);
        noise_functions.insert("Cellular".to_string(), noise);
        let mut noise = FastNoiseLite::new(seed);
        noise.set_noise_type(NoiseType::Perlin);
        noise_functions.insert("Perlin".to_string(), noise);
        let mut noise = FastNoiseLite::new(seed);
        noise.set_noise_type(NoiseType::ValueCubic);
        noise_functions.insert("ValueCubic".to_string(), noise);
        let mut noise = FastNoiseLite::new(seed);
        noise.set_noise_type(NoiseType::Value);
        noise_functions.insert("Value".to_string(), noise);

        let column_script_path = save.get_script_path("generateChunkColumn".to_string());

        World {
            save_file: save,
            column_map: BTreeMap::new(),
            item_manager,
            lua: Lua::new(),
            column_script: fs::read_to_string(column_script_path)
                .expect("Unable to load generateChunkColumn script"),
            noise_functions,
        }
    }

    /// Generates a new column at the given position (`x`,`y`)
    fn generate_column(&mut self, pos: &Vector2<i32>) {
        let mut col = ChunkColumn::new(pos, 0);
        let col_ptr = &mut col as *mut ChunkColumn;

        let mut was_saved = true;

        // For each chunk in column
        for height in 0..16 as u8 {
            let saved_chunk =
                self.save_file
                    .get_chunk(Vector3::<i32>::new(pos.x, height as i32, pos.y));
            match saved_chunk {
                Some(chunk_data) => {
                    let chunk = col.get_chunk(height as u8);
                    let mut i = 0;
                    for set in chunk_data.data.as_slice() {
                        for _ in 0..set.count {
                            chunk.set_block_i(i, set.id);
                            i += 1;
                        }
                    }
                }
                None => {
                    was_saved = false;
                }
            }
        }

        struct ToPlaceAfter {
            position: Vector3<i32>,
            id: i32,
        }

        let mut set_world_after_list = Vec::<ToPlaceAfter>::new();
        let set_world_after_list_ptr = &mut set_world_after_list as *mut Vec<ToPlaceAfter>;

        if !was_saved {
            self.lua.context(|lua_ctx| {
                let globals = lua_ctx.globals(); // Get globals from lua

                lua_ctx.scope(|scope| {
                    lua_ctx
                        .load(&format!(
                            r#"
                            column_x = {}
                            column_z = {}
                        "#,
                            pos.x, pos.y
                        ))
                        .set_name("Generate column variables")
                        .unwrap()
                        .exec()
                        .expect("Generate column variables failed to load");

                    let rust_random = scope
                        .create_function(|_, (): ()| Ok(rand::random::<i32>()))
                        .unwrap();
                    globals.set("random", rust_random).unwrap();

                    let get_id_by_name = scope
                        .create_function_mut(|_, item_name: String| {
                            let id = self.item_manager.get_id_by_name(item_name);
                            Ok(id)
                        })
                        .unwrap();
                    globals.set("get_id_by_name", get_id_by_name).unwrap();
                    let get_noise_2d = scope
                        .create_function(|_, (noise_type, x, y): (String, f32, f32)| {
                            let noise = self.noise_functions.get(noise_type.as_str()).expect(
                                format!("Noise function {} does not exist", noise_type.as_str())
                                    .as_str(),
                            );
                            let noise_val = noise.get_noise_2d(x, y);
                            Ok(noise_val)
                        })
                        .unwrap();
                    globals.set("get_noise_2d", get_noise_2d).unwrap();

                    let set_block = scope
                        .create_function(|_, (x, y, z, id): (i32, i32, i32, i32)| {
                            if x >= 0 && x < 16 && y >= 0 && y < 16 {
                                unsafe {
                                    (*col_ptr).set_block(&Vector3::new(x, y, z), id);
                                }
                            } else {
                                let to_place = ToPlaceAfter {
                                    position: Vector3::new(pos.x * 16 + x, y, pos.y * 16 + z),
                                    id,
                                };
                                unsafe {
                                    (*set_world_after_list_ptr).push(to_place);
                                }
                            }
                            Ok(())
                        })
                        .unwrap();
                    globals.set("set_block", set_block).unwrap();

                    let set_layers = scope
                        .create_function(|_, (lower, upper, id): (u32, u32, i32)| {
                            unsafe {
                                (*col_ptr).set_layers(lower, upper, id);
                            }
                            Ok(())
                        })
                        .unwrap();
                    globals.set("set_layers", set_layers).unwrap();

                    lua_ctx
                        .load(&self.column_script)
                        .set_name("Generate Chunk Column")
                        .unwrap()
                        .exec()
                        .expect("Lua chunk generation script failed!");
                });
            });
        }

        // Insert new col into map
        if !self.column_map.contains_key(&pos.x) {
            self.column_map.insert(pos.x, BTreeMap::new());
        }

        self.column_map.get_mut(&pos.x).unwrap().insert(pos.y, col);

        for to_place in set_world_after_list.as_slice() {
            self.set_block(&to_place.position, to_place.id);
        }
    }

    /// Returns whether the column at `pos` exists
    pub fn does_column_exist(&self, pos: &Vector2<i32>) -> bool {
        self.column_map.contains_key(&pos.x)
            && self.column_map.get(&pos.x).unwrap().contains_key(&pos.y)
    }

    /// Gets the column at `pos` and generates the column if it doesn't exist
    pub fn get_column(&mut self, pos: &Vector2<i32>) -> &mut ChunkColumn {
        if !self.does_column_exist(pos) {
            self.generate_column(pos);
        }

        self.column_map
            .get_mut(&pos.x)
            .unwrap()
            .get_mut(&pos.y)
            .unwrap()
    }

    /// Translates absolute world position to absolute column position
    pub fn world_to_column_position(pos: &Vector2<i32>) -> Vector2<i32> {
        let mut column_position = Vector2::new(pos.x / 16, pos.y / 16);

        if pos.x < 0 && -pos.x % 16 != 0 {
            column_position.x -= 1;
        }
        if pos.y < 0 && -pos.y % 16 != 0 {
            column_position.y -= 1;
        }

        column_position
    }

    /// Translates absolute world position to absolute chunk position
    pub fn world_to_chunk_position(pos: &Vector3<i32>) -> Vector3<i32> {
        let mut chunk_position = Vector3::new(pos.x / 16, pos.y / 16, pos.z / 16);

        if pos.x < 0 && -pos.x % 16 != 0 {
            chunk_position.x -= 1;
        }
        if pos.y < 0 && -pos.y % 16 != 0 {
            chunk_position.y -= 1;
        }
        if pos.z < 0 && -pos.z % 16 != 0 {
            chunk_position.z -= 1;
        }

        chunk_position
    }

    /// Translates absolute world position into internal chunk position
    pub fn world_to_position_in_chunk(pos: &Vector3<i32>) -> Vector3<i32> {
        let mut block_pos_in_chunk = Vector3::new(pos.x % 16, pos.y % 16, pos.z % 16);

        if block_pos_in_chunk.x < 0 {
            block_pos_in_chunk.x += 16;
        }
        if block_pos_in_chunk.y < 0 {
            block_pos_in_chunk.y += 16;
        }
        if block_pos_in_chunk.z < 0 {
            block_pos_in_chunk.z += 16;
        }

        block_pos_in_chunk
    }

    /// Gets the block at `pos`
    pub fn get_block(&mut self, position: &Vector3<i32>) -> i32 {
        let chunk_position = World::world_to_chunk_position(position);
        let block_position_in_chunk = World::world_to_position_in_chunk(position);

        let column = self.get_column(&Vector2::new(chunk_position.x, chunk_position.z));
        if !(chunk_position.y >= 0 && chunk_position.y <= 15) {
            return -1;
        }

        column.get_chunk(chunk_position.y as u8).get_block(
            block_position_in_chunk.x as u8,
            block_position_in_chunk.y as u8,
            block_position_in_chunk.z as u8,
        )
    }

    /// Sets the block at `pos` to `id`
    pub fn set_block(&mut self, position: &Vector3<i32>, id: i32) {
        if self.item_manager.get_item_by_id(id).is_none() {
            println!("Tried to set block of unknown id {}", id);
            return;
        }

        let chunk_position = World::world_to_chunk_position(position);
        let block_position_in_chunk = World::world_to_position_in_chunk(position);

        if !self.does_column_exist(&Vector2::new(chunk_position.x, chunk_position.z)) {
            self.generate_column(&Vector2::new(chunk_position.x, chunk_position.z));
        }

        self.get_column(&Vector2::new(chunk_position.x, chunk_position.z))
            .get_chunk(chunk_position.y as u8)
            .set_block(
                block_position_in_chunk.x as u8,
                block_position_in_chunk.y as u8,
                block_position_in_chunk.z as u8,
                id,
            );
    }

    pub fn get_save_file(&mut self) -> &mut SaveFile {
        &mut self.save_file
    }

    pub fn save_to_file(&mut self) {
        if self.save_file.save_directory.is_none() {
            return;
        }

        println!("Saving world data");
        for column_x in self.column_map.values() {
            for column_z in column_x.values() {
                for chunk in column_z.get_chunks() {
                    self.save_file.save_chunk_data(chunk);
                }
            }
        }

        // TODO: Save block_to_place

        println!("Writing save file");
        match self.save_file.write_save() {
            Ok(_) => println!("Save file written"),
            Err(e) => eprintln!("Save file NOT written with error {}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_to_column_position() {
        // Positive
        for x in 0..48 {
            for z in 0..48 {
                assert_eq!(
                    World::world_to_column_position(&Vector2::new(x, z)),
                    Vector2::new(x / 16, z / 16)
                );
            }
        }

        // Negative
        for x in -49..0 {
            for z in -49..0 {
                let mut correct_x = x / 16;
                if x < 0 && -x % 16 != 0 {
                    correct_x -= 1;
                }
                let mut correct_z = z / 16;
                if z < 0 && -z % 16 != 0 {
                    correct_z -= 1;
                }
                assert_eq!(
                    World::world_to_column_position(&Vector2::new(x, z)),
                    Vector2::new(correct_x, correct_z)
                );
            }
        }
    }

    #[test]
    fn test_world_to_chunk_position() {
        // Positive
        for x in 0..48 {
            for y in 0..48 {
                for z in 0..48 {
                    assert_eq!(
                        World::world_to_chunk_position(&Vector3::new(x, y, z)),
                        Vector3::new(x / 16, y / 16, z / 16)
                    );
                }
            }
        }

        // Negative
        for x in -49..0 {
            for y in -49..0 {
                for z in -49..0 {
                    let mut correct_x = x / 16;
                    if x < 0 && -x % 16 != 0 {
                        correct_x -= 1;
                    }
                    let mut correct_y = y / 16;
                    if y < 0 && -y % 16 != 0 {
                        correct_y -= 1;
                    }
                    let mut correct_z = z / 16;
                    if z < 0 && -z % 16 != 0 {
                        correct_z -= 1;
                    }
                    assert_eq!(
                        World::world_to_chunk_position(&Vector3::new(x, y, z)),
                        Vector3::new(correct_x, correct_y, correct_z)
                    );
                }
            }
        }
    }

    #[test]
    fn test_world_to_position_in_chunk() {
        // Chunk at 0,0,0
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    assert_eq!(
                        World::world_to_position_in_chunk(&Vector3::new(x, y, z)),
                        Vector3::new(x, y, z)
                    );
                }
            }
        }

        // Chunk at 16,16,16
        for x in 16..32 {
            for y in 16..32 {
                for z in 16..32 {
                    assert_eq!(
                        World::world_to_position_in_chunk(&Vector3::new(x, y, z)),
                        Vector3::new(x - 16, y - 16, z - 16)
                    );
                }
            }
        }

        // Chunk at -15,-15,-15
        for x in -16..0 {
            for y in -16..0 {
                for z in -16..0 {
                    assert_eq!(
                        World::world_to_position_in_chunk(&Vector3::new(x, y, z)),
                        Vector3::new(x + 16, y + 16, z + 16)
                    );
                }
            }
        }
    }
}
