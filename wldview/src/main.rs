use {
    crate::{config::Config, ui::UiState},
    egui_frogquad::{
        EguiFrogquad,
        frogquad::{
            self, Color,
            glam::{Vec2, vec2},
            input::{KeyCode, MouseButton},
            texture::{DrawTextureParams, FilterMode, Image, Texture2D},
        },
    },
    std::{fs::File, ops::Add, path::Path},
    terraria_strings::ItemIdMap,
    terraria_wld::{BaseHeader, Chest, Header, Tile},
};

mod config;
mod ui;

/// World data without the tiles
struct WorldBase {
    base_header: BaseHeader,
    header: Header,
    chests: Vec<Chest>,
    file: File,
}

struct ViewerState {
    cam: Vec2,
    pointed_tile: Vec2,
    scale: u8,
    map_tex: Option<Texture2D>,
}

impl Default for ViewerState {
    fn default() -> Self {
        Self {
            cam: Vec2::default(),
            pointed_tile: Vec2::default(),
            scale: 1,
            map_tex: None,
        }
    }
}

pub struct WorldState {
    base: Option<WorldBase>,
    tiles: Vec<Tile>,
    item_id_map: ItemIdMap,
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            base: None,
            tiles: Vec::default(),
            item_id_map: terraria_strings::item_ids(),
        }
    }
}

struct WldViewApp {
    efrog: EguiFrogquad,
    cfg: Config,
    viewer: ViewerState,
    world: WorldState,
    ui_state: UiState,
    receiver: std::sync::mpsc::Receiver<(Vec<Tile>, Image)>,
    sender: std::sync::mpsc::Sender<(Vec<Tile>, Image)>,
}

impl WldViewApp {
    fn new(fc: &mut frogquad::FrogCx) -> anyhow::Result<Self> {
        let mut cfg = Config::load_or_default()?;
        let mut viewer = ViewerState::default();
        let mut world = WorldState::default();
        if cfg.load_most_recent
            && let Some(most_recent) = cfg.recent_files.most_recent().cloned()
            && load_world(&most_recent, &mut world, &mut viewer.map_tex)
        {
            cfg.recent_files.use_(most_recent);
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut ui_state = UiState::default();
        if let Some(world_base) = &world.base
            && cfg.load_tiles_at_start
        {
            let base_header = world_base.base_header.clone();
            let header = world_base.header.clone();
            let file = world_base.file.try_clone().unwrap();
            let sender = sender.clone();
            std::thread::spawn(move || {
                let ret_val = load_tiles(&file, &base_header, &header);
                sender.send(ret_val).unwrap();
            });
            ui_state.loading_tiles = true;
        }
        let efrog = EguiFrogquad::new(fc);
        Ok(Self {
            efrog,
            cfg,
            viewer,
            world,
            ui_state,
            sender,
            receiver,
        })
    }
}

impl frogquad::App for WldViewApp {
    fn update(&mut self, fc: &mut frogquad::FrogCx) {
        fc.clear_background(Color::BLACK);

        // Process keys, mouse etc.

        if let Some(world_base) = &mut self.world.base {
            if let Some(tex) = &self.viewer.map_tex {
                let header = &world_base.header;
                fc.draw_texture_ex(
                    tex,
                    self.viewer.cam.x,
                    self.viewer.cam.y,
                    Color::WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(
                            header.width as f32 * self.viewer.scale as f32,
                            header.height as f32 * self.viewer.scale as f32,
                        )),
                        source: None,
                        rotation: 0.0,
                        flip_x: false,
                        flip_y: false,
                        pivot: None,
                    },
                );
            } else if let Ok((tiles_, img)) = self.receiver.try_recv() {
                self.world.tiles = tiles_;
                let tex = Texture2D::from_image(fc, &img);
                tex.set_filter(fc, FilterMode::Nearest);
                self.viewer.map_tex = Some(tex);
                self.ui_state.loading_tiles = false;
            }
        }

        let mp = Vec2::from(fc.mouse_position());
        self.viewer.pointed_tile =
            mp.to_tile(self.viewer.scale) - self.viewer.cam.to_tile(self.viewer.scale);
        self.ui_state.egui_wants_pointer = false;

        if self.cfg.draw_center_marker {
            fc.draw_line(
                fc.screen_width() / 2.,
                0.,
                fc.screen_width() / 2.,
                fc.screen_height(),
                1.0,
                Color::RED,
            );
            fc.draw_line(
                0.,
                fc.screen_height() / 2.,
                fc.screen_width(),
                fc.screen_height() / 2.,
                1.0,
                Color::RED,
            );
        }

        if fc.is_key_pressed(KeyCode::F12) {
            self.ui_state.hide_ui ^= true;
        }

