use super::{app::BioTrackerUIContext, color::Palette};
use crate::biotracker::protocol::*;

pub struct EntitySwitcher {
    selected_ids: [u32; 2],
}

impl EntitySwitcher {
    pub fn new() -> Self {
        Self {
            selected_ids: [0, 0],
        }
    }

    pub fn show(&mut self, egui_ctx: &egui::Context, ctx: &mut BioTrackerUIContext) {
        egui::Window::new("Switch Entities")
            .resizable(false)
            .collapsible(false)
            .open(&mut ctx.entity_switcher_open)
            .show(egui_ctx, |ui| {
                for i in 0..2 {
                    let label = match i {
                        0 => "First Entity",
                        1 => "Second Entity",
                        _ => panic!("Invalid entity index"),
                    };
                    let selected_text = self.id_to_text(self.selected_ids[i], &ctx.color_palette);
                    egui::ComboBox::from_label(label)
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            for id in &ctx.experiment.entity_ids {
                                let entity_text = self.id_to_text(*id, &ctx.color_palette);
                                if ui
                                    .selectable_value(&mut self.selected_ids[i], *id, entity_text)
                                    .clicked()
                                {}
                            }
                        });
                }
                if ui.add(egui::Button::new("Switch")).clicked() {
                    ctx.bt
                        .command(Command::SwitchEntities(EntityIdSwitch {
                            id1: self.selected_ids[0],
                            id2: self.selected_ids[1],
                        }))
                        .unwrap();
                };
            });
    }

    fn id_to_text(&self, id: u32, colors: &Palette) -> egui::RichText {
        if id == 0 {
            egui::RichText::new("")
        } else {
            egui::RichText::new(format!("        {}        ", id))
                .color(egui::Color32::WHITE)
                .background_color(colors.pick(id))
        }
    }
}
