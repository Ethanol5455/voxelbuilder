mod player_data;

mod save_file;
use cgmath::Vector2;
use rlua::Lua;
use save_file::SaveFile;

use anyhow::Result;

use enet::*;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, fs, str};

mod packets;
use common::{
    items::{ItemInfo, ItemManager, ItemType, TextureCoordinates},
    packets::{ChunkUpdateType, PacketType},
};

use crate::packets::*;

mod world;
use world::World;

struct GameOptions {
    init_only: bool,
}

impl GameOptions {
    pub fn new() -> Self {
        GameOptions { init_only: false }
    }

    pub fn parse_cli(mut self) -> Self {
        let args: Vec<String> = env::args().collect();

        self.init_only = args.contains(&"--no_run".to_string());

        self
    }
}

struct Game {
    options: GameOptions,

    server: Host<()>,

    world: World,
}

impl Game {
    pub fn new() -> Result<Self> {
        let options = GameOptions::new().parse_cli();
        let enet = Enet::new().unwrap();
        let address = Address::new(Ipv4Addr::UNSPECIFIED, 1234);
        let server = enet
            .create_host::<()>(
                Some(&address),
                1,
                ChannelLimit::Maximum,
                BandwidthLimit::Unlimited,
                BandwidthLimit::Unlimited,
            )
            .unwrap();

        let save_directory = "./save";
        let mut save = SaveFile::new(Some(save_directory.to_owned()));
        if save.load().is_err() {
            eprintln!("Save file could not be loaded with error \"{}\". The save file may not be generated yet!", save.load().unwrap_err());
        }

        let mut item_manager = ItemManager::new();
        load_items(
            &mut item_manager,
            save.get_script_path("loadAssetInfo".to_string()),
        );

        let world = World::new(item_manager, save);

        Ok(Game {
            options,
            server,
            world,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        if self.options.init_only {
            return Ok(());
        }
        println!("Running...");

        let term = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term)).unwrap();

        while !term.load(Ordering::Relaxed) {
            match self.server.service(1000).unwrap() {
                Some(Event::Connect(_)) => println!("Connected!"),
                Some(Event::Disconnect(..)) => {
                    println!("Disconnected!");
                }
                Some(Event::Receive {
                    ref mut sender,
                    channel_id,
                    ref packet,
                    ..
                }) => {
                    let data = packet.data();
                    let packet_type = match PacketType::fromu8(data[0]) {
                        Ok(p) => p,
                        Err(_) => {
                            eprintln!("Unknown packet type with id {}", data[0]);
                            continue;
                        }
                    };
                    match packet_type {
                        PacketType::PlayerConnect => {
                            let username = str::from_utf8(&data[1..(data.len() - 1)]).unwrap();
                            println!("Player {username} connected!");
                        }
                        PacketType::PlayerDisconnect => {
                            let username = str::from_utf8(&data[1..(data.len() - 1)]).unwrap();
                            println!("Player {username} has left.");
                        }
                        PacketType::PlayerInfoRequest => {
                            // [0: Type][1-(n-1): username][n: '\0']
                        let username = str::from_utf8(&data[1..(data.len() - 1)]).unwrap();
                        let player = self
                            .world
                            .get_save_file()
                            .get_user_data(&username.to_string());

                        let packet_data = assemble_player_info_data(&player);
                        let packet =
                            Packet::new(&packet_data, PacketMode::ReliableSequenced).unwrap();
                        sender.send_packet(packet, channel_id).unwrap();
                        }
                        PacketType::PlayerInfoData => {
                            // [0: Type][1-12: position][13-20: rotation][21-: username]
                            let username = str::from_utf8(&data[21..(data.len() - 1)]).unwrap();
                            let position = bincode::deserialize(&data[1..13]).unwrap();
                            let rotation = bincode::deserialize(&data[13..21]).unwrap();

                            let player = self
                                .world
                                .get_save_file()
                                .get_user_data(&username.to_string());
                            player.position = position;
                            player.rotation = rotation;
                        }
                        PacketType::ChunkRequest => {
                            // [0: Type][1-4: column X][5-8: column Z]
                        let col_x = bincode::deserialize(&data[1..5]).unwrap();
                        let col_z = bincode::deserialize(&data[5..9]).unwrap();
                        let col = self.world.get_column(&Vector2::new(col_x, col_z));

                        let packet_data = assemble_chunk_contents_packet(col);
                        let packet =
                            Packet::new(&packet_data, PacketMode::ReliableSequenced).unwrap();
                        sender.send_packet(packet, channel_id).unwrap();
                        }
                        PacketType::ChunkUpdate => {
                            let block_pos = bincode::deserialize(&data[1..13]).unwrap();
                            let action_type = data[13];

                            let existing_id = self.world.get_block(&block_pos);
                            if action_type == ChunkUpdateType::PlaceBlockEvent as u8 {
                                if existing_id > 0 {
                                    println!(
                                        "Cannot place block over id {} @ {},{},{}",
                                        existing_id, block_pos.x, block_pos.y, block_pos.z
                                    );
                                } else {
                                    let block_id: u32 = bincode::deserialize(&data[14..18]).unwrap();
                                    self.world.set_block(&block_pos, block_id as i32);
                                }
                            } else if action_type == ChunkUpdateType::DestroyBlockEvent as u8 {
                                if existing_id < 1 {
                                    println!(
                                        "Cannot destroy empty block id {} @ {},{},{}",
                                        existing_id, block_pos.x, block_pos.y, block_pos.z
                                    );
                                } else {
                                    self.world.set_block(&block_pos, 0);
                                }
                            } else {
                                println!(
                                    "Received unknown chunk update type with value {}",
                                    action_type
                                );
                            }

                            let col_position = World::world_to_column_position(&Vector2::new(
                                block_pos.x,
                                block_pos.z,
                            ));
                            let packet_data =
                                assemble_chunk_contents_packet(self.world.get_column(&col_position));
                            let packet =
                                Packet::new(&packet_data, PacketMode::ReliableSequenced).unwrap();
                            sender.send_packet(packet, channel_id).unwrap();
                        }
                        PacketType::ChunkContents => eprintln!("Server received \"PacketType::ChunkContents\". Clients should not be sending this..."),
                    }
                }
                _ => (),
            }
        }

        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.world.save_to_file();

        Ok(())
    }
}