        if fc.is_key_pressed(KeyCode::KpAdd) {
            self.viewer.cam.x *= 2.;
            self.viewer.cam.x -= fc.screen_width() / 2.;
            self.viewer.cam.y *= 2.;
            self.viewer.cam.y -= fc.screen_height() / 2.;
            self.viewer.scale *= 2;
        }

        if fc.is_key_pressed(KeyCode::KpSubtract) && self.viewer.scale > 1 {
            self.viewer.cam.x += fc.screen_width() / 2.;
            self.viewer.cam.x /= 2.;
            self.viewer.cam.y += fc.screen_height() / 2.;
            self.viewer.cam.y /= 2.;
            self.viewer.scale /= 2;
        }
        let (screen_width, screen_height) = (fc.screen_width(), fc.screen_height());
        if !self.ui_state.hide_ui {
            self.efrog.ui(fc, |_, ui| {
                ui::egui_ui(
                    ui,
                    &mut self.world,
                    &mut self.viewer,
                    &mut self.ui_state,
                    &mut self.cfg,
                    &self.sender,
                    screen_width,
                    screen_height,
                );
            });
            self.efrog.draw(fc);
        }

        if !self.ui_state.egui_wants_pointer && fc.is_mouse_button_pressed(MouseButton::Left) {
            self.ui_state.selected_chest = None;
            if let Some(world_base) = &self.world.base {
                for (i, chest) in world_base.chests.iter().enumerate() {
                    if rect_contains_point(
                        chest.x,
                        chest.y,
                        2,
                        2,
                        self.viewer.pointed_tile.x as u16,
                        self.viewer.pointed_tile.y as u16,
                    ) {
                        self.ui_state.selected_chest = Some(i);
                    }
                }
            }
        }

        let speed = 16.0;

        if fc.is_key_down(KeyCode::Left) {
            self.viewer.cam.x += speed;
        }
        if fc.is_key_down(KeyCode::Right) {
            self.viewer.cam.x -= speed;
        }
        if fc.is_key_down(KeyCode::Up) {
            self.viewer.cam.y += speed;
        }
        if fc.is_key_down(KeyCode::Down) {
            self.viewer.cam.y -= speed;
        }

        if let Some(world_base) = &self.world.base {
            self.viewer.cam.x = f32::clamp(
                self.viewer.cam.x,
                -(world_base.header.width as f32 * self.viewer.scale as f32)
                    + fc.screen_width() / 2.,
                fc.screen_width() / 2.,
            );
            self.viewer.cam.y = f32::clamp(
                self.viewer.cam.y,
                -(world_base.header.height as f32 * self.viewer.scale as f32)
                    + fc.screen_height() / 2.,
                fc.screen_height() / 2.,
            );
        }

        if fc.is_quit_requested() {
            self.cfg.save().unwrap();
        }
    }
}

fn rect_contains_point<T: PartialOrd + Add<Output = T> + Copy>(
    rx: T,
    ry: T,
    rw: T,
    rh: T,
    px: T,
    py: T,
) -> bool {
    px >= rx && py >= ry && px < rx + rw && py < ry + rh
}

fn load_tiles(file: &File, base_header: &BaseHeader, header: &Header) -> (Vec<Tile>, Image) {
    let mut tiles = vec![Tile::default(); header.width as usize * header.height as usize];
    let mut image =
        Image::gen_image_color(header.width, header.height, Color::from_rgba(0, 0, 0, 0));
    let mut n_read = 0;
    terraria_wld::read_tiles(file, base_header, |tile, x, y| {
        tiles[y as usize * header.width as usize + x as usize] = tile;
        if let Some(color) = tile_color(&tile) {
            image.set_pixel(x as u32, y as u32, color);
        }
        n_read += 1;
    })
    .unwrap();
    assert_eq!(
        n_read,
        header.width as u32 * header.height as u32,
        "Didn't read correct number of tiles"
    );
    (tiles, image)
}

fn load_world(path: &Path, world: &mut WorldState, map_tex: &mut Option<Texture2D>) -> bool {
    // Reset some stuff when loading new world over an existing one
    world.tiles.clear();
    *map_tex = None;
    match terraria_wld::open(path) {
        Ok((file, base_header)) => {
            let header =
                terraria_wld::read_header(&file, base_header.offsets.header as u64).unwrap();
            let chests =
                terraria_wld::read_chests(&file, base_header.offsets.chests as u64).unwrap();
            world.base = Some(WorldBase {
                base_header,
                header,
                file,
                chests,
            });
            true
        }
        Err(e) => {
            eprintln!("Error loading world: {e}");
            false
        }
    }
}

