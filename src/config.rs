use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use toml;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Config {
    pub world: World,
    #[serde(default)] pub required_items: Vec<RequiredItem>,
    #[serde(default)] pub npc_relocate: HashMap<String, Relocate>,
}

#[derive(Deserialize)]
pub struct Relocate {
    pub x: f32,
    pub y: f32,
}

impl Config {
    pub fn load() -> Result<Self, Box<Error>> {
        let mut f = File::open(Self::PATH)?;
        let mut text = String::new();
        f.read_to_string(&mut text)?;
        let cfg = toml::from_str(&text)?;
        Ok(cfg)
    }
    pub const PATH: &'static str = "wldmania.toml";
}

#[derive(Deserialize)]
pub struct World {
    pub path: String,
}

#[derive(Deserialize)]
pub struct RequiredItem {
    pub id: i32,
    #[serde(default)] pub name: String,
    #[serde(skip)] pub times_found: i32,
    #[serde(default = "default_amount")] pub required_amount: i32,
}

fn default_amount() -> i32 {
    1
}
