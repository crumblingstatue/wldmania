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
                                ui.label("World name");
                                ui.label(&header.name);
                                ui.end_row();
                                ui.label("Seed");
                                ui.label(&header.seed);
                                ui.end_row();
                                ui.label("Generator version");
                                ui.label(&header.generator_version.to_string());
                                ui.end_row();
                                ui.label("GUID");
                                ui.label(guid_to_hex(&header.guid));
                                ui.end_row();
                                ui.label("World id");
                                ui.label(header.id.to_string());
                                ui.end_row();
                                ui.label("== Bounds ==");
                                ui.end_row();
                                ui.label("left");
                                ui.label(header.bounds.left.to_string());
                                ui.end_row();
                                ui.label("right");
                                ui.label(header.bounds.right.to_string());
                                ui.end_row();
                                ui.label("top");
                                ui.label(header.bounds.top.to_string());
                                ui.end_row();
                                ui.label("bottom");
                                ui.label(header.bounds.bottom.to_string());
                                ui.end_row();
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
