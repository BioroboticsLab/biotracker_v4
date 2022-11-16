use super::texture_image::TextureImage;
use crate::core::{BufferManager, ImageData, ImageFeatures};

pub struct VideoPlane {
    buffer_manager: BufferManager,
    texture_image: Option<TextureImage>,
    last_features: Option<ImageFeatures>,
}

impl VideoPlane {
    pub fn new() -> Self {
        Self {
            buffer_manager: BufferManager::new(),
            texture_image: None,
            last_features: None,
        }
    }

    pub fn update_texture(&mut self, render_state: &egui_wgpu::RenderState, img: &ImageData) {
        let image_buffer = self.buffer_manager.get(&img.shm_id).unwrap();
        if self.texture_image.is_none() {
            self.texture_image = Some(TextureImage::new(&render_state, img.width, img.height));
        }

        if let Some(texture_image) = &mut self.texture_image {
            unsafe {
                texture_image.update(
                    &render_state,
                    img.width,
                    img.height,
                    image_buffer.as_slice(),
                )
            }
        }
    }

    pub fn update_features(&mut self, features: ImageFeatures) {
        self.last_features = Some(features);
    }

    pub fn show(&self, ui: &mut egui::Ui, scale: f32) {
        if let Some(texture_image) = &self.texture_image {
            let aspect_ratio = texture_image.height as f32 / texture_image.width as f32;
            let width = ui.available_width() * scale;
            let height = width * aspect_ratio;
            let (response, painter) =
                ui.allocate_painter(egui::Vec2::new(width, height), egui::Sense::hover());
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(
                texture_image.egui_id,
                response.rect,
                uv,
                egui::Color32::WHITE,
            );
            let to_screen = egui::emath::RectTransform::from_to(
                egui::Rect::from_x_y_ranges(
                    0.0..=texture_image.width as f32,
                    0.0..=texture_image.height as f32,
                ),
                response.rect,
            );
            //let from_screen = to_screen.inverse();

            if let Some(features) = &self.last_features {
                for feature in &features.features {
                    let (nodes, edges) = (&feature.nodes, &feature.edges);
                    for node in nodes {
                        let point = to_screen * egui::pos2(node.point.x, node.point.y);
                        painter.circle(
                            point,
                            3.0,
                            egui::Color32::GREEN,
                            egui::Stroke::new(1.0, egui::Color32::BLACK),
                        )
                    }
                    for edge in edges {
                        let point_from = &nodes[edge.from].point;
                        let point_to = &nodes[edge.to].point;
                        let from = to_screen * egui::pos2(point_from.x, point_from.y);
                        let to = to_screen * egui::pos2(point_to.x, point_to.y);
                        painter
                            .line_segment([from, to], egui::Stroke::new(1.0, egui::Color32::BLACK));
                    }
                }
            }
        }
    }
}
