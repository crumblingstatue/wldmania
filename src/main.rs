extern crate byteorder;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use std::fmt;
use config::Config;
use world::World;

mod config;
mod world;

struct NameOrId<'a>(&'a config::RequiredItem);

impl<'a> fmt::Display for NameOrId<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let req = self.0;
        if req.name.is_empty() {
            write!(f, "{}", req.id)
        } else {
            write!(f, "{}", req.name)
        }
    }
}

struct ChestNameFmt<'a>(&'a str);

impl<'a> fmt::Display for ChestNameFmt<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.is_empty() {
            write!(f, "chest")
        } else {
            write!(f, "chest \"{}\"", self.0)
        }
    }
}

fn main() {
    let mut cfg = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load {}: {}", Config::PATH, e);
            return;
        }
    };
    let mut world = World::load(&cfg.world.path).unwrap();
    println!("World seed: {}", world.seed);
    for chest in &world.chests[..] {
        for item in &chest.items[..] {
            if let Some(ref item) = *item {
                for req in &mut cfg.required_items {
                    if item.id == req.id {
                        let gps_pos = world.tile_to_gps_pos(chest.x, chest.y);
                        println!(
                            "Found {} in {} at {}",
                            NameOrId(req),
                            ChestNameFmt(&chest.name),
                            gps_pos
                        );
                        req.times_found += 1;
                    }
                }
            }
        }
    }
    println!("The world does not meet the following requirements: ");
    for req in cfg.required_items {
        if req.times_found < req.required_amount {
            println!(
                "{} - {}/{}",
                NameOrId(&req),
                req.times_found,
                req.required_amount
            );
        }
    }
    for (k, v) in &cfg.npc_relocate {
        relocate_npc(&mut world, k, v);
    }
    if !cfg.npc_relocate.is_empty() {
        world.patch_npcs(&cfg.world.path).unwrap();
    }
}

fn relocate_npc(world: &mut World, name: &str, to: &config::Relocate) {
    for npc in &mut world.npcs {
        if npc.name == name {
            eprintln!("Relocating npc {} to {}, {}", name, to.x, to.y);
            npc.x = to.x;
            npc.y = to.y;
        }
    }
}
