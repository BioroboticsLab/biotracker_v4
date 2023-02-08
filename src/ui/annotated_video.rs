use super::{
    app::BioTrackerUIContext,
    color::{Palette, ALPHABET},
    texture::Texture,
};
use crate::biotracker::{
    protocol::{Feature, Image, SkeletonDescriptor},
    SharedBuffer,
};
use egui_wgpu::wgpu;

pub struct AnnotatedVideo {
    color_palette: Palette,
    draw_features: bool,
    draw_entities: bool,
    draw_node_labels: bool,
    scale: f32,
    texture: Option<Texture>,
    onscreen_id: egui::TextureId,
    offscreen_id: egui::TextureId,
}

impl AnnotatedVideo {
    pub fn new() -> Self {
        Self {
            color_palette: Palette { colors: &ALPHABET },
            draw_features: false,
            draw_entities: true,
            draw_node_labels: false,
            scale: 1.0,
            texture: None,
            onscreen_id: egui::epaint::TextureId::default(),
            offscreen_id: egui::epaint::TextureId::default(),
        }
    }

    pub fn update_scale(&mut self, zoom_delta: f32) {
        if zoom_delta != 1.0 {
            self.scale = 0.1f32.max(self.scale * zoom_delta);
        }
    }

    pub fn update_image(
        &mut self,
        image: &Image,
        onscreen_render_state: &egui_wgpu::RenderState,
        offscreen_render_state: &egui_wgpu::RenderState,
    ) {
        let image_buffer = match SharedBuffer::open(&image.shm_id) {
            Ok(buffer) => buffer,
            Err(e) => {
                eprintln!("Failed to open shared buffer: {}", e);
                return;
            }
        };

        let mut reinitialize_texture = self.texture.is_none();
        if let Some(texture) = &mut self.texture {
            if texture.size.width != image.width || texture.size.height != image.height {
                reinitialize_texture = true;
            }
        }

        if reinitialize_texture {
            let texture = Texture::new(
                &onscreen_render_state.device,
                image.width,
                image.height,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                Some("egui_video_texture"),
            );
            self.onscreen_id = texture.egui_register(
                &onscreen_render_state.device,
                &onscreen_render_state.renderer,
            );
            self.offscreen_id = texture.egui_register(
                &offscreen_render_state.device,
                &offscreen_render_state.renderer,
            );
            self.texture = Some(texture);
        }

        unsafe {
            self.texture
                .as_mut()
                .expect("Texture not initialized")
                .update(
                    &onscreen_render_state.queue,
                    image.width,
                    image.height,
                    image_buffer.as_slice(),
                )
        }
    }

    pub fn show_settings(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.draw_features, "Draw unmatched entity features");
        ui.checkbox(&mut self.draw_entities, "Draw matched entities");
        ui.checkbox(&mut self.draw_node_labels, "Draw node labels");
    }

    pub fn show_onscreen(&self, ui: &mut egui::Ui, ctx: &BioTrackerUIContext) {
        self.show(ui, self.onscreen_id, Some(self.scale), ctx);
    }

    pub fn show_offscreen(&self, ui: &mut egui::Ui, ctx: &BioTrackerUIContext) {
        self.show(ui, self.offscreen_id, None, ctx);
    }

    fn paint_feature(
        &self,
        painter: &egui::Painter,
        to_screen: &egui::emath::RectTransform,
        feature: &Feature,
        skeleton: &Option<SkeletonDescriptor>,
        color: egui::Color32,
        scale: Option<f32>,
    ) {
        let scale = match scale {
            Some(x) => x,
            None => 1.0,
        };
        let nodes = &feature.nodes;
        if let Some(skeleton) = skeleton {
            for edge in &skeleton.edges {
                let from_idx = edge.source as usize;
                let to_idx = edge.target as usize;
                let from = to_screen * egui::pos2(nodes[from_idx].x, nodes[from_idx].y);
                let to = to_screen * egui::pos2(nodes[to_idx].x, nodes[to_idx].y);
                if from.any_nan() || to.any_nan() {
                    continue;
                }
                painter.line_segment([from, to], egui::Stroke::new(2.0 * scale, color));
            }
        }
        for node in nodes {
            let point = to_screen * egui::pos2(node.x, node.y);
            if point.any_nan() {
                continue;
            }
            painter.circle(
                point,
                1.5 * scale,
                egui::Color32::WHITE,
                egui::Stroke::new(1.0, egui::Color32::BLACK),
            )
        }
        if self.draw_node_labels {
            if let Some(skeleton) = skeleton {
                for i in 0..nodes.len() {
                    let node = &nodes[i];
                    let point = to_screen * egui::pos2(node.x, node.y);
                    painter.text(
                        point + egui::vec2(10.0 * scale, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &skeleton.node_names[i],
                        egui::FontId {
                            size: 9.0 * scale,
                            family: egui::FontFamily::Proportional,
                        },
                        egui::Color32::WHITE,
                    );
                }
            }
        }
    }

    fn show(
        &self,
        ui: &mut egui::Ui,
        texture_id: egui::epaint::TextureId,
        scale: Option<f32>,
        ctx: &BioTrackerUIContext,
    ) {
        let texture = match &self.texture {
            Some(texture) => texture,
            None => return,
        };

        let (response, painter) = match scale {
            Some(scale) => {
                let aspect_ratio = texture.size.height as f32 / texture.size.width as f32;
                let width = ui.available_width() * scale;
                let height = width * aspect_ratio;
                ui.allocate_painter(egui::Vec2::new(width, height), egui::Sense::hover())
            }
            None => ui.allocate_painter(
                egui::Vec2::new(texture.size.width as f32, texture.size.height as f32),
                egui::Sense::hover(),
            ),
        };

        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(texture_id, response.rect, uv, egui::Color32::WHITE);
        let to_screen = egui::emath::RectTransform::from_to(
            egui::Rect::from_x_y_ranges(
                0.0..=texture.size.width as f32,
                0.0..=texture.size.height as f32,
            ),
            response.rect,
        );
        let mut skeleton = None;
        if let Some(features) = &ctx.current_features {
            if self.draw_features {
                for feature in &features.features {
                    self.paint_feature(
                        &painter,
                        &to_screen,
                        feature,
                        &features.skeleton,
                        egui::Color32::GREEN,
                        scale,
                    );
                }
            }
            skeleton = features.skeleton.clone();
        }
        if let Some(entities) = &ctx.current_entities {
            if self.draw_entities {
                for entity in &entities.entities {
                    if let Some(feature) = &entity.feature {
                        let color = self.color_palette.pick(entity.id);
                        self.paint_feature(&painter, &to_screen, feature, &skeleton, color, scale);
                    }
                }
            }
        }
    }
}
