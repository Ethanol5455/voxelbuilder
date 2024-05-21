use cgmath::Vector2;

pub enum ItemType {
    Air,
    BlockCube,
    BlockCross,
    UserItem,
}

pub struct ItemInfo {
    pub item_type: ItemType,
    pub is_transparent: bool,
    pub show_in_inventory: bool,
    pub name: String,
    pub top_tex_coords: TextureCoordinates,
    pub side_tex_coords: TextureCoordinates,
    pub bottom_tex_coords: TextureCoordinates,
}

pub struct ItemManager {
    items: Vec<ItemInfo>,
}

impl ItemManager {
    /// Creates an empty ItemManager
    pub fn new() -> ItemManager {
        ItemManager { items: Vec::new() }
    }

    /// Inserts a new item into the ItemManager
    pub fn put_new_item(&mut self, item: ItemInfo) {
        self.items.push(item);
    }

    /// Gets item info from id
    pub fn get_item_by_id(&self, id: i32) -> Option<&ItemInfo> {
        self.items.get(id as usize)
    }

    /// Gets item id from name
    pub fn get_id_by_name(&self, name: String) -> Option<i32> {
        for i in 0..self.items.len() {
            if self.items.get(i).unwrap().name == name {
                return Some(i as i32);
            }
        }
        None
    }

    pub fn is_transparent(&self, item_id: i32) -> Option<bool> {
        match self.get_item_by_id(item_id) {
            Some(info) => Some(info.is_transparent),
            None => None,
        }
    }
}

pub struct TextureCoordinates {
    pub bl: Vector2<f32>,
    pub br: Vector2<f32>,
    pub tl: Vector2<f32>,
    pub tr: Vector2<f32>,
}

impl TextureCoordinates {
    pub fn extract_coordinates(count: Vector2<i32>, position: Vector2<i32>) -> Self {
        let segment_size = Vector2::new(1.0 / count.x as f32, 1.0 / count.y as f32);
        TextureCoordinates {
            bl: Vector2::new(
                segment_size.x * position.x as f32,
                segment_size.y * (position.y as f32 + 1.0),
            ),
            br: Vector2::new(
                segment_size.x * (position.x as f32 + 1.0),
                segment_size.y * (position.y as f32 + 1.0),
            ),
            tr: Vector2::new(
                segment_size.x * (position.x as f32 + 1.0),
                segment_size.y * position.y as f32,
            ),
            tl: Vector2::new(
                segment_size.x * position.x as f32,
                segment_size.y * position.y as f32,
            ),
        }
    }
}
