use super::{app::BioTrackerUIContext, entity_dropdown::EntityDropdown};
use crate::biotracker::protocol::*;

#[derive(Default)]
pub struct EntitySwitcher {
    selected_entities: [EntityDropdown; 2],
}

impl EntitySwitcher {
    pub fn show(&mut self, egui_ctx: &egui::Context, ctx: &mut BioTrackerUIContext) {
        egui::Window::new("Switch Entities")
            .resizable(false)
            .collapsible(false)
            .open(&mut ctx.entity_switcher_open)
            .show(egui_ctx, |ui| {
                self.selected_entities[0].show(ui, &ctx.experiment.entity_ids, "First Entity");
                self.selected_entities[1].show(ui, &ctx.experiment.entity_ids, "Second Entity");
                if let (Some(id1), Some(id2)) = (
                    self.selected_entities[0].selected_id,
                    self.selected_entities[1].selected_id,
                ) {
                    if ui.button("Switch").clicked() {
                        ctx.bt
                            .command(Command::SwitchEntities(EntityIdSwitch { id1, id2 }))
                            .unwrap();
                    }
                } else {
                    ui.add_enabled(false, egui::Button::new("Switch"));
                }
            });
    }
}
