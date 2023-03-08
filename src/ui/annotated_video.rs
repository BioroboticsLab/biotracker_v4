use super::{
    annotator::Annotator, app::BioTrackerUIContext, offscreen_renderer::OffscreenRenderer,
    polygon::Polygon, texture::Texture,
};
use crate::biotracker::{
    protocol::{Arena, Command, Feature, Image, SkeletonDescriptor},
    DoubleBuffer,
};
use cv::prelude::*;
use egui_wgpu::wgpu;

pub struct AnnotatedVideo {
    pub draw_features: bool,
    pub draw_entities: bool,
    pub draw_node_labels: bool,
    pub draw_ids: bool,
    pub draw_rectification: bool,
    pub draw_tracking_area: bool,
    image_updated: bool,
    render_texture_id: egui::TextureId,
    image_texture_id: egui::TextureId,
    scale: f32,
    image_texture: Option<Texture>,
    annotator: Annotator,
    rectification: Polygon,
    tracking_area: Polygon,
    offscreen_renderer: OffscreenRenderer,
    image_buffers: DoubleBuffer,
}

impl AnnotatedVideo {
    pub fn new(render_state: &egui_wgpu::RenderState) -> Self {
        let (offscreen_renderer, offscreen_texture_id) =
            init_offscreen_renderer(1024, 1024, render_state);
        Self {
            draw_features: false,
            draw_entities: true,
            draw_node_labels: false,
            draw_ids: true,
            draw_rectification: true,
            draw_tracking_area: true,
            image_updated: false,
            render_texture_id: offscreen_texture_id,
            image_texture_id: egui::TextureId::default(),
            scale: 1.0,
            image_texture: None,
            annotator: Annotator::default(),
            rectification: Polygon::new(),
            tracking_area: Polygon::new(),
            offscreen_renderer,
            image_buffers: DoubleBuffer::new(),
        }
    }

    pub fn update_image(&mut self, image: &Image, render_state: &egui_wgpu::RenderState) {
        let bgr_image = match self.image_buffers.get(image) {
            Ok(img) => img,
            Err(e) => {
                log::error!("Failed to open shared image: {}", e);
                return;
            }
        };
        self.image_updated = true;
        if self.offscreen_renderer.texture.size.width != image.width
            || self.offscreen_renderer.texture.size.height != image.height
        {
            (self.offscreen_renderer, self.render_texture_id) =
                init_offscreen_renderer(image.width, image.height, render_state);
        }

        let mut reinitialize_texture = self.image_texture.is_none();
        if let Some(texture) = &mut self.image_texture {
            if texture.size.width != image.width || texture.size.height != image.height {
                reinitialize_texture = true;
            }
        }

        if reinitialize_texture {
            let texture = Texture::new(
                &render_state.device,
                image.width,
                image.height,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                Some("egui_video_texture"),
            );
            self.image_texture_id = texture.egui_register(
                &render_state.device,
                &self.offscreen_renderer.render_state.renderer,
            );
            self.image_texture = Some(texture);
        }

        let rgba_data = vec![0; (image.width * image.height * 4) as usize];
        unsafe {
            let mut rgba_mat = Mat::new_size_with_data(
                cv::core::Size::new(image.width as i32, image.height as i32),
                cv::core::CV_8UC4,
                rgba_data.as_ptr() as *mut std::ffi::c_void,
                cv::core::Mat_AUTO_STEP,
            )
            .unwrap();
            cv::imgproc::cvt_color(
                &bgr_image.mat,
                &mut rgba_mat,
                cv::imgproc::COLOR_BGR2RGBA,
                0,
            )
            .unwrap();
        }
        self.image_texture
            .as_mut()
            .expect("Texture not initialized")
            .update(
                &self.offscreen_renderer.render_state.queue,
                image.width,
                image.height,
                rgba_data.as_slice(),
            )
    }

    fn update_scale(&mut self, ui: &mut egui::Ui) {
        let zoom_delta = ui.input(|i| i.zoom_delta());
        if zoom_delta != 1.0 {
            self.scale = 0.1f32.max(self.scale * zoom_delta);
        }
    }

