use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::{fs, io};

use anyhow::Result;
use cgmath::{Vector2, Vector3};

use crate::player_data::Player;

use crate::world::BlockToPlace;
use common::{Chunk, CompressedSet};

const DEFAULT_SCRIPT_SUBDIRECTORY: &str = "/default_scripts";
const SAVE_FILE_NAME: &str = "worldData";
const SAVE_FILE_EXTENSION: &str = "vbdat";
const PLAYER_SAVE_SUBDIRECTORY: &str = "/players";
const SCRIPT_SAVE_SUBDIRECTORY: &str = "/scripts";

pub struct ChunkInfo {
    pub position: Vector3<i32>,
    pub data: Vec<CompressedSet>,
}

pub struct SaveFile {
    // save_directory does not contain trailing slashes, if None do not save
    pub save_directory: Option<String>,
    pub world_seed: i32,
    chunk_data: Vec<ChunkInfo>,
    block_to_place: Vec<BlockToPlace>,
    players: HashMap<String, Player>,
}

impl SaveFile {
    pub fn new(directory: Option<String>) -> SaveFile {
        match directory.clone() {
            Some(dir) => {
                assert!(!dir.is_empty(), "Empty save directory entered!");
                assert!(
                    !dir.contains('\\'),
                    "Save directory may not contain \\ characters"
                );
                assert!(
                    !dir.ends_with('/'),
                    "Save directory may not end with / character"
                );
                match SaveFile::generate_save_structure(dir) {
                    Ok(_) => (),
                    Err(e) => panic!("Unable to generate save directory structure: {}", e),
                }
            }
            None => (),
        }

        SaveFile {
            save_directory: directory,
            world_seed: rand::random(),
            chunk_data: Vec::<ChunkInfo>::new(),
            block_to_place: Vec::<BlockToPlace>::new(),
            players: HashMap::new(),
        }
    }

    fn generate_save_structure(directory: String) -> io::Result<()> {
        fs::create_dir_all(format!("{}{}", directory, SCRIPT_SAVE_SUBDIRECTORY))?;
        fs::create_dir_all(format!("{}{}", directory, PLAYER_SAVE_SUBDIRECTORY))?;

        let script_files = ["loadAssetInfo.lua", "generateChunkColumn.lua"];

        for script_file in script_files {
            let to_path_str = format!("{}{}/{}", directory, SCRIPT_SAVE_SUBDIRECTORY, script_file);
            if !Path::new(&to_path_str).exists() {
                fs::copy(
                    format!(".{}/{}", DEFAULT_SCRIPT_SUBDIRECTORY, script_file),
                    to_path_str,
                )?;
            }
        }

        Ok(())
    }

    pub fn get_script_path(&self, script_name: String) -> String {
        match self.save_directory.clone() {
            Some(directory) => format!(
                "{}{}/{}.lua",
                directory, SCRIPT_SAVE_SUBDIRECTORY, script_name
            ),
            None => format!(".{}/{}.lua", DEFAULT_SCRIPT_SUBDIRECTORY, script_name),
        }
    }

    pub fn get_chunk(&self, position: Vector3<i32>) -> Option<&ChunkInfo> {
        for chunk in self.chunk_data.as_slice() {
            if chunk.position == position {
                return Some(chunk);
            }
        }

        None
    }

    pub fn get_user_data(&mut self, username: &String) -> &mut Player {
        if self.players.contains_key(username) {
            return self.players.get_mut(username).unwrap();
        }

        let player = Player {
            username: username.to_string(),
            position: Vector3::new(0.0, 80.0, 0.0),
            rotation: Vector2::new(0.0, 0.0),
        };
        self.players.insert(username.to_string(), player);

        self.players.get_mut(username).unwrap()
    }

    pub fn save_chunk_data(&mut self, chunk: &Chunk) {
        let data = ChunkInfo {
            position: chunk.position,
            data: chunk.compress(),
        };

        let mut remove_index = usize::MAX;
        for i in 0..self.chunk_data.len() {
            if self.chunk_data[i].position == chunk.position {
                remove_index = i;
            }
        }

        if remove_index != usize::MAX {
            self.chunk_data.remove(remove_index);
        }

        self.chunk_data.push(data)
    }

