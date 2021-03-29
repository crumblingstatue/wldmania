extern crate ansi_term;
extern crate bidir_map;
extern crate byteorder;
extern crate clap;
extern crate rand;

use ansi_term::Colour::{Green, Red};
use bidir_map::BidirMap;
use clap::{App, AppSettings, Arg, SubCommand};
use rand::{rngs::ThreadRng, seq::SliceRandom, thread_rng, Rng};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{self, prelude::*};
use std::path::Path;
use world::WorldFile;

mod item_id_pairs;
mod prefix_names;
mod req_file;
mod world;

fn run() -> Result<(), Box<dyn Error>> {
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
                .about("Show info about a chest at a particular position")
                .arg(
                    Arg::with_name("wld-file")
                        .required(true)
                        .help("Path to a Terraria .wld file."),
                )
                .arg(Arg::with_name("x").required(true))
                .arg(Arg::with_name("y").required(true)),
        )
        .subcommand(
            SubCommand::with_name("corruption-percent")
                .about("Give percentage of how corrupt/crimson your world is")
                .arg(
                    Arg::with_name("wld-file")
                        .required(true)
                        .help("Path to a Terraria .wld file"),
                ),
        )
        .subcommand(
            SubCommand::with_name("count-ores")
                .about("Count most common ores in the world")
                .arg(
                    Arg::with_name("wld-file")
                        .required(true)
                        .help("Path to a Terraria .wld file"),
                ),
        );

    let matches = app.get_matches();

    if let Some(submatches) = matches.subcommand_matches("itemhunt") {
        if let Some(template_cfg_path) = submatches.value_of("gen") {
            generate_template_cfg(template_cfg_path)?;
        } else {
            let req_path = submatches.value_of("req-file").unwrap();
            let world_paths = submatches.values_of("wld-file").unwrap();
            itemhunt(req_path, world_paths)?;
        }
    } else if let Some(submatches) = matches.subcommand_matches("bless-chests") {
        if let Some(template_cfg_path) = submatches.value_of("gen") {
            generate_template_cfg(template_cfg_path)?;
        } else {
            let req_path = submatches.value_of("req-file").unwrap();
            let world_path = submatches.value_of("wld-file").unwrap();
            bless_chests(req_path, world_path.as_ref())?;
        }
    } else if let Some(submatches) = matches.subcommand_matches("find") {
        let world_path = submatches.value_of("wld-file").unwrap();
        let item_name = submatches.value_of("item-name").unwrap();
        find_item(world_path.as_ref(), item_name)?;
    } else if let Some(submatches) = matches.subcommand_matches("fix-npcs") {
        let world_path = submatches.value_of("wld-file").unwrap();
        fix_npcs(world_path.as_ref())?;
    } else if let Some(submatches) = matches.subcommand_matches("analyze-chests") {
        let world_path = submatches.value_of("wld-file").unwrap();
        analyze_chests(world_path.as_ref())?;
    } else if let Some(submatches) = matches.subcommand_matches("chest-info") {
        let world_path = submatches.value_of("wld-file").unwrap();
        let x = submatches.value_of("x").unwrap().parse()?;
        let y = submatches.value_of("y").unwrap().parse()?;
        chest_info(world_path.as_ref(), x, y).unwrap();
    } else if let Some(submatches) = matches.subcommand_matches("corruption-percent") {
        let world_path = submatches.value_of("wld-file").unwrap();
        corruption_percent(world_path.as_ref())?;
    } else if let Some(submatches) = matches.subcommand_matches("count-ores") {
        let world_path = submatches.value_of("wld-file").unwrap();
        count_ores(world_path.as_ref()).unwrap();
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
    }
}

fn chest_info(wld_path: &Path, x: u16, y: u16) -> Result<(), Box<dyn Error>> {
    let mut file = WorldFile::open(wld_path, false)?;
    let chests = file.read_chests()?;
    let basic_info = file.read_basic_info()?;
    let chest_types = file.read_chest_types(&basic_info)?;
    let ids = item_ids();
    for chest in &chests {
        if chest.x == x && chest.y == y {
            println!("{:?} Chest containing: ", chest_types[&(chest.x, chest.y)]);
            for item in chest.items.iter() {
                if item.stack > 0 {
                    print!("{} ", item.stack);
                    match ids.name_by_id(item.id as u16) {
                        Some(name) => println!("{}", name),
                        None => println!("Unknown({})", item.id),
                    }
                }
            }
            return Ok(());
        }
    }
    println!("No chest at {}, {}", x, y);
    Ok(())
}

