use cgmath::{Vector2, Vector3};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Player {
    pub username: String,
    pub position: Vector3<f32>,
    pub rotation: Vector2<f32>,
}
