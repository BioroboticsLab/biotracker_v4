use serde_json::Value;

pub struct ConfigJson {
    changed: bool,
}

impl ConfigJson {
    pub fn new() -> Self {
        Self { changed: false }
    }

    pub fn show(mut self, ui: &mut egui::Ui, config_json: &mut String) -> Self {
        let mut config: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(config_json).unwrap();
        for (key, mut value) in config.iter_mut() {
            ui.add(egui::Label::new(key));
            match &mut value {
                Value::Bool(ref mut b) => {
                    if ui.checkbox(b, "").changed() {
                        self.changed = true;
                    }
                }
                _ => {}
            }
            ui.end_row();
        }
        if self.changed {
            *config_json = serde_json::to_string(&config).unwrap();
        }
        self
    }

    pub fn changed(&self) -> bool {
        self.changed
    }
}