fn generate_template_cfg(path: &str) -> io::Result<()> {
    let mut f = File::create(path)?;
    f.write_all(include_bytes!("../templates/itemhunt.list"))
}

pub struct ItemIdMap(BidirMap<u16, &'static str>);

impl ItemIdMap {
    fn name_by_id(&self, id: u16) -> Option<&str> {
        self.0.get_by_first(&id).cloned()
    }
    fn id_by_name(&self, name: &str) -> Option<u16> {
        self.0.get_by_second(&&name.to_lowercase()[..]).cloned()
    }
}

fn item_ids() -> ItemIdMap {
    use byteorder::{ReadBytesExt, LE};
    use std::io::{Cursor, SeekFrom};
    let mut reader = Cursor::new(item_id_pairs::ITEM_ID_PAIRS);
    let len = reader.read_u16::<LE>().unwrap();
    let mut item_ids = BidirMap::new();
    for _ in 0..len {
        let id = reader.read_u16::<LE>().unwrap();
        let name_len = reader.read_u8().unwrap();
        let pos = reader.seek(SeekFrom::Current(i64::from(name_len))).unwrap();
        item_ids.insert(id, unsafe {
            ::std::str::from_utf8_unchecked(
                &item_id_pairs::ITEM_ID_PAIRS[pos as usize - name_len as usize..pos as usize],
            )
        });
    }
    ItemIdMap(item_ids)
}

/// There are buffer areas at the edges of terraria worlds that exist, but the player cannot
/// access. For some reason, the world generator can generate chests there, even though they
/// cannot be looted by legit means.
const INACCESSIBLE_EDGE: u16 = 42;

fn is_inaccessible(x: u16, y: u16, basic_info: &::world::BasicInfo) -> bool {
    x < INACCESSIBLE_EDGE
        || y < INACCESSIBLE_EDGE
        || x > basic_info.width - INACCESSIBLE_EDGE
        || y > basic_info.height - INACCESSIBLE_EDGE
}

fn itemhunt<'a, I: Iterator<Item = &'a str>>(
    cfg_path: &str,
    world_paths: I,
) -> Result<(), Box<dyn Error>> {
    let id_map = item_ids();
    let mut required_items = req_file::from_path::<u16>(cfg_path.as_ref(), &id_map)?;
    let mut n_meet_reqs = 0;
    for world_path in world_paths {
        println!("{}:", world_path);
        let mut file = WorldFile::open(world_path.as_ref(), false)?;
        let basic_info = file.read_basic_info()?;
        let chests = file.read_chests()?;
        for chest in &chests[..] {
            if is_inaccessible(chest.x, chest.y, &basic_info) {
                eprintln!(
                    "Warning: Ignoring out-of-bounds chest at {}, {}",
                    chest.x, chest.y
                );
                continue;
            }
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
                    id_map.name_by_id(req.id).ok_or("Invalid id")?,
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
    Ok(())
}

fn find_item(world_path: &Path, name: &str) -> Result<(), Box<dyn Error>> {
    let ids = item_ids();
    let id = ids
        .id_by_name(name)
        .ok_or_else(|| format!("No matching id found for item '{}'", name))?;
    let mut file = WorldFile::open(world_path, false)?;
    let basic_info = file.read_basic_info()?;
    let chests = file.read_chests()?;
    for chest in &chests[..] {
        for item in &chest.items[..] {
            if item.stack != 0 && item.id == i32::from(id) {
                let pos = basic_info.tile_to_gps_pos(chest.x, chest.y);
                println!("Found in chest at {}", pos);
            }
        }
    }
    Ok(())
}

fn fix_npcs(world_path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = WorldFile::open(world_path, true)?;
    let basic_info = file.read_basic_info()?;
    let mut npcs = file.read_npcs()?;
    let mut fixed_any = false;
    for npc in &mut npcs {
        if npc.x.is_nan() || npc.y.is_nan() {
            // TODO: Need proper conversion from tile to entity coordinates.
            // Try multiplying by 16.
            npc.x = basic_info.spawn_x as f32 * 16.;
            npc.y = basic_info.spawn_y as f32 * 16.;
            fixed_any = true;
            println!("{} has NaN position, reset to spawn.", npc.name);
        }
    }
    if fixed_any {
        file.write_npcs(&npcs)?;
    } else {
        println!("No NPCs needed fixing.");
    }
    Ok(())
}

fn place_in_chest(
    chest: &mut world::Chest,
    id: i32,
    prefix: u8,
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
            item.prefix_id = prefix;
            return;
        }
    }
}

fn validate_req_for_bless<T: Default>(reqs: &[req_file::Requirement<T>]) -> Result<(), String> {
    let ids = item_ids();
    for req in reqs {
        if req.only_in.is_empty() {
            let name = ids.name_by_id(req.id).unwrap();
            return Err(format!(
                "You need to specify at least one chest type for {}",
                name
            ));
        }
    }
    Ok(())
}

fn bless_chests(cfg_path: &str, world_path: &Path) -> Result<(), Box<dyn Error>> {
    let item_ids = item_ids();
    struct Tracker {
        acceptable_chest_indexes: Box<dyn Iterator<Item = usize>>,
    }
    impl Default for Tracker {
        fn default() -> Self {
            Self {
                acceptable_chest_indexes: Box::new(::std::iter::empty()),
            }
        }
    }
    let mut reqs = req_file::from_path::<Tracker>(cfg_path.as_ref(), &item_ids)?;
    validate_req_for_bless(&reqs)?;
    let mut file = WorldFile::open(world_path, true)?;
    let mut chests = file.read_chests()?;
    let basic_info = file.read_basic_info()?;
    let chest_types = file.read_chest_types(&basic_info)?;
    let mut rng = thread_rng();
    let chest_indexes = 0..chests.len();
    for req in &mut reqs {
        // Decrease stack count for every item that already exists in the world
        for chest in &chests[..] {
            if is_inaccessible(chest.x, chest.y, &basic_info) {
                continue;
            }
            for item in &chest.items[..] {
                if item.stack != 0 && item.id == i32::from(req.id) && req.n_stacks > 0 {
                    req.n_stacks -= 1;
                }
            }
        }
        // Set up chest indexes to place the item in. Might be only specific chest types.
        if req.n_stacks > 0 {
            let mut matching_indexes: Vec<usize> = chest_indexes
                .clone()
                .filter(|&idx| {
                    let chest = &chests[idx];
                    let type_ = chest_types[&(chest.x, chest.y)];
                    req.only_in.contains(&type_) && !is_inaccessible(chest.x, chest.y, &basic_info)
                })
                .collect();
            matching_indexes.shuffle(&mut rng);
            req.tracker.acceptable_chest_indexes = Box::new(matching_indexes.into_iter().cycle())
        }
    }
    for mut req in reqs {
        for _ in 0..req.n_stacks {
            let index = match req.tracker.acceptable_chest_indexes.next() {
                Some(idx) => idx,
                None => {
                    let name = item_ids.name_by_id(req.id).unwrap();
                    return Err(format!("No chest available for {}", name).into());
                }
            };
            let chest = &mut chests[index];
            place_in_chest(
                chest,
                i32::from(req.id),
                req.prefix_id,
                req.min_per_stack,
                req.max_per_stack,
                &mut rng,
            );
        }
    }
    file.write_chests(&chests)?;
    Ok(())
}

fn analyze_chests(world_path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = WorldFile::open(world_path, false)?;
    let chests = file.read_chests()?;
    #[derive(Debug)]
    struct ItemStat {
        stack_count: u32,
        total_count: u32,
    }
    let mut item_stats: HashMap<i32, ItemStat> = HashMap::new();
    let mut chests_containing_something = 0;
    for chest in &chests {
        let mut contains_something = false;
        for item in chest.items.iter() {
            if item.stack != 0 {
                contains_something = true;
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
        if contains_something {
            chests_containing_something += 1;
        }
    }
    let mut vec = item_stats.into_iter().collect::<Vec<_>>();
    vec.sort_by(|&(_, ref v1), &(_, ref v2)| v1.stack_count.cmp(&v2.stack_count).reverse());
    let ids = item_ids();
    println!("{:30}stack total", "name");
    for (k, v) in vec {
        match ids.name_by_id(k as u16) {
            Some(name) => print!("{:30}", name),
            None => print!("unknown({:4})                 ", k),
        }
        println!("{:<5} {}", v.stack_count, v.total_count);
    }
    println!(
        "{} total chests that contain something",
        chests_containing_something
    );
    Ok(())
}

fn corruption_percent(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = WorldFile::open(path, false)?;
    file.corruption_percent()
}

fn count_ores(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = WorldFile::open(path, false)?;
    file.count_ores()
}
