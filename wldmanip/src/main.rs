#![feature(let_chains, decl_macro)]

use std::{
    fs::File,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use egui_macroquad::{
    egui::{Grid, ScrollArea, Spinner, TopBottomPanel, Window},
    macroquad,
};

use macroquad::prelude::*;
use recently_used_list::RecentlyUsedList;
use serde::{Deserialize, Serialize};
use terraria_wld::{BaseHeader, Header, Tile};

#[derive(Serialize, Deserialize, Default)]
struct Config {
    recent_files: RecentlyUsedList<PathBuf>,
    #[serde(default)]
    load_most_recent: bool,
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
    let proj_dir = ProjectDirs::from("", "crumblingstatue", "wldmanip").unwrap();
    let cfg_path = proj_dir.config_dir().join("wldmanip.json");
    cfg_path
}

/// World data without the tiles
struct WorldBase {
    base_header: BaseHeader,
    header: Header,
    file: File,
}

#[macroquad::main("egui with macroquad")]
async fn main() -> anyhow::Result<()> {
    let mut world_base = None;
    let mut map_tex = None;
    let mut cfg = Config::load_or_default()?;
    let mut show_ui = true;
    let mut tiles = Vec::new();
    prevent_quit();
    if cfg.load_most_recent && let Some(most_recent) = cfg.recent_files.most_recent().cloned() && load_world(&most_recent, &mut world_base) {
        cfg.recent_files.use_(most_recent);
    }
    let mut cam_x = 0.0;
    let mut cam_y = 0.0;
    let mut scale = 1;
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut loading_tiles = false;
    loop {
        clear_background(BLACK);

        // Process keys, mouse etc.

        if let Some(world_base) = &mut world_base {
            if let Some(tex) = map_tex {
                let header = &world_base.header;
                draw_texture_ex(
                    tex,
                    cam_x,
                    cam_y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(
                            header.width as f32 * scale as f32,
                            header.height as f32 * scale as f32,
                        )),
                        source: None,
                        rotation: 0.0,
                        flip_x: false,
                        flip_y: false,
                        pivot: None,
                    },
                );
            } else if let Ok((tiles_, img)) = receiver.try_recv() {
                tiles = tiles_;
                let tex = Texture2D::from_image(&img);
                tex.set_filter(FilterMode::Nearest);
                map_tex = Some(tex);
                loading_tiles = false;
            }
        }

        let mp = mouse_position();
        let tile_x = f32::floor(mp.0 / scale as f32 - cam_x / scale as f32);
        let tile_y = f32::floor(mp.1 / scale as f32 - cam_y / scale as f32);

        if show_ui {
            egui_macroquad::ui(|egui_ctx| {
                TopBottomPanel::top("top_panel").show(egui_ctx, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                if load_world(&path, &mut world_base) {
                                    cfg.recent_files.use_(path);
                                }
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        let mut used = None;
                        for recent in cfg.recent_files.iter() {
                            if ui.button(recent.display().to_string()).clicked() {
                                load_world(recent, &mut world_base);
                                used = Some(recent.to_owned());
                                ui.close_menu();
                                break;
                            }
                        }
                        if let Some(used) = used {
                            cfg.recent_files.use_(used);
                        }
                        ui.separator();
                        ui.checkbox(&mut cfg.load_most_recent, "Load most recent file at start");
                    });
                });
                if let Some(world_base) = &mut world_base {
                    Window::new("World").show(egui_ctx, |ui| {
                        ScrollArea::vertical().show(ui, |ui| {
                            ui.set_height(600.0);
                            ui.heading("Basic");
                            Grid::new("basic_info_grid").striped(true).show(ui, |ui| {
                                ui.label("Version");
                                ui.label(world_base.base_header.version.to_string());
                                ui.end_row();
                                ui.label("Times saved");
                                ui.label(world_base.base_header.times_saved.to_string());
                            });
                            ui.separator();
                            ui.heading("Header");
                            if !loading_tiles {
                                if ui.button("Load tiles").clicked() {
                                    let base_header = world_base.base_header.clone();
                                    let header = world_base.header.clone();
                                    let file = world_base.file.try_clone().unwrap();
                                    let sender = sender.clone();
                                    std::thread::spawn(move || {
                                        let ret_val = load_tiles(&file, &base_header, &header);
                                        sender.send(ret_val).unwrap();
                                    });
                                    loading_tiles = true;
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label("Loading tiles...");
                                    ui.add(Spinner::new());
                                });
                            }
                            Grid::new("world_base.header_grid")
                                .striped(true)
                                .show(ui, |ui| {
                                    field_macro!(ui, field);
                                    field!("Name", world_base.header.name);
                                    field!("Seed", world_base.header.seed);
                                    field!(
                                        "Generator version",
                                        world_base.header.generator_version
                                    );
                                    field!("GUID", guid_to_hex(&world_base.header.guid));
                                    field!("World id", world_base.header.id);
                                    ui.label("Bounds");
                                    Grid::new("bounds_grid").striped(true).show(ui, |ui| {
                                        field_macro!(ui, field2);
                                        field2!("left", world_base.header.bounds.left);
                                        field2!("right", world_base.header.bounds.right);
                                        field2!("top", world_base.header.bounds.top);
                                        field2!("bottom", world_base.header.bounds.bottom);
                                    });
                                    ui.end_row();
                                    field!(
                                        "size",
                                        format!(
                                            "{}x{}",
                                            world_base.header.width, world_base.header.height
                                        )
                                    );
                                    field!(
                                        "Game mode",
                                        format!(
                                            "{} ({})",
                                            game_mode_name(world_base.header.game_mode),
                                            world_base.header.game_mode
                                        )
                                    );
                                    if let Some(tile) = tiles.get(
                                        tile_y as usize * world_base.header.width as usize
                                            + tile_x as usize,
                                    ) {
                                        field!("Pointing at", format!("{}, {}", tile_x, tile_y));
                                        field!("Tile", format!("{:#?}", tile));
                                    }
                                });
                        })
                    });
                }
            });
            egui_macroquad::draw();
        }

        if is_key_pressed(KeyCode::F12) {
            show_ui ^= true;
        }

        if is_key_pressed(KeyCode::KpAdd) {
            scale *= 2;
            cam_x *= 2.0;
            cam_y *= 2.0;
        }

        if is_key_pressed(KeyCode::KpSubtract) && scale > 1 {
            scale /= 2;
            cam_x /= 2.0;
            cam_y /= 2.0;
        }

        let speed = 16.0;

        if is_key_down(KeyCode::Left) {
            cam_x += speed;
        }
        if is_key_down(KeyCode::Right) {
            cam_x -= speed;
        }
        if is_key_down(KeyCode::Up) {
            cam_y += speed;
        }
        if is_key_down(KeyCode::Down) {
            cam_y -= speed;
        }

        if let Some(world_base) = &world_base {
            cam_x = clamp(
                cam_x,
                -(world_base.header.width as f32 * scale as f32) + screen_width(),
                0.,
            );
            cam_y = clamp(
                cam_y,
                -(world_base.header.height as f32 * scale as f32) + screen_height(),
                0.,
            );
        }

        if is_quit_requested() {
            cfg.save()?;
            return Ok(());
        }

        next_frame().await;
    }
}

