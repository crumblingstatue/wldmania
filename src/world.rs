use std::error::Error;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, SeekFrom};
use std::fmt;

pub struct World {
    pub chests: Vec<Chest>,
    pub width: i32,
    pub surface_y: f64,
    pub seed: String,
    pub npcs: Vec<Npc>,
}

struct Offsets {
    header: u64,
    chests: u64,
    npcs: u64,
}

fn read_offsets(f: &mut File) -> Result<Offsets, Box<Error>> {
    let _terraria_ver = f.read_i32::<LE>()?;
    let mut magic = [0u8; 7];
    f.read_exact(&mut magic)?;
    if magic[..] != b"relogic"[..] {
        return Err("Not a valid terraria map file.".into());
    }
    let filetype = f.read_u8()?;
    if filetype != 2 {
        return Err(format!("Unsupported filetype: {}", filetype).into());
    }
    let _times_saved = f.read_u32::<LE>()?;
    let _is_favorite = f.read_u64::<LE>()?;
    let n_pointers = f.read_i16::<LE>()?;
    if n_pointers != 10 {
        return Err(format!("Unsupported number of pointers: {}", n_pointers).into());
    }
    let header_ptr = f.read_i32::<LE>()?;
    let _tiles_ptr = f.read_i32::<LE>()?;
    let chests_ptr = f.read_i32::<LE>()?;
    let _signs_ptr = f.read_i32::<LE>()?;
    let npcs_ptr = f.read_i32::<LE>()?;
    Ok(Offsets {
        header: header_ptr as u64,
        chests: chests_ptr as u64,
        npcs: npcs_ptr as u64,
    })
}

impl World {
    pub fn load(path: &str) -> Result<Self, Box<Error>> {
        let mut f = File::open(path)?;
        let offsets = read_offsets(&mut f)?;
        f.seek(SeekFrom::Start(offsets.npcs))?;
        let mut npcs = Vec::new();
        while let Some(npc) = read_npc(&mut f)? {
            npcs.push(npc);
        }
        f.seek(SeekFrom::Start(offsets.header))?;
        let _name = read_string(&mut f)?;
        let seed = read_string(&mut f)?;
        let _gen_version = f.read_i64::<LE>()?;
        let mut guid = [0u8; 16];
        f.read_exact(&mut guid)?;
        let _id = f.read_i32::<LE>()?;
        let _bound_left = f.read_i32::<LE>()?;
        let _bound_right = f.read_i32::<LE>()?;
        let _bound_top = f.read_i32::<LE>()?;
        let _bound_bottom = f.read_i32::<LE>()?;
        let _height = f.read_i32::<LE>()?;
        let width = f.read_i32::<LE>()?;
        let _expert = f.read_u8()?;
        let _creation_time = f.read_i64::<LE>()?;
        let _moon_type = f.read_u8()?;
        let _tree_x_1 = f.read_i32::<LE>()?;
        let _tree_x_2 = f.read_i32::<LE>()?;
        let _tree_x_3 = f.read_i32::<LE>()?;
        let _tree_style_1 = f.read_i32::<LE>()?;
        let _tree_style_2 = f.read_i32::<LE>()?;
        let _tree_style_3 = f.read_i32::<LE>()?;
        let _tree_style_4 = f.read_i32::<LE>()?;
        let _cave_back_1 = f.read_i32::<LE>()?;
        let _cave_back_2 = f.read_i32::<LE>()?;
        let _cave_back_3 = f.read_i32::<LE>()?;
        let _cave_back_style_1 = f.read_i32::<LE>()?;
        let _cave_back_style_2 = f.read_i32::<LE>()?;
        let _cave_back_style_3 = f.read_i32::<LE>()?;
        let _cave_back_style_4 = f.read_i32::<LE>()?;
        let _ice_back_style = f.read_i32::<LE>()?;
        let _jungle_back_style = f.read_i32::<LE>()?;
        let _hell_back_style = f.read_i32::<LE>()?;
        let _spawn_x = f.read_i32::<LE>()?;
        let _spawn_y = f.read_i32::<LE>()?;
        let surface_y = f.read_f64::<LE>()?;
        f.seek(SeekFrom::Start(offsets.chests))?;
        let n_chests = f.read_i16::<LE>()?;
        let items_per_chest = f.read_i16::<LE>()?;
        if items_per_chest != 40 {
            return Err(format!("Unsupported items per chest: {}", items_per_chest).into());
        }
        let mut chests = Vec::new();
        for _ in 0..n_chests {
            chests.push(Chest::read(&mut f)?);
        }
        Ok(Self {
            npcs,
            chests,
            width,
            surface_y,
            seed,
        })
    }
    pub fn tile_to_gps_pos(&self, x: i32, y: i32) -> GpsPos {
        let raw_x = x * 2 - self.width;
        let raw_y = self.surface_y * 2.0 - f64::from(y) * 2.0;
        let x_side = if raw_x > 0 { XSide::East } else { XSide::West };
        let y_side = if raw_y > 0.0 {
            YSide::AboveSurface
        } else {
            YSide::BelowSurface
        };
        GpsPos {
            x_offset: raw_x.abs() as u32,
            y_offset: raw_y.abs() as u32,
            x_side,
            y_side,
        }
    }
    pub fn patch_npcs(&self, file_path: &str) -> Result<(), Box<Error>> {
        use std::fs::OpenOptions;
        let mut f = OpenOptions::new().read(true).write(true).open(file_path)?;
        let offsets = read_offsets(&mut f)?;
        f.seek(SeekFrom::Start(offsets.npcs))?;
        for npc in &self.npcs {
            write_npc(&mut f, npc)?;
        }
        Ok(())
    }
}

