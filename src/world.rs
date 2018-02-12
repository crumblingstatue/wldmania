use std::error::Error;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, SeekFrom};
use std::fmt;
use std::collections::HashMap;

pub struct World {
    pub chests: Vec<Chest>,
    pub chest_types: HashMap<(u16, u16), ChestType>,
    pub width: u16,
    pub surface_y: f64,
    pub seed: String,
    pub npcs: Vec<Npc>,
    pub spawn_x: i32,
    pub spawn_y: i32,
}

struct Header {
    header: i32,
    tiles: i32,
    chests: i32,
    signs: i32,
    npcs: i32,
    entities: i32,
    footer: i32,
    unused_1: i32,
    unused_2: i32,
    unused_3: i32,
    tile_frame_important: Vec<u8>,
}

impl Header {
    fn write(&self, f: &mut File) -> Result<(), io::Error> {
        f.write_i32::<LE>(self.header)?;
        f.write_i32::<LE>(self.tiles)?;
        f.write_i32::<LE>(self.chests)?;
        f.write_i32::<LE>(self.signs)?;
        f.write_i32::<LE>(self.npcs)?;
        f.write_i32::<LE>(self.entities)?;
        f.write_i32::<LE>(self.footer)?;
        f.write_i32::<LE>(self.unused_1)?;
        f.write_i32::<LE>(self.unused_2)?;
        f.write_i32::<LE>(self.unused_3)?;
        Ok(())
    }
}

fn bit_index(bytes: &[u8], idx: usize) -> bool {
    let byte_idx = idx / 8;
    let bit_idx = (idx % 8) as u8;
    bsa(bytes[byte_idx], bit_idx)
}

fn read_offsets(f: &mut File) -> Result<Header, Box<Error>> {
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
    let header = f.read_i32::<LE>()?;
    let tiles = f.read_i32::<LE>()?;
    let chests = f.read_i32::<LE>()?;
    let signs = f.read_i32::<LE>()?;
    let npcs = f.read_i32::<LE>()?;
    let entities = f.read_i32::<LE>()?;
    let footer = f.read_i32::<LE>()?;
    let unused_1 = f.read_i32::<LE>()?;
    let unused_2 = f.read_i32::<LE>()?;
    let unused_3 = f.read_i32::<LE>()?;
    let n_tile_frame_important = f.read_i16::<LE>()? as usize;
    let mut tile_frame_important = vec![0; n_tile_frame_important];
    f.read_exact(&mut tile_frame_important)?;
    Ok(Header {
        header,
        tiles,
        chests,
        signs,
        npcs,
        entities,
        footer,
        unused_1,
        unused_2,
        unused_3,
        tile_frame_important,
    })
}

const ITEMS_PER_CHEST: i16 = 40;
const OFFSET_TABLE_OFFSET: u64 = 0x1A;

// bit set at
fn bsa(byte: u8, idx: u8) -> bool {
    byte & (1 << idx) != 0
}

#[test]
fn test_bsa() {
    assert!(bsa(0b0000_0001, 0));
    assert!(!bsa(0b0000_0000, 0));
    assert!(bsa(0b0000_0010, 1));
    assert!(bsa(0b1000_0010, 7));
}

#[derive(Debug, Clone, Copy)]
pub enum ChestType {
    Plain,
    Gold,
    Skyware,
    Ice,
    Granite,
    Marble,
    Mushroom,
    RichMahogany,
    Ivy,
    Water,
    WebCovered,
    LockedGold,
    LockedShadow,
    LockedCorruption,
    LockedCrimson,
    LockedHallowed,
    LockedJungle,
    LockedFrozen,
    Lihzahrd,
}

impl ChestType {
    fn from_frame_x(frame_x: i16) -> Option<Self> {
        use self::ChestType::*;
        Some(match frame_x {
            0 => Plain,
            36 => Gold,
            72 => LockedGold,
            144 => LockedShadow,
            288 => RichMahogany,
            360 => Ivy,
            396 => Ice,
            468 => Skyware,
            540 => WebCovered,
            576 => Lihzahrd,
            612 => Water,
            828 => LockedJungle,
            864 => LockedCorruption,
            900 => LockedCrimson,
            936 => LockedHallowed,
            972 => LockedFrozen,
            1152 => Mushroom,
            1800 => Granite,
            1836 => Marble,
            _ => return None,
        })
    }
}