macro field_macro($ui:expr, $macname:ident) {
    macro $macname($name:expr, $val:expr) {
        $ui.label($name);
        $ui.label($val.to_string());
        $ui.end_row();
    }
}

fn guid_to_hex(guid: &[u8; 16]) -> String {
    use std::fmt::Write;

    let mut s = String::new();
    for byte in guid {
        write!(&mut s, "{:02x}", byte).unwrap();
    }
    s
}

fn load_tiles(file: &File, base_header: &BaseHeader, header: &Header) -> (Vec<Tile>, Image) {
    let mut tiles = vec![Tile::default(); header.width as usize * header.height as usize];
    let mut image = Image::gen_image_color(
        header.width as u16,
        header.height as u16,
        Color::from_rgba(0, 0, 0, 0),
    );
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

fn load_world(path: &Path, world_base: &mut Option<WorldBase>) -> bool {
    match terraria_wld::open(path, false) {
        Ok((file, base_header)) => {
            let header =
                terraria_wld::read_header(&file, base_header.offsets.header as u64).unwrap();
            *world_base = Some(WorldBase {
                base_header,
                header,
                file,
            });
            true
        }
        Err(e) => {
            rfd::MessageDialog::new()
                .set_description(&e.to_string())
                .show();
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
            3 => YELLOW,
            4 => RED,
            5 => BROWN,
            // Iron/copper/etc
            6 | 7 | 8 | 9 => ORANGE,
            // Platform
            19 => BROWN,
            // Wood
            30 => BROWN,
            // Clay
            40 => Color::from_rgba(154, 73, 40, 255),
            // Dungeon brick
            43 | 44 => Color::from_rgba(131, 0, 178, 255),
            // Chest
            21 => YELLOW,
            // Cobweb
            51 => Color::from_rgba(188, 175, 174, 255),
            // Vine
            52 => GREEN,
            // Sand
            53 | 112 | 116 | 234 => YELLOW,
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
            70 | 71 | 72 => Color::from_rgba(56, 230, 255, 255),
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

fn game_mode_name(name: i32) -> &'static str {
    match name {
        0 => "Normal",
        1 => "Expert",
        2 => "Master",
        3 => "Journey",
        _ => "Unknown",
    }
}