pub struct GpsPos {
    x_offset: u32,
    y_offset: u32,
    x_side: XSide,
    y_side: YSide,
}

impl fmt::Display for GpsPos {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let xside = match self.x_side {
            XSide::West => "west",
            XSide::East => "east",
        };
        let yside = match self.y_side {
            YSide::AboveSurface => "above surface",
            YSide::BelowSurface => "below surface",
        };
        write!(
            f,
            "{} {}, {} {}",
            self.x_offset,
            xside,
            self.y_offset,
            yside
        )
    }
}

pub enum XSide {
    West,
    East,
}

pub enum YSide {
    AboveSurface,
    BelowSurface,
}

const CHEST_MAX_ITEMS: i8 = 40;

pub struct Chest {
    pub x: i32,
    pub y: i32,
    pub name: String,
    pub items: [Option<Item>; CHEST_MAX_ITEMS as usize],
}

impl Chest {
    fn read(f: &mut File) -> io::Result<Self> {
        let x = f.read_i32::<LE>()?;
        let y = f.read_i32::<LE>()?;
        let name = read_string(f)?;
        let mut items: [Option<Item>; CHEST_MAX_ITEMS as usize] =
            unsafe { ::std::mem::uninitialized() };
        for item in &mut items[..] {
            *item = Item::read(f)?;
        }
        Ok(Self { name, x, y, items })
    }
}

fn read_string(f: &mut File) -> io::Result<String> {
    let len = read_string_len(f)?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn write_string(f: &mut File, string: &str) -> io::Result<()> {
    let len = string.len();
    // Can't bother with that whole encoding bullshit. Just simply write the length value,
    // bail if it's larger than 127.
    assert!(len < 127);
    f.write_u8(len as u8)?;
    f.write_all(string.as_bytes())?;
    Ok(())
}

fn read_string_len(f: &mut File) -> io::Result<usize> {
    let mut len = 0;
    let mut shift: u32 = 0;
    loop {
        let segment = f.read_u8()?;
        len |= usize::from((segment & 0b0111_1111).wrapping_shl(shift));
        shift += 7;
        if (segment & 0b1000_0000) == 0 {
            break;
        }
    }
    Ok(len)
}

pub struct Item {
    pub stack: u16,
    pub id: i32,
    pub prefix_id: u8,
}

impl Item {
    fn read(f: &mut File) -> io::Result<Option<Self>> {
        let stack = f.read_u16::<LE>()?;
        if stack == 0 {
            Ok(None)
        } else {
            let id = f.read_i32::<LE>()?;
            let prefix_id = f.read_u8()?;
            Ok(Some(Self {
                stack,
                id,
                prefix_id,
            }))
        }
    }
}

fn read_npc(f: &mut File) -> io::Result<Option<Npc>> {
    let active = f.read_u8()? != 0;
    if !active {
        return Ok(None);
    }
    let sprite = f.read_i32::<LE>()?;
    let name = read_string(f)?;
    let x = f.read_f32::<LE>()?;
    let y = f.read_f32::<LE>()?;
    let homeless = f.read_u8()?;
    let home_x = f.read_i32::<LE>()?;
    let home_y = f.read_i32::<LE>()?;
    Ok(Some(Npc {
        sprite,
        name,
        x,
        y,
        homeless: homeless != 0,
        home_x,
        home_y,
    }))
}

fn write_npc(f: &mut File, npc: &Npc) -> io::Result<()> {
    f.write_u8(1)?;
    f.write_i32::<LE>(npc.sprite)?;
    write_string(f, &npc.name)?;
    f.write_f32::<LE>(npc.x)?;
    f.write_f32::<LE>(npc.y)?;
    f.write_u8(if npc.homeless { 1 } else { 0 })?;
    f.write_i32::<LE>(npc.home_x)?;
    f.write_i32::<LE>(npc.home_y)?;
    Ok(())
}

pub struct Npc {
    pub sprite: i32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub homeless: bool,
    pub home_x: i32,
    pub home_y: i32,
}
