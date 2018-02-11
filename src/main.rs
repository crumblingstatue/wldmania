#![feature(nll)]

extern crate ansi_term;
extern crate bidir_map;
extern crate byteorder;
extern crate clap;
extern crate csv;
extern crate rand;

use std::fmt;
use world::World;
use clap::{App, AppSettings, Arg, SubCommand};
use std::fs::File;
use std::io::prelude::*;
use ansi_term::Colour::{Green, Red};
use std::collections::HashMap;
use bidir_map::BidirMap;
use rand::{thread_rng, Rng, ThreadRng};

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
                        .help("File containing the desired items"),
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
            SubCommand::with_name("bless-chests")
                .about("Bless the chests of your world with the desired items")
                .arg(
                    Arg::with_name("req-file")
                        .index(1)
                        .required_unless("gen")
                        .help("File containing the desired items"),
                )
                .arg(
                    Arg::with_name("gen")
                        .short("g")
                        .help("Generate a template requirements file.")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("wld-file")
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
                    Arg::with_name("item-name")
                        .required(true)
                        .help("Name of the item you want to find"),
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
        )
        .subcommand(
            SubCommand::with_name("analyze-chests")
                .about("Analyze the contents of chests")
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
    } else if let Some(submatches) = matches.subcommand_matches("bless-chests") {
        if let Some(template_cfg_path) = submatches.value_of("gen") {
            generate_template_cfg(template_cfg_path);
        } else {
            let req_path = submatches.value_of("req-file").unwrap();
            let world_path = submatches.value_of("wld-file").unwrap();
            bless_chests(req_path, world_path);
        }
    } else if let Some(submatches) = matches.subcommand_matches("find") {
        let world_path = submatches.value_of("wld-file").unwrap();
        let item_name = submatches.value_of("item-name").unwrap();
        find_item(world_path, item_name);
    } else if let Some(submatches) = matches.subcommand_matches("fix-npcs") {
        let world_path = submatches.value_of("wld-file").unwrap();
        fix_npcs(world_path);
    } else if let Some(submatches) = matches.subcommand_matches("analyze-chests") {
        let world_path = submatches.value_of("wld-file").unwrap();
        analyze_chests(world_path);
    }
}

fn generate_template_cfg(path: &str) {
    let mut f = File::create(path).unwrap();
    f.write_all(include_bytes!("../templates/itemhunt.list"))
        .unwrap();
}

struct ItemIdMap(BidirMap<u16, String>);

impl ItemIdMap {
    fn name_by_id(&self, id: u16) -> Option<&str> {
        self.0.get_by_first(&id).map(String::as_str)
    }
    fn id_by_name(&self, name: &str) -> Option<u16> {
        self.0.get_by_second(&name.to_lowercase()).cloned()
    }
}

fn read_item_ids() -> ItemIdMap {
    let mut rdr = csv::Reader::from_path("./items/items.csv").unwrap();
    let mut item_ids = BidirMap::new();
    for result in rdr.records() {
        let record = result.unwrap();
        item_ids.insert(record[0].parse().unwrap(), record[1].to_lowercase());
    }
    ItemIdMap(item_ids)
}

struct Item {
    id: u16,
    amount: i32,
    times_found: i32,
    min_stack: u16,
    max_stack: u16,
}

fn read_item_req_list(cfg_path: &str) -> HashMap<String, Item> {
    let ids = read_item_ids();
    let mut f = File::open(cfg_path).unwrap();
    let mut buf = String::new();
    f.read_to_string(&mut buf).unwrap();
    let mut items: HashMap<String, Item> = HashMap::new();
    for line in buf.lines() {
        let (amount, name, min_stack, max_stack);
        if line.starts_with('*') {
            match line.find('~') {
                Some(tilde_pos) => {
                    amount = line[1..tilde_pos].parse().unwrap();
                    let first_hyphen = line.find('-').expect("Expected - after *amount~minstack");
                    min_stack = line[tilde_pos + 1..first_hyphen].parse().unwrap();
                    let first_space = line.find(' ')
                        .expect("Expected space after *amount~minstack-maxstack");
                    max_stack = line[first_hyphen + 1..first_space].parse().unwrap();
                    name = line[first_space..].trim();
                }
                None => {
                    let first_space = line.find(' ').expect("Expected space after *amount");
                    amount = line[1..first_space].parse().unwrap();
                    name = line[first_space..].trim();
                    min_stack = 1;
                    max_stack = 1;
                }
            }
        } else {
            amount = 1;
            min_stack = 1;
            max_stack = 1;
            name = line.trim();
        }
        let id = match ids.id_by_name(name) {
            Some(id) => id,
            None => panic!("Item \"{}\" doesn't map to a valid id.", name),
        };
        items.insert(
            name.into(),
            Item {
                id,
                amount,
                times_found: 0,
                min_stack,
                max_stack,
            },
        );
    }
    items
}

