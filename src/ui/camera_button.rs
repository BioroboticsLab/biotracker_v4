use super::app::BioTrackerUIContext;
use crate::biotracker::protocol::*;

pub struct CameraButton {
    pub cameras: Vec<String>,
    pub dialog_open: bool,
}

impl CameraButton {
    pub fn new() -> Self {
        let mut cameras = enumerate_webcams();
        cameras.append(&mut enumerate_pylon());
        Self {
            cameras,
            dialog_open: false,
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        let camera_icon = egui::RichText::new("📷");
        let response = ui.toggle_value(&mut self.dialog_open, camera_icon);
        if self.dialog_open {
            egui::Window::new("Select Camera")
                .fixed_pos(response.rect.center_bottom())
                .pivot(egui::Align2::LEFT_TOP)
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .show(ui.ctx(), |ui| {
                    egui::ComboBox::from_label("Select Camera")
                        .selected_text("")
                        .show_ui(ui, |ui| {
                            for camera in &self.cameras {
                                if ui.selectable_label(false, camera).clicked() {
                                    ctx.bt.command(Command::OpenVideo(camera.to_owned()));
                                }
                            }
                        });
                });
        }
    }
}

#[cfg(not(feature = "pylon"))]
fn enumerate_pylon() -> Vec<String> {
    vec![]
}

#[cfg(feature = "pylon")]
fn enumerate_pylon() -> Vec<String> {
    let pylon = pylon_cxx::Pylon::new();
    pylon_cxx::TlFactory::instance(&pylon)
        .enumerate_devices()
        .unwrap()
        .iter()
        .map(|device| format!("pylon:///{}", device.model_name().unwrap()))
        .collect()
}

fn enumerate_webcams() -> Vec<String> {
    let mut cameras = Vec::new();
    let v4l_dir = "/dev/v4l/by-id/";
    match std::fs::read_dir(v4l_dir) {
        Ok(entries) => entries.map(|e| e.unwrap()).for_each(|e| {
            let path = e.path();
            let path_str = path.to_str().unwrap();
            cameras.push(path_str.to_owned());
        }),
        Err(_) => {
            // fallback to /dev/video*
            for i in 0..10 {
                let path = format!("/dev/video{}", i);
                if std::path::Path::new(&path).exists() {
                    cameras.push(path);
                }
            }
        }
    }
    cameras
}
