use {
    directories::ProjectDirs,
    egui_file_dialog::FileDialog,
    egui_macroquad::{EguiMqInteg, macroquad},
    macroquad::prelude::*,
    recently_used_list::RecentlyUsedList,
    serde::{Deserialize, Serialize},
    std::{
        fs::File,
        ops::Add,
        path::{Path, PathBuf},
    },
    terraria_strings::ItemIdMap,
    terraria_wld::{BaseHeader, Chest, Header, Tile},
};

mod ui;

#[derive(Serialize, Deserialize, Default)]
struct Config {
    recent_files: RecentlyUsedList<PathBuf>,
    #[serde(default)]
    load_most_recent: bool,
    #[serde(default)]
    draw_center_marker: bool,
    #[serde(default)]
    load_tiles_at_start: bool,
}

impl Config {
    fn load_or_default() -> anyhow::Result<Self> {
        let cfg_path = cfg_path();
        if cfg_path.exists() {
            let text = std::fs::read_to_string(&cfg_path)?;
            Ok(serde_json::from_str(&text)?)
        } else {
            Ok(Default::default())
        }
    }
    fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(cfg_path().parent().unwrap())?;
        Ok(std::fs::write(
            cfg_path(),
            serde_json::to_string_pretty(self)?,
        )?)
    }
}

fn cfg_path() -> PathBuf {
    let proj_dir = ProjectDirs::from("", "crumblingstatue", "wldview").unwrap();

    proj_dir.config_dir().join("wldview.json")
}

/// World data without the tiles
struct WorldBase {
    base_header: BaseHeader,
    header: Header,
    chests: Vec<Chest>,
    file: File,
}

#[derive(Default)]
struct UiState {
    selected_chest: Option<usize>,
    loading_tiles: bool,
    hide_ui: bool,
    file_dia: FileDialog,
    egui_wants_pointer: bool,
}

struct ViewerState {
    cam_x: f32,
    cam_y: f32,
    tile_x: f32,
    tile_y: f32,
    scale: u8,
    map_tex: Option<Texture2D>,
}

