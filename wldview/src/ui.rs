use {
    crate::{Config, MqVec2Ext, ViewerState, WorldState, load_tiles, load_world},
    egui_file_dialog::FileDialog,
    egui_macroquad::{
        egui,
        macroquad::{
            math::vec2,
            texture::Image,
            window::{screen_height, screen_width},
        },
    },
    std::sync::mpsc::Sender,
    terraria_wld::{Chest, Liquid, Tile},
};

macro_rules! field_macro {
    ($ui:expr, $macname:ident) => {
        macro_rules! $macname {
            ($name:expr, $val:expr) => {{
                $ui.label($name);
                $ui.label($val.to_string());
                $ui.end_row();
            }};
        }
    };
}

#[derive(Default)]
pub struct UiState {
    pub egui_wants_pointer: bool,
    pub selected_chest: Option<usize>,
    pub loading_tiles: bool,
    pub hide_ui: bool,
    file_dia: FileDialog,
    chests_window: bool,
}

pub fn egui_ui(
    egui_ctx: &egui::Context,
    world: &mut WorldState,
    view: &mut ViewerState,
    ui_state: &mut UiState,
    cfg: &mut Config,
    sender: &Sender<(Vec<Tile>, Image)>,
) {
    egui::TopBottomPanel::top("top_panel").show(egui_ctx, |ui| {
        ui.horizontal(|ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    ui_state.file_dia.pick_file();
                }
                ui.separator();
                let mut used = None;
                ui.menu_button("Recent", |ui| {
                    for recent in cfg.recent_files.iter() {
                        if ui.button(recent.display().to_string()).clicked() {
                            load_world(recent, world, &mut view.map_tex);
                            used = Some(recent.to_owned());
                            break;
                        }
                    }
                });
                if let Some(used) = used {
                    cfg.recent_files.use_(used);
                }
                ui.separator();
                ui.checkbox(&mut cfg.load_most_recent, "Load most recent file at start");
                ui.checkbox(&mut cfg.load_tiles_at_start, "Load tiles at start");
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut cfg.draw_center_marker, "Draw center marker");
                ui.checkbox(&mut ui_state.chests_window, "Chests");
            });
        });
    });
    ui_state.file_dia.update(egui_ctx);
    if let Some(path) = ui_state.file_dia.take_picked()
        && load_world(&path, world, &mut view.map_tex)
    {
        cfg.recent_files.use_(path);
    }
    if let Some(world_base) = &world.base {
        egui::Window::new("World").show(egui_ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_height(600.0);
                ui.heading("Basic");
                egui::Grid::new("basic_info_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Version");
                        ui.label(world_base.base_header.version.to_string());
                        ui.end_row();
                        ui.label("Times saved");
                        ui.label(world_base.base_header.times_saved.to_string());
                    });
                ui.separator();
                ui.heading("Header");
                if !ui_state.loading_tiles {
                    if ui.button("Load tiles").clicked() {
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
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Loading tiles...");
                        ui.add(egui::Spinner::new());
                    });
                }
                egui::Grid::new("world_base.header_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        field_macro!(ui, field);
                        field!("Name", world_base.header.name);
                        field!("Seed", world_base.header.seed);
                        field!("Generator version", world_base.header.generator_version);
                        field!("GUID", guid_to_hex(&world_base.header.guid));
                        field!("World id", world_base.header.id);
                        ui.label("Bounds");
                        egui::Grid::new("bounds_grid").striped(true).show(ui, |ui| {
                            field_macro!(ui, field2);
                            field2!("left", world_base.header.bounds.left);
                            field2!("right", world_base.header.bounds.right);
                            field2!("top", world_base.header.bounds.top);
                            field2!("bottom", world_base.header.bounds.bottom);
                        });
                        ui.end_row();
                        field!(
                            "size",
                            format!("{}x{}", world_base.header.width, world_base.header.height)
                        );
                        field!(
                            "Game mode",
                            format!(
                                "{} ({})",
                                game_mode_name(world_base.header.game_mode),
                                world_base.header.game_mode
                            )
                        );
                        if let Some(tile) = world.tiles.get(
                            view.pointed_tile.y as usize * world_base.header.width as usize
                                + view.pointed_tile.x as usize,
                        ) {
                            field!(
                                "Pointing at",
                                format!("{}, {}", view.pointed_tile.x, view.pointed_tile.y)
                            );
                            match tile.front {
                                Some(id) => field!("Tile", id),
                                None => field!("Tile", "[none]"),
                            };
                            match tile.back {
                                Some(id) => field!("Wall", id),
                                None => field!("Wall", "[none]"),
                            };
                            field!(
                                "Liquid",
                                match tile.liquid {
                                    None => "[none]",
                                    Some(Liquid::Water) => "Water",
                                    Some(Liquid::Honey) => "Honey",
                                    Some(Liquid::Lava) => "Lava",
                                }
                            );
                        }
                        field!("cam x", view.cam.x);
                        field!("cam y", view.cam.y);
                    });
            })
        });
        if let Some(index) = ui_state.selected_chest {
            let chest: &Chest = &world_base.chests[index];
            let chest_name_fmt: String;
            let label = if chest.name.is_empty() {
                "Chest"
            } else {
                chest_name_fmt = format!("Chest: {}", chest.name);
                &chest_name_fmt
            };
            egui::Window::new(label)
                .id("chest_popup".into())
                .show(egui_ctx, |ui| {
                    for item in &chest.items {
                        if item.id != 0 {
                            match world.item_id_map.name_by_id(item.id as u16) {
                                Some(name) => ui.label(format!("{} x {}", name, item.stack)),
                                None => {
                                    ui.label(format!("Unknown item ({}) x {}", item.id, item.stack))
                                }
                            };
                        }
                        ui.end_row();
                    }
                });
        }
        egui::Window::new("Chests")
            .open(&mut ui_state.chests_window)
            .show(egui_ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, chest) in world_base.chests.iter().enumerate() {
                        let name = if chest.name.is_empty() {
                            "<unnamed>"
                        } else {
                            &chest.name
                        };
                        if ui
                            .selectable_label(
                                ui_state.selected_chest == Some(i),
                                format!("{name} at {} {}", chest.x, chest.y),
                            )
                            .clicked()
                        {
                            ui_state.selected_chest = Some(i);
                            center_on_chest(view, chest);
                        }
                    }
                });
            });
    }
    ui_state.egui_wants_pointer = egui_ctx.wants_pointer_input();
}

fn center_on_chest(view: &mut ViewerState, chest: &Chest) {
    let pix_pos = vec2(chest.x as f32, chest.y as f32).to_pix(view.scale);
    view.cam = -pix_pos;
    view.cam.x += screen_width() / 2.;
    view.cam.y += screen_height() / 2.;
    // "Center" on the chest rather than pointing at left top
    view.cam.x -= view.scale as f32;
    view.cam.y -= view.scale as f32;
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

fn guid_to_hex(guid: &[u8; 16]) -> String {
    use std::fmt::Write;

    let mut s = String::new();
    for byte in guid {
        write!(&mut s, "{byte:02x}").unwrap();
    }
    s
}