fn tile_color(tile: &Tile) -> Option<Color> {
    if let Some(id) = tile.front {
        Some(match id {
            0 => Color::BROWN,
            1 => Color::GRAY,
            2 => Color::GREEN,
            3 => Color::from_rgba(204, 255, 0, 255),
            4 => Color::RED,
            5 => Color::BROWN,
            // Iron/copper/etc
            6..=9 => Color::ORANGE,
            // Platform
            19 => Color::BROWN,
            // Pots
            28 => Color::from_rgba(144, 69, 0, 255),
            // Wood
            30 => Color::BROWN,
            // Clay
            40 => Color::from_rgba(154, 73, 40, 255),
            // Dungeon brick
            43 | 44 => Color::from_rgba(131, 0, 178, 255),
            // Chest
            21 | 467 => Color::YELLOW,
            // Cobweb
            51 => Color::from_rgba(188, 175, 174, 255),
            // Vine
            52 => Color::GREEN,
            // Sand
            53 | 112 | 116 | 234 => Color::from_rgba(234, 213, 0, 255),
            // Ash
            57 => Color::DARKGRAY,
            // Hellstone
            58 => Color::from_rgba(168, 53, 17, 255),
            // Mud
            59 => Color::from_rgba(57, 36, 10, 255),
            // Jungle grass
            60 => Color::DARKGREEN,
            // Jungle vine
            62 => Color::DARKGREEN,
            // Glowing mushroom stuff
            70..=72 => Color::from_rgba(56, 230, 255, 255),
            // Hallowed grass
            109 => Color::from_rgba(135, 234, 193, 255),
            // Hallowed vine
            115 => Color::from_rgba(45, 133, 126, 255),
            // Pearlstone
            117 => Color::from_rgba(162, 117, 137, 255),
            // Wooden beam
            124 => Color::BROWN,
            // Snow
            147 => Color::from_rgba(202, 234, 252, 255),
            // Ice
            161 | 162 => Color::from_rgba(151, 165, 220, 255),
            // Pink ice
            164 => Color::from_rgba(194, 165, 220, 255),
            // Stones, tiny rubble, etc
            185 => Color::from_rgba(153, 125, 99, 255),
            // Living wood
            191 => Color::BROWN,
            // Living leaf
            192 => Color::GREEN,
            // Crimson grass
            199 => Color::from_rgba(220, 89, 69, 255),
            // Crimstone
            203 => Color::from_rgba(127, 15, 0, 255),
            // Crimson vines
            205 => Color::from_rgba(176, 53, 30, 255),
            // Hive
            225 => Color::ORANGE,
            // Temple bricks
            226 => Color::from_rgba(250, 95, 0, 255),
            // Marble
            367 => Color::from_rgba(172, 189, 191, 255),
            // Granite
            368 => Color::from_rgba(15, 18, 34, 255),
            // Living mahogany
            383 => Color::BROWN,
            // Living mahogany leaf
            384 => Color::GREEN,
            // Sandstone
            396 => Color::from_rgba(197, 116, 0, 255),
            // Hardened sand
            397 => Color::from_rgba(192, 160, 19, 255),
            // Desert fossil
            404 => Color::BROWN,
            _ => Color::MAGENTA,
        })
    } else if let Some(liq) = tile.liquid {
        Some(match liq {
            terraria_wld::Liquid::Water => Color::BLUE,
            terraria_wld::Liquid::Lava => Color::RED,
            terraria_wld::Liquid::Honey => Color::from_rgba(216, 167, 0, 255),
        })
    } else {
        tile.back.map(|back| match back {
            // Stone
            1 => Color::DARKGRAY,
            // Dirt
            2 => Color::DARKBROWN,
            // Wood
            4 | 78 => Color::DARKBROWN,
            // Dungeon
            7 | 8 | 9 | 17 | 18 | 19 | 94..=105 => Color::DARKPURPLE,
            // Crimson
            83 => Color::from_rgba(59, 8, 8, 255),
            _ => Color::from_rgba(180, 0, 180, 255),
        })
    }
}

trait MqVec2Ext {
    fn to_pix(&self, scale: u8) -> Self;
    fn to_tile(&self, scale: u8) -> Self;
}

impl MqVec2Ext for Vec2 {
    fn to_tile(&self, scale: u8) -> Self {
        let scale = scale as f32;
        Self {
            x: f32::floor(self.x / scale),
            y: f32::floor(self.y / scale),
        }
    }
    fn to_pix(&self, scale: u8) -> Self {
        let scale = scale as f32;
        Self {
            x: f32::floor(self.x * scale),
            y: f32::floor(self.y * scale),
        }
    }
}

fn main() {
    frogquad::start("Wldview", |fc| WldViewApp::new(fc).unwrap());
}