fn load_chest_types(
    f: &mut File,
    w: u16,
    h: u16,
    tile_frame_important: &[u8],
) -> Result<HashMap<(u16, u16), ChestType>, Box<Error>> {
    let mut chest_types = HashMap::new();
    let mut i = 0;
    let len: usize = w as usize * h as usize;
    while i < len {
        let flags1 = f.read_u8()?;
        let flags2_present = bsa(flags1, 0);
        let not_air = bsa(flags1, 1);
        let has_wall = bsa(flags1, 2);
        let liquid_type_lo = bsa(flags1, 3);
        let liquid_type_hi = bsa(flags1, 4);
        let long_type_id = bsa(flags1, 5);
        let rle_on = bsa(flags1, 6);
        let rle_on_long = bsa(flags1, 7);
        let flags3_present = if flags2_present {
            let flags2 = f.read_u8()?;
            bsa(flags2, 0)
        } else {
            false
        };
        let (tile_painted, wall_painted);
        if flags3_present {
            let flags3 = f.read_u8()?;
            tile_painted = bsa(flags3, 3);
            wall_painted = bsa(flags3, 4);
        } else {
            tile_painted = false;
            wall_painted = false;
        }
        if not_air {
            let tile_id = if long_type_id {
                f.read_u16::<LE>()?
            } else {
                u16::from(f.read_u8()?)
            };
            assert!(tile_id < 500);
            if bit_index(tile_frame_important, tile_id as usize) {
                let frame_x = f.read_i16::<LE>()?;
                let frame_y = f.read_i16::<LE>()?;
                if tile_id == 21 {
                    let x = (i / h as usize) as u16;
                    let y = (i % h as usize) as u16;
                    if frame_y == 0 {
                        if let Some(type_) = ChestType::from_frame_x(frame_x) {
                            chest_types.insert((x, y), type_);
                        }
                    }
                }
            }
        }
        if tile_painted {
            let _paint = f.read_u8()?;
        }
        if has_wall {
            let _wall = f.read_u8()?;
            if wall_painted {
                let _wall_paint = f.read_u8()?;
            }
        }
        if liquid_type_lo || liquid_type_hi {
            let _liquid_volume = f.read_u8()?;
        }
        if rle_on || rle_on_long {
            let rle: u16 = if rle_on_long {
                f.read_u16::<LE>()?
            } else {
                u16::from(f.read_u8()?)
            };
            i += rle as usize;
        }
        i += 1;
    }
    Ok(chest_types)
}

