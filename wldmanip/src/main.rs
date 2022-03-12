#![feature(let_chains, decl_macro)]

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use egui_macroquad::{
    egui::{Grid, ScrollArea, TopBottomPanel, Window},
    macroquad,
};

use macroquad::prelude::*;
use recently_used_list::RecentlyUsedList;
use serde::{Deserialize, Serialize};
use terraria_wld::{Header, WorldFile};

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

#[macroquad::main("egui with macroquad")]
async fn main() -> anyhow::Result<()> {
    let mut world = None;
    let mut header = None;
    let mut cfg = Config::load_or_default()?;
    prevent_quit();
    if cfg.load_most_recent && let Some(most_recent) = cfg.recent_files.most_recent().cloned() && load_world(&most_recent, &mut header, &mut world) {
        cfg.recent_files.use_(most_recent);
    }
    loop {
        clear_background(WHITE);

        // Process keys, mouse etc.

        egui_macroquad::ui(|egui_ctx| {
            TopBottomPanel::top("top_panel").show(egui_ctx, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            if load_world(&path, &mut header, &mut world) {
                                cfg.recent_files.use_(path);
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    let mut used = None;
                    for recent in cfg.recent_files.iter() {
                        if ui.button(recent.display().to_string()).clicked() {
                            load_world(recent, &mut header, &mut world);
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
            if let Some(world) = &world {
                Window::new("World").show(egui_ctx, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Basic");
                        Grid::new("basic_info_grid").striped(true).show(ui, |ui| {
                            ui.label("Version");
                            ui.label(world.base_header.version.to_string());
                            ui.end_row();
                            ui.label("Times saved");
                            ui.label(world.base_header.times_saved.to_string());
                        });
                        ui.separator();
                        if let Some(header) = &header {
                            ui.heading("Header");
                            Grid::new("header_grid").striped(true).show(ui, |ui| {
                                field_macro!(ui, field);
                                field!("Name", header.name);
                                field!("Seed", header.seed);
                                field!("Generator version", header.generator_version);
                                field!("GUID", guid_to_hex(&header.guid));
                                field!("World id", header.id);
                                ui.label("Bounds");
                                Grid::new("bounds_grid").striped(true).show(ui, |ui| {
                                    field_macro!(ui, field2);
                                    field2!("left", header.bounds.left);
                                    field2!("right", header.bounds.right);
                                    field2!("top", header.bounds.top);
                                    field2!("bottom", header.bounds.bottom);
                                });
                                ui.end_row();
                                field!("width", header.width);
                                field!("height", header.height);
                                field!(
                                    "Game mode",
                                    format!(
                                        "{} ({})",
                                        game_mode_name(header.game_mode),
                                        header.game_mode
                                    )
                                );
                            });
                        }
                    })
                });
            }
        });

        // Draw things before egui

        egui_macroquad::draw();

        // Draw things after egui

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

fn load_world(path: &Path, header: &mut Option<Header>, world: &mut Option<WorldFile>) -> bool {
    match terraria_wld::WorldFile::open(path, false) {
        Ok(mut wld) => {
            *header = Some(wld.read_header().unwrap());
            *world = Some(wld);
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

fn game_mode_name(name: i32) -> &'static str {
    match name {
        0 => "Normal",
        1 => "Expert",
        2 => "Master",
        3 => "Journey",
        _ => "Unknown",
    }
}