fn main() -> Result<()> {
    let mut game = Game::new()?;
    game.run()?;
    game.shutdown()
}

/// Runs the lua script at the `path` and inserts the new items into the `item_manager`
pub fn load_items(item_manger: &mut ItemManager, path: String) {
    let asset_script = fs::read_to_string(path).expect("Unable to load loadAssetInfo script");

    let lua = Lua::new();

    lua.context(|lua_ctx| {
        let globals = lua_ctx.globals(); // Get globals from lua

        lua_ctx.scope(|scope| {

            let add_asset = // Create a function that takes in all info and compiles it into a ItemInfo struct
                scope.create_function_mut(|_, (item_name, item_type_str, is_transparent, show_in_inventory, coords): (String, String, bool, bool, Vec<u16>)| {

                    let item_type: ItemType;
                    match item_type_str.as_str() {
                        "Air" => item_type = ItemType::Air,
                        "BlockCube" => item_type = ItemType::BlockCube,
                        "BlockCross" => item_type = ItemType::BlockCross,
                        "UserItem" => item_type = ItemType::UserItem,
                        _ => item_type = ItemType::UserItem,
                    }

                    let new_item = ItemInfo {
                        item_type,
                        is_transparent,
                        show_in_inventory,
                        name: item_name,
                        top_tex_coords: TextureCoordinates::extract_coordinates(Vector2::new(10, 10),
                        Vector2::new(coords[0].into(), coords[1].into())),
                        side_tex_coords: TextureCoordinates::extract_coordinates(Vector2::new(10, 10),
                        Vector2::new(coords[2].into(), coords[3].into())),
                        bottom_tex_coords: TextureCoordinates::extract_coordinates(Vector2::new(10, 10),
                        Vector2::new(coords[4].into(), coords[5].into())),
                    };

                    item_manger.put_new_item(new_item);

                    Ok(())
                }).unwrap();
            globals.set("add_asset", add_asset).unwrap();

            let set_atlas = // Sets which atlas the texture is in
                lua_ctx.create_function(|_, (atlas_path, width, height): (String, u16, u16)| {
                    println!("Set Atlas: {}, {}, {}", atlas_path, width, height);

                    Ok(())
                }).unwrap();
            globals.set("set_atlas", set_atlas).unwrap();

            lua_ctx.load(
                r#"
                    item_name = "UNKNOWN"
                    item_type = "UserItem"
                    is_transparent = false
                    show_in_inventory = true
                    top_coord_x, top_coord_y = 0, 0
                    side_coord_x, side_coord_y = 0, 0
                    bottom_coord_x, bottom_coord_y = 0, 0

                    function setInfo(name, itemType, isTransparent, showInInventory)
                        item_name = name or "UNKNOWN"
                        item_type = itemType or "UserItem"
                        is_transparent = isTransparent or false
                        show_in_inventory = showInInventory or true
                    end

                    function setCoords(topX, topY, sideX, sideY, bottomX, bottomY)
                        top_coord_x = topX or 0
                        top_coord_y = topY or 0
                        side_coord_x = sideX or top_coord_x
                        side_coord_y = sideY or top_coord_y
                        bottom_coord_x = bottomX or top_coord_x
                        bottom_coord_y = bottomY or top_coord_y
                    end

                    function pushItem()
                        add_asset(item_name, item_type, is_transparent, show_in_inventory, {top_coord_x, top_coord_y, side_coord_x, side_coord_y, bottom_coord_x, bottom_coord_y}) -- Change to pull from global variables
                    end

                    function setAtlas(path, width, height)
                        width = width or 10
                        height = height or 10
                        set_atlas(path, width, height)
                    end
                "#
            )
            .set_name("Load Asset Functions").unwrap()
            .exec()
            .expect("Load asset utility functions failed to load");

            lua_ctx
            .load(&asset_script)
            .set_name("Load Asset Info").unwrap()
            .exec()
            .expect("Lua asset script failed!");

        });
    })
}