impl World {
    pub fn load(path: &str) -> Result<Self, Box<Error>> {
        let mut f = File::open(path)?;
        let header = read_offsets(&mut f)?;
        f.seek(SeekFrom::Start(header.npcs as u64))?;
        let mut npcs = Vec::new();
        while let Some(npc) = read_npc(&mut f)? {
            npcs.push(npc);
        }
        f.seek(SeekFrom::Start(header.header as u64))?;
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
        let height = f.read_i32::<LE>()?;
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
        let spawn_x = f.read_i32::<LE>()?;
        let spawn_y = f.read_i32::<LE>()?;
        let surface_y = f.read_f64::<LE>()?;
        f.seek(SeekFrom::Start(header.tiles as u64))?;
        let chest_types = load_chest_types(
            &mut f,
            width as u16,
            height as u16,
            &header.tile_frame_important,
        )?;
        f.seek(SeekFrom::Start(header.chests as u64))?;
        let n_chests = f.read_i16::<LE>()?;
        let items_per_chest = f.read_i16::<LE>()?;
        if items_per_chest != ITEMS_PER_CHEST {
            return Err(format!("Unsupported items per chest: {}", items_per_chest).into());
        }
        let mut chests = Vec::new();
        for _ in 0..n_chests {
            chests.push(Chest::read(&mut f)?);
        }
        Ok(Self {
            npcs,
            chests,
            width: width as u16,
            surface_y,
            seed,
            spawn_x,
            spawn_y,
            chest_types,
        })
    }
    pub fn tile_to_gps_pos(self_width: u16, self_surface_y: f64, x: u16, y: u16) -> GpsPos {
        let raw_x = i32::from(x) * 2 - i32::from(self_width);
        let raw_y = self_surface_y * 2.0 - f64::from(y) * 2.0;
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
        f.seek(SeekFrom::Start(offsets.npcs as u64))?;
        for npc in &self.npcs {
            write_npc(&mut f, npc)?;
        }
        Ok(())
    }
    pub fn patch_chests(&self, file_path: &str) -> Result<(), Box<Error>> {
        use std::fs::OpenOptions;
        let mut f = OpenOptions::new().read(true).write(true).open(file_path)?;
        let mut offsets = read_offsets(&mut f)?;
        // Save the contents after chests into a buffer to write back later
        f.seek(SeekFrom::Start(offsets.signs as u64))?;
        let mut rest_buf = Vec::new();
        f.read_to_end(&mut rest_buf)?;
        f.seek(SeekFrom::Start(offsets.chests as u64))?;
        self.write_chests(&mut f)?;
        let new_signs_offset = f.seek(SeekFrom::Current(0))?;
        // Write back everything after chests
        f.write_all(&rest_buf)?;
        let offs_diff = new_signs_offset as i32 - offsets.signs as i32;
        f.seek(SeekFrom::Start(OFFSET_TABLE_OFFSET))?;
        offsets.signs += offs_diff;
        offsets.npcs += offs_diff;
        offsets.entities += offs_diff;
        offsets.footer += offs_diff;
        offsets.unused_1 += offs_diff;
        offsets.unused_2 += offs_diff;
        offsets.unused_3 += offs_diff;
        offsets.write(&mut f)?;
        Ok(())
    }
    fn write_chests(&self, f: &mut File) -> Result<(), Box<Error>> {
        f.write_i16::<LE>(self.chests.len() as i16)?;
        f.write_i16::<LE>(ITEMS_PER_CHEST)?;
        for chest in &self.chests {
            chest.write(f)?;
        }
        Ok(())
    }
    pub fn width(&self) -> u16 {
        self.width
    }
    pub fn surface_y(&self) -> f64 {
        self.surface_y
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
            self.x_offset, xside, self.y_offset, yside
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
    pub x: u16,
    pub y: u16,
    pub name: String,
    pub items: [Item; CHEST_MAX_ITEMS as usize],
}

impl Chest {
    fn read(f: &mut File) -> io::Result<Self> {
        let x = f.read_i32::<LE>()? as u16;
        let y = f.read_i32::<LE>()? as u16;
        let name = read_string(f)?;
        let mut items: [Item; CHEST_MAX_ITEMS as usize] = unsafe { ::std::mem::uninitialized() };
        for item in &mut items[..] {
            *item = Item::read(f)?;
        }
        Ok(Self { name, x, y, items })
    }
    fn write(&self, f: &mut File) -> io::Result<()> {
        f.write_i32::<LE>(i32::from(self.x))?;
        f.write_i32::<LE>(i32::from(self.y))?;
        write_string(f, &self.name)?;
        for item in self.items.iter() {
            item.write(f)?;
        }
        Ok(())
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

#[derive(Default)]
pub struct Item {
    pub stack: u16,
    pub id: i32,
    pub prefix_id: u8,
}

impl Item {
    fn read(f: &mut File) -> io::Result<Self> {
        let stack = f.read_u16::<LE>()?;
        if stack == 0 {
            Ok(Self::default())
        } else {
            let id = f.read_i32::<LE>()?;
            let prefix_id = f.read_u8()?;
            Ok(Self {
                stack,
                id,
                prefix_id,
            })
        }
    }
    fn write(&self, f: &mut File) -> io::Result<()> {
        f.write_u16::<LE>(self.stack)?;
        if self.stack != 0 {
            f.write_i32::<LE>(self.id)?;
            f.write_u8(self.prefix_id)?;
        }
        Ok(())
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
