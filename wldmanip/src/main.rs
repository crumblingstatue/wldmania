use egui_macroquad::{
    egui::{ScrollArea, TopBottomPanel, Window},
    macroquad,
};

use macroquad::prelude::*;

#[macroquad::main("egui with macroquad")]
async fn main() {
    let mut world = None;
    loop {
        clear_background(WHITE);

        // Process keys, mouse etc.

        egui_macroquad::ui(|egui_ctx| {
            TopBottomPanel::top("top_panel").show(egui_ctx, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            match terraria_wld::WorldFile::open(&path, false) {
                                Ok(wld) => world = Some(wld),
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
                        ui.label(&format!("{:#?}", world));
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