impl Default for ViewerState {
    fn default() -> Self {
        Self {
            cam_x: 0.0,
            cam_y: 0.0,
            tile_x: 0.0,
            tile_y: 0.0,
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

#[macroquad::main("egui with macroquad")]
async fn main() -> anyhow::Result<()> {
    let mut cfg = Config::load_or_default()?;
    prevent_quit();
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
    let mut egui_mq = EguiMqInteg::new();
    loop {
        clear_background(BLACK);

        // Process keys, mouse etc.

        if let Some(world_base) = &mut world.base {
            if let Some(tex) = &viewer.map_tex {
                let header = &world_base.header;
                draw_texture_ex(
                    tex,
                    viewer.cam_x,
                    viewer.cam_y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(
                            header.width as f32 * viewer.scale as f32,
                            header.height as f32 * viewer.scale as f32,
                        )),
                        source: None,
                        rotation: 0.0,
                        flip_x: false,
                        flip_y: false,
                        pivot: None,
                    },
                );
            } else if let Ok((tiles_, img)) = receiver.try_recv() {
                world.tiles = tiles_;
                let tex = Texture2D::from_image(&img);
                tex.set_filter(FilterMode::Nearest);
                viewer.map_tex = Some(tex);
                ui_state.loading_tiles = false;
            }
        }

        let mp = mouse_position();
        let tile_x = f32::floor(mp.0 / viewer.scale as f32 - viewer.cam_x / viewer.scale as f32);
        let tile_y = f32::floor(mp.1 / viewer.scale as f32 - viewer.cam_y / viewer.scale as f32);
        ui_state.egui_wants_pointer = false;

        if !ui_state.hide_ui {
            egui_mq.ui(|_, egui_ctx| {
                ui::egui_ui(
                    egui_ctx,
                    &mut world,
                    &mut viewer,
                    &mut ui_state,
                    &mut cfg,
                    &sender,
                );
            });
            egui_mq.draw();
        }

        if cfg.draw_center_marker {
            draw_line(
                screen_width() / 2.,
                0.,
                screen_width() / 2.,
                screen_height(),
                1.0,
                RED,
            );
            draw_line(
                0.,
                screen_height() / 2.,
                screen_width(),
                screen_height() / 2.,
                1.0,
                RED,
            );
        }

        if is_key_pressed(KeyCode::F12) {
            ui_state.hide_ui ^= true;
        }

        if is_key_pressed(KeyCode::KpAdd) {
            viewer.cam_x *= 2.;
            viewer.cam_x -= screen_width() / 2.;
            viewer.cam_y *= 2.;
            viewer.cam_y -= screen_height() / 2.;
            viewer.scale *= 2;
        }

        if is_key_pressed(KeyCode::KpSubtract) && viewer.scale > 1 {
            viewer.cam_x += screen_width() / 2.;
            viewer.cam_x /= 2.;
            viewer.cam_y += screen_height() / 2.;
            viewer.cam_y /= 2.;
            viewer.scale /= 2;
        }

        if !ui_state.egui_wants_pointer && is_mouse_button_pressed(MouseButton::Left) {
            ui_state.selected_chest = None;
            if let Some(world_base) = &world.base {
                for (i, chest) in world_base.chests.iter().enumerate() {
                    if rect_contains_point(chest.x, chest.y, 2, 2, tile_x as u16, tile_y as u16) {
                        ui_state.selected_chest = Some(i);
                    }
                }
            }
        }

        let speed = 16.0;

        if is_key_down(KeyCode::Left) {
            viewer.cam_x += speed;
        }
        if is_key_down(KeyCode::Right) {
            viewer.cam_x -= speed;
        }
        if is_key_down(KeyCode::Up) {
            viewer.cam_y += speed;
        }
        if is_key_down(KeyCode::Down) {
            viewer.cam_y -= speed;
        }

        if let Some(world_base) = &world.base {
            viewer.cam_x = clamp(
                viewer.cam_x,
                -(world_base.header.width as f32 * viewer.scale as f32) + screen_width() / 2.,
                screen_width() / 2.,
            );
            viewer.cam_y = clamp(
                viewer.cam_y,
                -(world_base.header.height as f32 * viewer.scale as f32) + screen_height() / 2.,
                screen_height() / 2.,
            );
        }

        if is_quit_requested() {
            cfg.save()?;
            return Ok(());
        }

        next_frame().await;
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
            0 => BROWN,
            1 => GRAY,
            2 => GREEN,
            3 => Color::from_rgba(204, 255, 0, 255),
            4 => RED,
            5 => BROWN,
            // Iron/copper/etc
            6..=9 => ORANGE,
            // Platform
            19 => BROWN,
            // Pots
            28 => Color::from_rgba(144, 69, 0, 255),
            // Wood
            30 => BROWN,
            // Clay
            40 => Color::from_rgba(154, 73, 40, 255),
            // Dungeon brick
            43 | 44 => Color::from_rgba(131, 0, 178, 255),
            // Chest
            21 | 467 => YELLOW,
            // Cobweb
            51 => Color::from_rgba(188, 175, 174, 255),
            // Vine
            52 => GREEN,
            // Sand
            53 | 112 | 116 | 234 => Color::from_rgba(234, 213, 0, 255),
            // Ash
            57 => DARKGRAY,
            // Hellstone
            58 => Color::from_rgba(168, 53, 17, 255),
            // Mud
            59 => Color::from_rgba(57, 36, 10, 255),
            // Jungle grass
            60 => DARKGREEN,
            // Jungle vine
            62 => DARKGREEN,
            // Glowing mushroom stuff
            70..=72 => Color::from_rgba(56, 230, 255, 255),
            // Hallowed grass
            109 => Color::from_rgba(135, 234, 193, 255),
            // Hallowed vine
            115 => Color::from_rgba(45, 133, 126, 255),
            // Pearlstone
            117 => Color::from_rgba(162, 117, 137, 255),
            // Wooden beam
            124 => BROWN,
            // Snow
            147 => Color::from_rgba(202, 234, 252, 255),
            // Ice
            161 | 162 => Color::from_rgba(151, 165, 220, 255),
            // Pink ice
            164 => Color::from_rgba(194, 165, 220, 255),
            // Stones, tiny rubble, etc
            185 => Color::from_rgba(153, 125, 99, 255),
            // Living wood
            191 => BROWN,
            // Living leaf
            192 => GREEN,
            // Crimson grass
            199 => Color::from_rgba(220, 89, 69, 255),
            // Crimstone
            203 => Color::from_rgba(127, 15, 0, 255),
            // Crimson vines
            205 => Color::from_rgba(176, 53, 30, 255),
            // Hive
            225 => ORANGE,
            // Temple bricks
            226 => Color::from_rgba(250, 95, 0, 255),
            // Marble
            367 => Color::from_rgba(172, 189, 191, 255),
            // Granite
            368 => Color::from_rgba(15, 18, 34, 255),
            // Living mahogany
            383 => BROWN,
            // Living mahogany leaf
            384 => GREEN,
            // Sandstone
            396 => Color::from_rgba(197, 116, 0, 255),
            // Hardened sand
            397 => Color::from_rgba(192, 160, 19, 255),
            // Desert fossil
            404 => BROWN,
            _ => MAGENTA,
        })
    } else if let Some(liq) = tile.liquid {
        Some(match liq {
            terraria_wld::Liquid::Water => BLUE,
            terraria_wld::Liquid::Lava => RED,
            terraria_wld::Liquid::Honey => Color::from_rgba(216, 167, 0, 255),
        })
    } else {
        tile.back.map(|back| match back {
            // Stone
            1 => DARKGRAY,
            // Dirt
            2 => DARKBROWN,
            // Wood
            4 | 78 => DARKBROWN,
            // Dungeon
            7 | 8 | 9 | 17 | 18 | 19 | 94..=105 => DARKPURPLE,
            // Crimson
            83 => Color::from_rgba(59, 8, 8, 255),
            _ => Color::from_rgba(180, 0, 180, 255),
        })
    }
}