fn itemhunt<'a, I: Iterator<Item = &'a str>>(cfg_path: &str, world_paths: I) {
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
    let mut required_items: HashMap<String, Item> = read_item_req_list(cfg_path);
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
                if item.stack != 0 {
                    for req in required_items.values_mut() {
                        if item.id == i32::from(req.id) {
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
                    NameOrId(name, i32::from(req.id)),
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

fn find_item(world_path: &str, name: &str) {
    let ids = read_item_ids();
    let id = match ids.id_by_name(name) {
        Some(id) => id,
        None => {
            eprintln!("No matching id found for item '{}'", name);
            return;
        }
    };
    let world = match World::load(world_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("Failed to load world \"{}\": {}", world_path, e);
            return;
        }
    };
    let (w_width, w_surface_y) = (world.width(), world.surface_y());
    for chest in &world.chests[..] {
        for item in &chest.items[..] {
            if item.stack != 0 && item.id == i32::from(id) {
                let pos = World::tile_to_gps_pos(w_width, w_surface_y, chest.x, chest.y);
                println!("Found in chest at {}", pos);
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

fn place_in_chest(
    chest: &mut world::Chest,
    id: i32,
    min_stack: u16,
    max_stack: u16,
    rng: &mut ThreadRng,
) {
    for item in chest.items.iter_mut() {
        if item.stack == 0 {
            item.stack = if min_stack == 1 && max_stack == 1 {
                1
            } else {
                rng.gen_range(min_stack, max_stack)
            };
            item.id = id;
            item.prefix_id = 0;
            return;
        }
    }
}

fn bless_chests(cfg_path: &str, world_path: &str) {
    let reqs = read_item_req_list(cfg_path);
    let mut world = match World::load(world_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("Failed to load world \"{}\": {}", world_path, e);
            return;
        }
    };
    let mut rng = thread_rng();
    let mut chest_indexes: Vec<usize> = (0..world.chests.len()).collect();
    rng.shuffle(&mut chest_indexes);
    let mut chest_indexes = chest_indexes.into_iter().cycle();
    for req in reqs.values() {
        for _ in 0..req.amount {
            let chest = &mut world.chests[chest_indexes.next().unwrap()];
            place_in_chest(
                chest,
                i32::from(req.id),
                req.min_stack,
                req.max_stack,
                &mut rng,
            );
        }
    }
    world.patch_chests(world_path).unwrap();
}

fn analyze_chests(world_path: &str) {
    let world = match World::load(world_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("Failed to load world \"{}\": {}", world_path, e);
            return;
        }
    };
    #[derive(Debug)]
    struct ItemStat {
        stack_count: u32,
        total_count: u32,
    }
    let mut item_stats: HashMap<i32, ItemStat> = HashMap::new();
    for chest in &world.chests {
        for item in chest.items.iter() {
            if item.stack != 0 {
                match item_stats.get_mut(&item.id) {
                    Some(ref mut stat) => {
                        stat.stack_count += 1;
                        stat.total_count += u32::from(item.stack);
                    }
                    None => {
                        item_stats.insert(
                            item.id,
                            ItemStat {
                                stack_count: 1,
                                total_count: u32::from(item.stack),
                            },
                        );
                    }
                }
            }
        }
    }
    let mut vec = item_stats.into_iter().collect::<Vec<_>>();
    vec.sort_by(|&(_, ref v1), &(_, ref v2)| v1.stack_count.cmp(&v2.stack_count).reverse());
    let ids = read_item_ids();
    println!("{:30}stack total", "name");
    for (k, v) in vec {
        println!(
            "{:30}{:<5} {}",
            ids.name_by_id(k as u16).unwrap(),
            v.stack_count,
            v.total_count
        );
    }
}