    fn paint_feature(
        &self,
        id: Option<u32>,
        painter: &egui::Painter,
        feature: &Feature,
        skeleton: &Option<SkeletonDescriptor>,
        color: egui::Color32,
    ) {
        let line_width = 6.0;
        let circle_radius = 3.0;
        let text_size = 12.0;

        let nodes = &feature.nodes;
        if let Some(skeleton) = skeleton {
            for edge in &skeleton.edges {
                let from_idx = edge.source as usize;
                let to_idx = edge.target as usize;
                let from = egui::pos2(nodes[from_idx].x, nodes[from_idx].y);
                let to = egui::pos2(nodes[to_idx].x, nodes[to_idx].y);
                if from.any_nan() || to.any_nan() {
                    continue;
                }
                painter.line_segment([from, to], egui::Stroke::new(line_width, color));
            }
        }

        let mut center_point = egui::pos2(0.0, 0.0);
        let mut n_nodes = 0;
        for node in nodes {
            let point = egui::pos2(node.x, node.y);
            if point.any_nan() {
                continue;
            }
            center_point += egui::Vec2 {
                x: point.x,
                y: point.y,
            };
            n_nodes += 1;

            painter.circle(
                point,
                circle_radius,
                egui::Color32::WHITE,
                egui::Stroke::new(1.0, egui::Color32::BLACK),
            )
        }

        if let Some(id) = id {
            if n_nodes > 0 && self.draw_ids {
                center_point.x /= n_nodes as f32;
                center_point.y /= n_nodes as f32;
                painter.text(
                    center_point,
                    egui::Align2::CENTER_TOP,
                    id.to_string(),
                    egui::FontId {
                        size: text_size,
                        family: egui::FontFamily::Proportional,
                    },
                    egui::Color32::WHITE,
                );
            }
        }

        if self.draw_node_labels {
            if let Some(skeleton) = skeleton {
                for i in 0..nodes.len() {
                    let node = &nodes[i];
                    let point = egui::pos2(node.x, node.y);
                    painter.text(
                        point + egui::vec2(10.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &skeleton.node_names[i],
                        egui::FontId {
                            size: text_size,
                            family: egui::FontFamily::Proportional,
                        },
                        egui::Color32::WHITE,
                    );
                }
            }
        }
    }

    pub fn post_rendering(&mut self, ctx: &mut BioTrackerUIContext) {
        if self.image_updated {
            self.image_updated = false;
            let image = self
                .offscreen_renderer
                .texture_to_image(ctx.current_frame_number)
                .unwrap();
            match ctx.bt.add_image(image) {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to send image: {}", e);
                }
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        self.update_scale(ui);
        let response = self.show_onscreen(ui);
        let events = ui.input(|i| i.raw.events.clone());

        let raw_input = self
            .offscreen_renderer
            .transform_events(response.rect, events);
        let offscreen_ctx = &self.offscreen_renderer.context.clone();
        let full_output = offscreen_ctx.run(raw_input, |_| {
            egui::CentralPanel::default().show(offscreen_ctx, |ui| {
                self.show_offscreen(ui, ctx);
            });
        });
        self.offscreen_renderer
            .render(full_output, self.image_updated);
    }

    fn show_onscreen(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let aspect_ratio = self.offscreen_renderer.texture.size.height as f32
            / self.offscreen_renderer.texture.size.width as f32;
        let width = ui.available_width() * self.scale;
        let height = width * aspect_ratio;
        egui::ScrollArea::both()
            .show(ui, |ui| {
                ui.add(egui::Image::new(
                    self.render_texture_id,
                    egui::vec2(width, height),
                ))
            })
            .inner
    }

    fn show_offscreen(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        let texture = match &self.image_texture {
            Some(texture) => texture,
            None => return,
        };
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(texture.size.width as f32, texture.size.height as f32),
            egui::Sense::hover(),
        );

        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(
            self.image_texture_id,
            response.rect,
            uv,
            egui::Color32::WHITE,
        );
        let mut skeleton = None;
        if let Some(features) = &ctx.experiment.last_features {
            if self.draw_features {
                for feature in &features.features {
                    let color = match feature.out_of_bounds {
                        true => egui::Color32::RED,
                        false => egui::Color32::GREEN,
                    };
                    self.paint_feature(None, &painter, feature, &features.skeleton, color);
                }
            }
            skeleton = features.skeleton.clone();
        }
        if let Some(entities) = &ctx.experiment.last_entities {
            if self.draw_entities {
                for entity in &entities.entities {
                    if let Some(feature) = &entity.feature {
                        let color = ctx.color_palette.pick(entity.id);
                        self.paint_feature(Some(entity.id), &painter, feature, &skeleton, color);
                    }
                }
            }
        }

        self.annotator.show(&response, &painter, ctx);
        if let Some(arena) = ctx.experiment.arena.as_ref() {
            if self.draw_rectification {
                if let Some(changed_corners) = self.rectification.show(
                    "rectification_area".into(),
                    ui,
                    &response,
                    &painter,
                    &arena.rectification_corners,
                    &egui::Stroke::new(4.0, egui::Color32::RED.linear_multiply(0.25)),
                ) {
                    ctx.bt.check_command(Command::UpdateArena(Arena {
                        rectification_corners: changed_corners,
                        ..ctx.experiment.arena.clone().expect("Arena not set")
                    }));
                }
            }
            if self.draw_tracking_area {
                if let Some(changed_corners) = self.tracking_area.show(
                    "tracking_area".into(),
                    ui,
                    &response,
                    &painter,
                    &arena.tracking_area_corners,
                    &egui::Stroke::new(4.0, egui::Color32::BLUE.linear_multiply(0.25)),
                ) {
                    ctx.bt.check_command(Command::UpdateArena(Arena {
                        tracking_area_corners: changed_corners,
                        ..ctx.experiment.arena.clone().expect("Arena not set")
                    }));
                }
            }
        }
    }
}

fn init_offscreen_renderer(
    width: u32,
    height: u32,
    render_state: &egui_wgpu::RenderState,
) -> (OffscreenRenderer, egui::TextureId) {
    let offscreen_renderer = OffscreenRenderer::new(
        render_state.device.clone(),
        render_state.queue.clone(),
        width,
        height,
    );
    let texture_id = offscreen_renderer
        .texture
        .egui_register(&render_state.device, &render_state.renderer);
    (offscreen_renderer, texture_id)
}
