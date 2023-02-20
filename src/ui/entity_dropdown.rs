use super::color::Palette;

#[derive(Default)]
pub struct EntityDropdown {
    pub selected_id: Option<u32>,
    colors: Palette,
}

impl EntityDropdown {
    pub fn show(&mut self, ui: &mut egui::Ui, ids: &Vec<u32>, label: &str) {
        let selected_text = self.id_to_text(self.selected_id);
        egui::ComboBox::from_label(label)
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for id in ids {
                    let entity_text = self.id_to_text(Some(*id));
                    if ui
                        .selectable_value(&mut self.selected_id, Some(*id), entity_text)
                        .clicked()
                    {}
                }
            });
    }

    fn id_to_text(&self, id: Option<u32>) -> egui::RichText {
        match id {
            Some(id) => egui::RichText::new(format!("        {}        ", id))
                .color(egui::Color32::WHITE)
                .background_color(self.colors.pick(id)),
            None => egui::RichText::new(""),
        }
    }
}