    pub fn write_save(&self) -> Result<()> {
        if self.save_directory.is_none() {
            eprintln!("Save directory not provided, save will not be written");
            return Err(anyhow::Error::new(std::io::Error::new(
                io::ErrorKind::InvalidInput,
                "No save directory given!",
            )));
        }
        let directory_str = self.save_directory.clone().unwrap();

        // Player data
        for (username, player) in &self.players {
            let mut file = match File::create(format!(
                "{}{}/{}.{}",
                directory_str, PLAYER_SAVE_SUBDIRECTORY, username, SAVE_FILE_EXTENSION
            )) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!(
                        "Unable to write save file for player \"{}\" with error \"{}\"",
                        username, e
                    );
                    continue;
                }
            };
            let binary_player = bincode::serialize(&player)?;
            file.write(&binary_player)?;
        }

        // World data
        let mut file = match File::create(format!(
            "{}/{}.{}",
            directory_str, SAVE_FILE_NAME, SAVE_FILE_EXTENSION
        )) {
            Ok(file) => file,
            Err(e) => return Err(anyhow::Error::new(e)),
        };

        // World seed
        file.write(&bincode::serialize(&self.world_seed)?)?;

        // Compressed chunk data
        for chunk in &self.chunk_data {
            file.write(&[b'C'])?;
            // Chunk Position
            file.write(&bincode::serialize(&chunk.position)?)?;
            file.write(&bincode::serialize(&(chunk.data.len() as u32))?)?;
            for set in &chunk.data {
                file.write(&bincode::serialize(&set)?)?;
            }
        }

        // Blocks to place
        for block in &self.block_to_place {
            file.write(&[b'N'])?;
            file.write(&bincode::serialize(&block)?)?;
        }

        Ok(())
    }

    pub fn load(&mut self) -> Result<()> {
        // Load saved users
        assert!(self.save_directory.is_some(), "Cannot load temporary save!");
        let directory_str = self.save_directory.clone().unwrap();

        match fs::read_dir(format!("{}{}", directory_str, PLAYER_SAVE_SUBDIRECTORY)) {
            Ok(contents) => {
                for entry in contents {
                    let path = entry?.path();
                    let mut in_file = match File::open(path.clone()) {
                        Ok(file) => file,
                        Err(e) => {
                            eprintln!(
                                "Unable to write save file \"{:?}\" with error \"{}\"",
                                path, e
                            );
                            continue;
                        }
                    };

                    let metadata = fs::metadata(&path).expect("unable to read metadata");
                    let mut buffer = vec![0; metadata.len() as usize];
                    in_file.read(&mut buffer).expect("buffer overflow");

                    let new_player: Player = bincode::deserialize(&buffer)?;
                    self.players.insert(new_player.username.clone(), new_player);
                }
            }
            Err(e) => eprintln!("Unable to open player save files with error \"{}\".", e),
        }

        // Load world
        let file = match File::open(format!(
            "{}/{}.{}",
            directory_str, SAVE_FILE_NAME, SAVE_FILE_EXTENSION
        )) {
            Ok(file) => file,
            Err(e) => return Err(e.into()),
        };

        let mut reader = BufReader::new(file);

        let mut buffer: [u8; 4] = [0; 4];
        reader.read_exact(&mut buffer)?;
        self.world_seed = bincode::deserialize(&buffer)?;

        loop {
            let mut buffer: [u8; 1] = [0; 1];
            if reader.read(&mut buffer)? == 0 {
                break;
            }
            if buffer[0] == b'C' {
                let mut buffer: [u8; 12 + 4] = [0; 12 + 4];
                reader.read_exact(&mut buffer)?;

                let position: Vector3<i32> = bincode::deserialize(&buffer[..12])?;
                let mut new_chunk = ChunkInfo {
                    position: position,
                    data: Vec::new(),
                };

                let num_sets: u32 = bincode::deserialize(&buffer[12..])?;
                new_chunk.data.reserve(num_sets as usize);

                for _ in 0..num_sets {
                    let mut buffer: [u8; 8] = [0; 8];
                    reader.read_exact(&mut buffer)?;

                    new_chunk.data.push(bincode::deserialize(&buffer)?);
                }

                self.chunk_data.push(new_chunk);
            } else if buffer[0] == b'N' {
                let mut buffer: [u8; 24] = [0; 24];
                reader.read_exact(&mut buffer)?;

                self.block_to_place.push(bincode::deserialize(&buffer)?);
            } else {
                panic!("Unknown save data type {}", buffer[0]);
            }
        }

        println!("Done Reading Save!");

        Ok(())
    }
}
