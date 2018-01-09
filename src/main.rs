extern crate ansi_term;
extern crate byteorder;
extern crate clap;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use std::fmt;
use world::World;
use clap::{App, AppSettings, Arg, SubCommand};
use std::fs::File;
use std::io::prelude::*;
use ansi_term::Colour::{Green, Red};

mod world;

/*struct ChestNameFmt<'a>(&'a str);

impl<'a> fmt::Display for ChestNameFmt<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.is_empty() {
            write!(f, "chest")
        } else {
            write!(f, "chest \"{}\"", self.0)
        }
    }
}*/

fn main() {
    let app = App::new("wldmania")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Terraria world inspection/manupilation tool")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("itemhunt")
                .about("Check if world(s) contain the desired items")
                .arg(
                    Arg::with_name("req-file")
                        .index(1)
                        .required_unless("gen")
                        .help("TOML file containing the desired items"),
                )
                .arg(
                    Arg::with_name("gen")
                        .short("g")
                        .help("Generate a template requirements file.")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("wld-file")
                        .multiple(true)
                        .help("Path to a Terraria .wld file.")
                        .required_unless("gen"),
                ),
        )
        .subcommand(
            SubCommand::with_name("find")
                .about("Find an item in the world")
                .arg(
                    Arg::with_name("wld-file")
                        .required(true)
                        .help("Path to a Terraria .wld file."),
                )
                .arg(
                    Arg::with_name("item-id")
                        .required(true)
                        .help("Id of the item you want to find"),
                ),
        )
        .subcommand(
            SubCommand::with_name("fix-npcs")
                .about("Fix NPCs that disappeared due to the NaN position bug.")
                .arg(
                    Arg::with_name("wld-file")
                        .required(true)
                        .help("Path to a Terraria .wld file."),
                ),
        );

    let matches = app.get_matches();

    if let Some(submatches) = matches.subcommand_matches("itemhunt") {
        if let Some(template_cfg_path) = submatches.value_of("gen") {
            generate_template_cfg(template_cfg_path);
        } else {
            let req_path = submatches.value_of("req-file").unwrap();
            let world_paths = submatches.values_of("wld-file").unwrap();
            itemhunt(req_path, world_paths);
        }
    } else if let Some(submatches) = matches.subcommand_matches("find") {
        let world_path = submatches.value_of("wld-file").unwrap();
        let item_id = submatches.value_of("item-id").unwrap();
        let item_id = item_id.parse::<i32>().unwrap();
        find_item(world_path, item_id);
    } else if let Some(submatches) = matches.subcommand_matches("fix-npcs") {
        let world_path = submatches.value_of("wld-file").unwrap();
        fix_npcs(world_path);
    }
}

fn generate_template_cfg(path: &str) {
    let mut f = File::create(path).unwrap();
    f.write_all(include_bytes!("../templates/itemhunt.toml"))
        .unwrap();
}

fn itemhunt<'a, I: Iterator<Item = &'a str>>(cfg_path: &str, world_paths: I) {
    use std::collections::HashMap;

    #[derive(Deserialize)]
    struct Item {
        id: i32,
        #[serde(default = "item_amount_default")] amount: i32,
        #[serde(skip)] times_found: i32,
    }
    fn item_amount_default() -> i32 {
        1
    }
    struct NameOrId<'a>(&'a str, i32);

    impl<'a> fmt::Display for NameOrId<'a> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            if self.0.is_empty() {
                write!(f, "{}", self.1)
            } else {
                write!(f, "{}", self.0)
            }
        }
    }
    let mut f = File::open(cfg_path).unwrap();
    let mut buf = String::new();
    f.read_to_string(&mut buf).unwrap();
    let mut required_items: HashMap<String, Item> = toml::from_str(&buf).unwrap();
    let mut n_meet_reqs = 0;
    for world_path in world_paths {
        let world = match World::load(world_path) {
            Ok(world) => world,
            Err(e) => {
                eprintln!("Failed to load world \"{}\": {}", world_path, e);
                return;
            }
        };
        println!("{} ({}):", world_path, world.seed);
        for chest in &world.chests[..] {
            for item in &chest.items[..] {
                if let Some(ref item) = *item {
                    for req in required_items.values_mut() {
                        if item.id == req.id {
                            req.times_found += 1;
                        }
                    }
                }
            }
        }
        let mut didnt_meet_reqs = false;
        for (name, req) in &required_items {
            if req.times_found < req.amount {
                didnt_meet_reqs = true;
                let msg = format!(
                    "{} - {}/{}",
                    NameOrId(name, req.id),
                    req.times_found,
                    req.amount
                );
                println!("{}", Red.paint(msg));
            }
        }
        if !didnt_meet_reqs {
            println!("{}", Green.paint("This world meets all requirements."));
            n_meet_reqs += 1;
        }
        // Reset times_found values
        for req in required_items.values_mut() {
            req.times_found = 0;
        }
    }
    println!("{} worlds in total meet the requirements.", n_meet_reqs);
}

fn find_item(world_path: &str, id: i32) {
    let world = match World::load(world_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("Failed to load world \"{}\": {}", world_path, e);
            return;
        }
    };
    for chest in &world.chests[..] {
        for item in &chest.items[..] {
            if let Some(ref item) = *item {
                if item.id == id {
                    let pos = world.tile_to_gps_pos(chest.x, chest.y);
                    println!("Found in chest at {}", pos);
                }
            }
        }
    }
}

fn fix_npcs(world_path: &str) {
    let mut world = match World::load(world_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("Failed to load world \"{}\": {}", world_path, e);
            return;
        }
    };
    let mut fixed_any = false;
    for npc in &mut world.npcs {
        if npc.x.is_nan() || npc.y.is_nan() {
            // TODO: Need proper conversion from tile to entity coordinates.
            // Try multiplying by 16.
            npc.x = world.spawn_x as f32 * 16.;
            npc.y = world.spawn_y as f32 * 16.;
            fixed_any = true;
            println!("{} has NaN position, reset to spawn.", npc.name);
        }
    }
    if fixed_any {
        world.patch_npcs(world_path).unwrap();
    } else {
        println!("No NPCs needed fixing.");
    }
}
