use ansi_term::Colour::{Green, Red};
use clap::Parser;
use rand::{rngs::ThreadRng, seq::SliceRandom, thread_rng, Rng};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{self, prelude::*};
use std::path::{Path, PathBuf};
use terraria_wld::WorldFile;

use crate::item_id_pairs::ITEM_ID_PAIRS;

mod item_id_pairs;
mod prefix_names;
mod req_file;

#[derive(Parser)]
#[clap(about, version)]
/// Terraria world inspection/manupilation tool
enum Args {
    /// Check if world(s) contain the desired items
    Itemhunt {
        /// File containing the list of desired items
        req_path: PathBuf,
        /// Paths to terraria .wld files to search
        #[clap(required = true)]
        world_paths: Vec<PathBuf>,
    },
    /// Bless the chests in the world with the desired items
    BlessChests {
        /// File containing the list of desired items
        req_path: PathBuf,
        /// Paths to terraria .wld files to search
        #[clap(required = true)]
        world_paths: Vec<PathBuf>,
    },
    /// Find an item in the world
    Find {
        /// Name of the item to find
        item_name: String,
        /// Paths to terraria .wld files to search
        #[clap(required = true)]
        world_paths: Vec<PathBuf>,
    },
    /// Fix NPCs that disappeared due to the NaN position bug.
    FixNpcs {
        /// Paths to terraria .wld files to fix
        #[clap(required = true)]
        world_paths: Vec<PathBuf>,
    },
    /// Analyze the contents of chests
    AnalyzeChests {
        /// Paths to terraria .wld files to analyze
        #[clap(required = true)]
        world_paths: Vec<PathBuf>,
    },
    /// Show info about a chest at the given position
    ChestInfo {
        /// Path to a Terraria .wld file to look at
        world_path: PathBuf,
        /// X position of chest
        x: u16,
        /// Y position of chest
        y: u16,
    },
    /// Show the corruption/crimson percentage of worlds
    CorruptionPercent {
        /// Paths to terraria .wld files to analyze
        #[clap(required = true)]
        world_paths: Vec<PathBuf>,
    },
    /// Count the ores in the given worlds
    CountOres {
        /// Paths to terraria .wld files to analyze
        #[clap(required = true)]
        world_paths: Vec<PathBuf>,
    },
    /// Generate template requirements file
    GenReq {
        /// Path to write the template file to
        path: PathBuf,
    },
}

