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
        for (key, value) in config.iter_mut() {
            ui.add(egui::Label::new(key));
            match value {
                Value::Bool(ref mut b) => {
                    if ui.checkbox(b, "").changed() {
                        self.changed = true;
                    }
                }
                Value::Number(ref mut n) => {
                    if let Some(mut f) = n.as_f64() {
                        if ui.add(egui::DragValue::new(&mut f)).changed() {
                            *n = serde_json::Number::from_f64(f).unwrap();
                            self.changed = true;
                        }
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
