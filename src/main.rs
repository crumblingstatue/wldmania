#![feature(nll)]

extern crate ansi_term;
extern crate bidir_map;
extern crate byteorder;
extern crate clap;
extern crate csv;
extern crate rand;

use world::World;
use clap::{App, AppSettings, Arg, SubCommand};
use std::fs::File;
use std::io::prelude::*;
use ansi_term::Colour::{Green, Red};
use std::collections::HashMap;
use bidir_map::BidirMap;
use rand::{thread_rng, Rng, ThreadRng};

mod world;
mod req_file;

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
        )
        .subcommand(
            SubCommand::with_name("chest-info")
                .about("Show infor about a chest at a particular position")
                .arg(
                    Arg::with_name("wld-file")
                        .required(true)
                        .help("Path to a Terraria .wld file."),
                )
                .arg(Arg::with_name("x").required(true))
                .arg(Arg::with_name("y").required(true)),
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
    } else if let Some(submatches) = matches.subcommand_matches("chest-info") {
        let world_path = submatches.value_of("wld-file").unwrap();
        let x = submatches.value_of("x").unwrap().parse().unwrap();
        let y = submatches.value_of("y").unwrap().parse().unwrap();
        chest_info(world_path, x, y);
    }
}

fn chest_info(wld_path: &str, x: u16, y: u16) {
    let world = match World::load(wld_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("Failed to load world \"{}\": {}", wld_path, e);
            return;
        }
    };
    for chest in &world.chests {
        if chest.x == x && chest.y == y {
            println!("{:?}", world.chest_types[&(chest.x, chest.y)]);
            return;
        }
    }
    println!("No chest at {}, {}", x, y);
}

fn generate_template_cfg(path: &str) {
    let mut f = File::create(path).unwrap();
    f.write_all(include_bytes!("../templates/itemhunt.list"))
        .unwrap();
}

pub struct ItemIdMap(BidirMap<u16, String>);

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

fn itemhunt<'a, I: Iterator<Item = &'a str>>(cfg_path: &str, world_paths: I) {
    let id_map = read_item_ids();
    let mut required_items = req_file::from_path::<u16>(cfg_path.as_ref(), &id_map).unwrap();
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
                    for req in &mut required_items {
                        if item.id == i32::from(req.id) {
                            req.tracker += 1;
                        }
                    }
                }
            }
        }
        let mut didnt_meet_reqs = false;
        for req in &required_items {
            if req.tracker < req.n_stacks {
                didnt_meet_reqs = true;
                let msg = format!(
                    "{} - {}/{}",
                    id_map.name_by_id(req.id).unwrap(),
                    req.tracker,
                    req.n_stacks
                );
                println!("{}", Red.paint(msg));
            }
        }
        if !didnt_meet_reqs {
            println!("{}", Green.paint("This world meets all requirements."));
            n_meet_reqs += 1;
        }
        // Reset times_found values
        for req in &mut required_items {
            req.tracker = 0;
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
                let type_ = world.chest_types[&(chest.x, chest.y)];
                println!("Found in {:?} chest at {}", type_, pos);
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
    let item_ids = read_item_ids();
    struct Tracker {
        acceptable_chest_indexes: Box<Iterator<Item = usize>>,
    }
    impl Default for Tracker {
        fn default() -> Self {
            Self {
                acceptable_chest_indexes: Box::new(::std::iter::empty()),
            }
        }
    }
    let mut reqs = req_file::from_path::<Tracker>(cfg_path.as_ref(), &item_ids).unwrap();
    let mut world = match World::load(world_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("Failed to load world \"{}\": {}", world_path, e);
            return;
        }
    };
    let mut rng = thread_rng();
    let chest_indexes = 0..world.chests.len();
    for req in &mut reqs {
        // chest type of chest at index matches any of required chest types
        let mut matching_indexes: Vec<usize> = chest_indexes
            .clone()
            .filter(|&idx| {
                let chest = &world.chests[idx];
                let type_ = world.chest_types[&(chest.x, chest.y)];
                req.only_in.is_empty() || req.only_in.contains(&type_)
            })
            .collect();
        rng.shuffle(&mut matching_indexes);
        req.tracker.acceptable_chest_indexes = Box::new(matching_indexes.into_iter().cycle())
    }
    for mut req in reqs {
        for _ in 0..req.n_stacks {
            let chest = &mut world.chests[req.tracker.acceptable_chest_indexes.next().unwrap()];
            place_in_chest(
                chest,
                i32::from(req.id),
                req.min_per_stack,
                req.max_per_stack,
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