fn run() -> Result<(), Box<dyn Error>> {
    match Args::parse() {
        Args::Itemhunt {
            req_path,
            world_paths,
        } => {
            itemhunt(&req_path, &world_paths)?;
        }
        Args::BlessChests {
            req_path,
            world_paths,
        } => {
            for world in world_paths {
                bless_chests(&req_path, &world)?;
            }
        }
        Args::Find {
            world_paths,
            item_name,
        } => {
            for path in world_paths {
                find_item(&path, &item_name)?;
            }
        }
        Args::FixNpcs { world_paths } => {
            for path in world_paths {
                fix_npcs(&path)?;
            }
        }
        Args::AnalyzeChests { world_paths } => {
            for path in world_paths {
                analyze_chests(&path)?;
            }
        }
        Args::ChestInfo { world_path, x, y } => {
            chest_info(&world_path, x, y)?;
        }
        Args::CorruptionPercent { world_paths } => {
            for path in world_paths {
                corruption_percent(&path)?;
            }
        }
        Args::CountOres { world_paths } => {
            for path in world_paths {
                count_ores(&path)?;
            }
        }
        Args::GenReq { path } => {
            generate_template_cfg(&path)?;
        }
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
    let chest_types = file.read_chest_types()?;
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

fn generate_template_cfg(path: &Path) -> io::Result<()> {
    let mut f = File::create(path)?;
    f.write_all(include_bytes!("../templates/itemhunt.list"))
}

pub struct ItemIdMap(Vec<(u16, &'static str)>);

impl ItemIdMap {
    fn name_by_id(&self, id: u16) -> Option<&str> {
        self.0.iter().find(|pair| pair.0 == id).map(|pair| pair.1)
    }
    fn id_by_name(&self, name: &str) -> Option<u16> {
        self.0.iter().find(|pair| pair.1 == name).map(|pair| pair.0)
    }
}

fn item_ids() -> ItemIdMap {
    let mut item_ids = Vec::new();
    for line in ITEM_ID_PAIRS.lines() {
        let mut parts = line.split('\t');
        let id: u16 = parts.next().unwrap().parse().unwrap();
        let name = parts.next().unwrap();
        item_ids.push((id, name));
    }
    ItemIdMap(item_ids)
}

/// There are buffer areas at the edges of terraria worlds that exist, but the player cannot
/// access. For some reason, the world generator can generate chests there, even though they
/// cannot be looted by legit means.
const INACCESSIBLE_EDGE: u16 = 42;

fn is_inaccessible(x: u16, y: u16, header: &terraria_wld::Header) -> bool {
    x < INACCESSIBLE_EDGE
        || y < INACCESSIBLE_EDGE
        || x > header.width - INACCESSIBLE_EDGE
        || y > header.height - INACCESSIBLE_EDGE
}

fn itemhunt<T, Iter>(cfg_path: &Path, world_paths: Iter) -> Result<(), Box<dyn Error>>
where
    T: AsRef<Path>,
    Iter: IntoIterator<Item = T>,
{
    let id_map = item_ids();
    let mut required_items = req_file::from_path::<u16>(cfg_path, &id_map)?;
    let mut n_meet_reqs = 0;
    for world_path in world_paths {
        let world_path = world_path.as_ref();
        eprintln!("{}:", world_path.display());
        let mut file = WorldFile::open(world_path, false)?;
        let header = file.read_header()?;
        let chests = file.read_chests()?;
        for chest in &chests[..] {
            if is_inaccessible(chest.x, chest.y, &header) {
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
    let header = file.read_header()?;
    let chests = file.read_chests()?;
    for chest in &chests[..] {
        for item in &chest.items[..] {
            if item.stack != 0 && item.id == i32::from(id) {
                let pos = header.tile_to_gps_pos(chest.x, chest.y);
                println!("Found in chest at {}", pos);
            }
        }
    }
    Ok(())
}

fn fix_npcs(world_path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = WorldFile::open(world_path, true)?;
    let header = file.read_header()?;
    let mut npcs = file.read_npcs()?;
    let mut fixed_any = false;
    for npc in &mut npcs {
        if npc.x.is_nan() || npc.y.is_nan() {
            // TODO: Need proper conversion from tile to entity coordinates.
            // Try multiplying by 16.
            npc.x = header.spawn_x as f32 * 16.;
            npc.y = header.spawn_y as f32 * 16.;
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
    chest: &mut terraria_wld::Chest,
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
                rng.gen_range(min_stack..max_stack)
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

fn bless_chests(cfg_path: &Path, world_path: &Path) -> Result<(), Box<dyn Error>> {
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
    let mut reqs = req_file::from_path::<Tracker>(cfg_path, &item_ids)?;
    validate_req_for_bless(&reqs)?;
    let mut file = WorldFile::open(world_path, true)?;
    let mut chests = file.read_chests()?;
    let header = file.read_header()?;
    let chest_types = file.read_chest_types()?;
    let mut rng = thread_rng();
    let chest_indexes = 0..chests.len();
    for req in &mut reqs {
        // Decrease stack count for every item that already exists in the world
        for chest in &chests[..] {
            if is_inaccessible(chest.x, chest.y, &header) {
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
                    req.only_in.contains(&type_) && !is_inaccessible(chest.x, chest.y, &header)
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
    let mut world_file = WorldFile::open(path, false)?;
    let mut copper = 0;
    let mut tin = 0;
    let mut iron = 0;
    let mut lead = 0;
    let mut silver = 0;
    let mut tungsten = 0;
    let mut gold = 0;
    let mut platinum = 0;
    let mut sapphire = 0;
    let mut ruby = 0;
    let mut emerald = 0;
    let mut topaz = 0;
    let mut amethyst = 0;
    let mut diamond = 0;
    let mut amber = 0;
    world_file.read_tiles(|id, _i, tfo| match id {
        7 => copper += 1,
        166 => tin += 1,
        6 => iron += 1,
        167 => lead += 1,
        9 => silver += 1,
        168 => tungsten += 1,
        8 => gold += 1,
        169 => platinum += 1,
        63 => sapphire += 1,
        64 => ruby += 1,
        65 => emerald += 1,
        66 => topaz += 1,
        67 => amethyst += 1,
        68 => diamond += 1,
        566 => amber += 1,
        178 => {
            let tfi = tfo.unwrap();
            match tfi.x / 18 {
                0 => amethyst += 1,
                1 => topaz += 1,
                2 => sapphire += 1,
                3 => emerald += 1,
                4 => ruby += 1,
                5 => diamond += 1,
                6 => amber += 1,
                _ => panic!("invalid/unknown gem tile frame x"),
            }
        }
        _ => {}
    })?;
    if copper > 0 {
        println!("copper: {}", copper);
    }
    if tin > 0 {
        println!("tin: {}", tin);
    }
    if iron > 0 {
        println!("iron: {}", iron);
    }
    if lead > 0 {
        println!("lead: {}", lead);
    }
    if silver > 0 {
        println!("silver: {}", silver);
    }
    if tungsten > 0 {
        println!("tungsten: {}", tungsten);
    }
    if gold > 0 {
        println!("gold: {}", gold);
    }
    if platinum > 0 {
        println!("platinum: {}", platinum);
    }
    println!("=============");
    println!("ametyhst: {}", amethyst);
    println!("topaz: {}", topaz);
    println!("sapphire: {}", sapphire);
    println!("emerald: {}", emerald);
    println!("ruby: {}", ruby);
    println!("diamond: {}", diamond);
    println!("amber: {}", amber);
    Ok(())
}
