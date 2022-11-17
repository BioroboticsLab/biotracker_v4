use super::{
    color::{Palette, ALPHABET},
    texture_image::TextureImage,
};
use crate::core::{BufferManager, Entities, ImageData, ImageFeature, ImageFeatures};

pub struct VideoPlane {
    buffer_manager: BufferManager,
    texture_image: Option<TextureImage>,
    last_features: Option<ImageFeatures>,
    last_entities: Option<Entities>,
    color_palette: Palette,
    draw_features: bool,
    draw_entities: bool,
}

impl VideoPlane {
    pub fn new() -> Self {
        Self {
            buffer_manager: BufferManager::new(),
            texture_image: None,
            last_features: None,
            last_entities: None,
            color_palette: Palette { colors: &ALPHABET },
            draw_features: false,
            draw_entities: true,
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

    pub fn update_entities(&mut self, entities: Entities) {
        self.last_entities = Some(entities);
    }

    fn paint_feature(
        &self,
        painter: &egui::Painter,
        to_screen: &egui::emath::RectTransform,
        feature: &ImageFeature,
        color: egui::Color32,
    ) {
        let (nodes, edges) = (&feature.nodes, &feature.edges);
        for edge in edges {
            let point_from = &nodes[edge.from].point;
            let point_to = &nodes[edge.to].point;
            let from = to_screen * egui::pos2(point_from.x, point_from.y);
            let to = to_screen * egui::pos2(point_to.x, point_to.y);
            painter.line_segment([from, to], egui::Stroke::new(2.0, egui::Color32::BLACK));
        }
        for node in nodes {
            let point = to_screen * egui::pos2(node.point.x, node.point.y);
            painter.circle(
                point,
                5.0,
                color,
                egui::Stroke::new(1.0, egui::Color32::BLACK),
            )
        }
    }

    pub fn show_settings(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.draw_features, "Draw unmatched entity features");
        ui.checkbox(&mut self.draw_entities, "Draw matched entities");
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
            if let Some(features) = &self.last_features {
                if self.draw_features {
                    for feature in &features.features {
                        self.paint_feature(&painter, &to_screen, feature, egui::Color32::GREEN);
                    }
                }
            }
            if let Some(entities) = &self.last_entities {
                if self.draw_entities {
                    for (uuid, feature) in &entities.entities {
                        let color = self.color_palette.pick(uuid.0);
                        self.paint_feature(&painter, &to_screen, feature, color);
                    }
                }
            }
        }
    }
}
