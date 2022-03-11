use egui_macroquad::{
    egui::{Grid, ScrollArea, TopBottomPanel, Window},
    macroquad,
};

use macroquad::prelude::*;

#[macroquad::main("egui with macroquad")]
async fn main() {
    let mut world = None;
    let mut header = None;
    loop {
        clear_background(WHITE);

        // Process keys, mouse etc.

        egui_macroquad::ui(|egui_ctx| {
            TopBottomPanel::top("top_panel").show(egui_ctx, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            match terraria_wld::WorldFile::open(&path, false) {
                                Ok(mut wld) => {
                                    header = Some(wld.read_header().unwrap());
                                    world = Some(wld);
                                }
                                Err(e) => {
                                    rfd::MessageDialog::new()
                                        .set_description(&e.to_string())
                                        .show();
                                }
                            }
                        }
                        ui.close_menu();
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
