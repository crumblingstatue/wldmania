use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, SeekFrom};
use std::path::Path;

pub struct Header {
    pub id: i32,
    pub bounds: Rect,
    pub name: String,
    pub height: u16,
    pub width: u16,
    pub surface_y: f64,
    pub seed: String,
    pub spawn_x: i32,
    pub spawn_y: i32,
    pub generator_version: i64,
    pub guid: [u8; 16],
    pub game_mode: i32,
}

#[derive(Debug)]
pub struct WorldFile {
    file: File,
    pub base_header: BaseHeader,
}

#[derive(Debug)]
pub struct Rect {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl WorldFile {
    pub fn open(path: &Path, write: bool) -> Result<Self, Box<dyn Error>> {
        use std::fs::OpenOptions;
        let mut file = OpenOptions::new().read(true).write(write).open(path)?;
        let base_header = read_base_header(&mut file)?;
        Ok(Self { file, base_header })
    }
    pub fn read_npcs(&mut self) -> Result<Vec<Npc>, Box<dyn Error>> {
        self.file
            .seek(SeekFrom::Start(self.base_header.offsets.npcs as u64))?;
        let mut npcs = Vec::new();
        while let Some(npc) = read_npc(&mut self.file)? {
            npcs.push(npc);
        }
        Ok(npcs)
    }
    pub fn read_header(&mut self) -> Result<Header, Box<dyn Error>> {
        let f = &mut self.file;
        f.seek(SeekFrom::Start(self.base_header.offsets.header as u64))?;
        let name = read_string(f)?;
        let seed = read_string(f)?;
        let generator_version = f.read_i64::<LE>()?;
        let mut guid = [0u8; 16];
        f.read_exact(&mut guid)?;
        let id = f.read_i32::<LE>()?;
        let bounds = read_rect(f)?;
        let height = f.read_i32::<LE>()?;
        let width = f.read_i32::<LE>()?;
        let game_mode = f.read_i32::<LE>()?;
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
        Ok(Header {
            width: width as u16,
            height: height as u16,
            surface_y,
            seed,
            spawn_x,
            spawn_y,
            name,
            generator_version,
            id,
            bounds,
            guid,
            game_mode,
        })
    }
    pub fn read_chest_types(&mut self) -> Result<HashMap<(u16, u16), ChestType>, Box<dyn Error>> {
        self.file
            .seek(SeekFrom::Start(self.base_header.offsets.tiles as u64))?;
        let chest_types = self.load_chest_types()?;
        Ok(chest_types)
    }
    pub fn read_chests(&mut self) -> Result<Vec<Chest>, Box<dyn Error>> {
        let f = &mut self.file;
        f.seek(SeekFrom::Start(self.base_header.offsets.chests as u64))?;
        let n_chests = f.read_i16::<LE>()?;
        let items_per_chest = f.read_i16::<LE>()?;
        if items_per_chest != ITEMS_PER_CHEST {
            return Err(format!("Unsupported items per chest: {}", items_per_chest).into());
        }
        let mut chests = Vec::new();
        for _ in 0..n_chests {
            chests.push(Chest::read(f)?);
        }
        Ok(chests)
    }
    pub fn write_npcs(&mut self, npcs: &[Npc]) -> Result<(), Box<dyn Error>> {
        let f = &mut self.file;
        f.seek(SeekFrom::Start(self.base_header.offsets.npcs as u64))?;
        for npc in npcs {
            write_npc(f, npc)?;
        }
        Ok(())
    }
    pub fn write_chests(&mut self, chests: &[Chest]) -> Result<(), Box<dyn Error>> {
        // Save the contents after chests into a buffer to write back later
        self.file
            .seek(SeekFrom::Start(self.base_header.offsets.signs as u64))?;
        let mut rest_buf = Vec::new();
        self.file.read_to_end(&mut rest_buf)?;
        self.file
            .seek(SeekFrom::Start(self.base_header.offsets.chests as u64))?;
        self.write_chests_inner(chests)?;
        let new_signs_offset = self.file.seek(SeekFrom::Current(0))?;
        // Write back everything after chests
        self.file.write_all(&rest_buf)?;
        let offs_diff = new_signs_offset as i32 - self.base_header.offsets.signs as i32;
        self.file.seek(SeekFrom::Start(OFFSET_TABLE_OFFSET))?;
        self.base_header.offsets.signs += offs_diff;
        self.base_header.offsets.npcs += offs_diff;
        self.base_header.offsets.entities += offs_diff;
        self.base_header.offsets.footer += offs_diff;
        self.base_header.offsets.unused_1 += offs_diff;
        self.base_header.offsets.unused_2 += offs_diff;
        self.base_header.offsets.unused_3 += offs_diff;
        self.base_header.write(&mut self.file)?;
        Ok(())
    }
    fn write_chests_inner(&mut self, chests: &[Chest]) -> Result<(), Box<dyn Error>> {
        let f = &mut self.file;
        f.write_i16::<LE>(chests.len() as i16)?;
        f.write_i16::<LE>(ITEMS_PER_CHEST)?;
        for chest in chests {
            chest.write(f)?;
        }
        Ok(())
    }
    /// New, more accurate version
    pub fn read_tiles<TC>(&mut self, mut tile_callback: TC) -> Result<(), Box<dyn Error>>
    where
        TC: FnMut(/*tile: */ Tile, /*x: */ u16, /*y: */ u16),
    {
        let basic_info = self.read_header()?;
        self.file
            .seek(SeekFrom::Start(self.base_header.offsets.tiles as u64))?;
        let w = basic_info.width;
        let h = basic_info.height;
        for x in 0..w {
            let mut y = 0;
            while y < h {
                let (tile, rle_repeat) =
                    read_tile(&mut self.file, &self.base_header.tile_frame_important)?;
                tile_callback(tile, x, y);
                for i in 0..rle_repeat {
                    tile_callback(tile, x, y + 1 + i);
                }
                y += rle_repeat + 1;
            }
        }
        Ok(())
    }
    fn load_chest_types(&mut self) -> Result<HashMap<(u16, u16), ChestType>, Box<dyn Error>> {
        let mut chest_types = HashMap::new();
        self.read_tiles(|tile, x, y| {
            if tile.front == Some(21) {
                let tfo = tile.frame.unwrap();
                if tfo.y == 0 {
                    let type_ = ChestType::from_frame_x(tfo.x);
                    chest_types.insert((x, y), type_);
                }
            } else if tile.front == Some(88)
            /* dresser */
            {
                let tfo = tile.frame.unwrap();
                chest_types.insert((x, y), ChestType::UnknownDresser(tfo.x));
            }
        })?;
        Ok(chest_types)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Tile {
    pub front: Option<u16>,
    pub back: Option<u16>,
    pub liquid: Option<Liquid>,
    pub frame: Option<TileFrameOffset>,
}

#[derive(Clone, Copy, Debug)]
pub enum Liquid {
    Water,
    Lava,
    Honey,
}

fn read_tile(file: &mut File, tile_frame_important: &[u8]) -> io::Result<(Tile, u16)> {
    let flags1 = file.read_u8()?;
    let flags2;
    let mut flags3 = 0;
    if flags1.nth_bit_set(0) {
        flags2 = file.read_u8()?;
        if flags2.nth_bit_set(0) {
            flags3 = file.read_u8()?;
        }
    }
    let mut tile_frame = None;
    let front = if flags1.nth_bit_set(1) {
        let mut type_inner = file.read_u8()? as u16;
        if flags1.nth_bit_set(5) {
            type_inner |= (file.read_u8()? as u16) << 8;
        }
        if tile_frame_important.nth_bit_set(type_inner as usize) {
            tile_frame = Some(TileFrameOffset {
                x: file.read_u16::<LE>()?,
                y: file.read_u16::<LE>()?,
            })
        }
        if flags3.nth_bit_set(3) {
            let _color = file.read_u8()?;
        }
        Some(type_inner)
    } else {
        None
    };
    let mut back = None;
    if flags1.nth_bit_set(2) {
        back = Some(file.read_u8()? as u16);
        if flags3.nth_bit_set(4) {
            let _wall_color = file.read_u8()?;
        }
    }
    let liquid = match flags1 & 0b00011000 {
        0b00000000 => None,
        liquid => Some({
            let _liquid_amount = file.read_u8()?;
            match liquid {
                0b00001000 => Liquid::Water,
                0b00010000 => Liquid::Lava,
                0b00011000 => Liquid::Honey,
                _ => unreachable!(),
            }
        }),
    };
    if flags3.nth_bit_set(6) {
        *back.as_mut().unwrap() |= (file.read_u8()? as u16) << 8;
    }
    let mut rle = 0;
    match flags1 >> 6 {
        0 => {}
        1 => rle = file.read_u8()? as u16,
        2 => rle = file.read_u16::<LE>()?,
        etc => println!("Invalid rle flag ({})", etc),
    }
    Ok((
        Tile {
            front,
            back,
            liquid,
            frame: tile_frame,
        },
        rle,
    ))
}

fn read_rect(f: &mut File) -> io::Result<Rect> {
    Ok(Rect {
        left: f.read_i32::<LE>()?,
        right: f.read_i32::<LE>()?,
        top: f.read_i32::<LE>()?,
        bottom: f.read_i32::<LE>()?,
    })
}

/// Contains the offsets of different sections, and some other base information.
#[derive(Debug)]
pub struct BaseHeader {
    pub version: i32,
    pub offsets: Offsets,
    pub times_saved: u32,
    pub is_favorite: u64,
    pub tile_frame_important: Vec<u8>,
}

/// The offsets of different sections
#[derive(Debug)]
pub struct Offsets {
    pub header: i32,
    pub tiles: i32,
    pub chests: i32,
    pub signs: i32,
    pub npcs: i32,
    pub entities: i32,
    pub footer: i32,
    pub unused_1: i32,
    pub unused_2: i32,
    pub unused_3: i32,
    pub unknown_4: i32,
}

impl BaseHeader {
    fn write(&self, f: &mut File) -> Result<(), io::Error> {
        f.write_i32::<LE>(self.offsets.header)?;
        f.write_i32::<LE>(self.offsets.tiles)?;
        f.write_i32::<LE>(self.offsets.chests)?;
        f.write_i32::<LE>(self.offsets.signs)?;
        f.write_i32::<LE>(self.offsets.npcs)?;
        f.write_i32::<LE>(self.offsets.entities)?;
        f.write_i32::<LE>(self.offsets.footer)?;
        f.write_i32::<LE>(self.offsets.unused_1)?;
        f.write_i32::<LE>(self.offsets.unused_2)?;
        f.write_i32::<LE>(self.offsets.unused_3)?;
        f.write_i32::<LE>(self.offsets.unknown_4)?;
        Ok(())
    }
}

fn read_base_header(f: &mut File) -> Result<BaseHeader, Box<dyn Error>> {
    let terraria_version = f.read_i32::<LE>()?;
    let mut magic = [0u8; 7];
    f.read_exact(&mut magic)?;
    if magic[..] != b"relogic"[..] {
        return Err("Not a valid terraria map file.".into());
    }
    let filetype = f.read_u8()?;
    if filetype != 2 {
        return Err(format!("Unsupported filetype: {}", filetype).into());
    }
    let times_saved = f.read_u32::<LE>()?;
    let is_favorite = f.read_u64::<LE>()?;
    let n_offsets = f.read_u16::<LE>()?;
    if n_offsets != 11 {
        return Err(format!("Unsupported number of offsets: {}", n_offsets).into());
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
    let unknown_4 = f.read_i32::<LE>()?;
    let n_tile_frame_important = f.read_u16::<LE>()? as usize;
    let mut tile_frame_important = vec![0; n_tile_frame_important];
    f.read_exact(&mut tile_frame_important)?;
    Ok(BaseHeader {
        offsets: Offsets {
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
            unknown_4,
        },
        tile_frame_important,
        times_saved,
        is_favorite,
        version: terraria_version,
    })
}

const ITEMS_PER_CHEST: i16 = 40;
const OFFSET_TABLE_OFFSET: u64 = 0x1A;

trait Bits {
    type Index;
    fn nth_bit_set(self, index: Self::Index) -> bool;
}

impl Bits for u8 {
    type Index = u8;
    fn nth_bit_set(self, index: u8) -> bool {
        self & (1 << index) != 0
    }
}

impl<'a> Bits for &'a [u8] {
    type Index = usize;
    fn nth_bit_set(self, index: usize) -> bool {
        let byte_idx = index / 8;
        let bit_idx = (index % 8) as u8;
        self[byte_idx].nth_bit_set(bit_idx)
    }
}

#[test]
fn test_bits_u8() {
    assert!(0b0000_0001.nth_bit_set(0));
    assert!(!0b0000_0000.nth_bit_set(0));
    assert!(0b0000_0010.nth_bit_set(1));
    assert!(0b1000_0010.nth_bit_set(7));
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
    UnknownChest(u16),
    UnknownDresser(u16),
}

impl ChestType {
    fn from_frame_x(frame_x: u16) -> Self {
        use self::ChestType::*;
        match frame_x {
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
            _ => UnknownChest(frame_x),
        }
    }
}

/// The offset of the subimage a tile has.
///
/// Terraria graphics are contained in texture atlases, which we'll call tile frames, because
/// that seems to be the term the Terraria code base is using.
/// These atlases contain multiple images, each representing a different piece of graphic.
/// To know what image to draw, we need to know the offset. In most cases, the offset can be
/// calculated on the fly when Terraria is running. For example dirt has a lot of different
/// sub-graphics, but every single graphic is just dirt. It doesn't matter what subimage it has,
/// it's just dirt. In this case, we'll say that the "tile frame is not important".
///
/// But there are cases where multiple kinds of tiles have the same tile id.
///
/// For example, all placed gems all have the tile id 178, but the different types of gems
/// have different tile frame offsets. If we didn't record the offsets, we wouldn't know what kind
/// of gem we're dealing with.
///
/// In these cases, we say that "the tile frame is important". I'm borrowing Terraria terminology
/// here.
///
/// Important tile frames are recorded along with the tile id, but only if the
/// tile frame is important.
/// Which tile frames are important are stored in an array called tile_frame_important in the .wld
/// file, which are read along with other metadata.
#[derive(Clone, Copy, Debug)]
pub struct TileFrameOffset {
    pub x: u16,
    pub y: u16,
}

impl Header {
    pub fn tile_to_gps_pos(&self, x: u16, y: u16) -> GpsPos {
        let raw_x = i32::from(x) * 2 - i32::from(self.width);
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
        let mut items = [Item::default(); CHEST_MAX_ITEMS as usize];
        for item in &mut items[..] {
            *item = Item::read(f)?;
        }
        Ok(Self { x, y, name, items })
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

#[derive(Default, Copy, Clone)]
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
