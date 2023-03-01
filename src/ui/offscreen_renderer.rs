use super::texture::Texture;
use crate::biotracker::{protocol::*, DoubleBuffer};
use anyhow::Result;
use core::num::NonZeroU32;
use cv::prelude::*;
use egui::{mutex::RwLock, RawInput};
use egui_wgpu::wgpu;
use std::sync::Arc;

pub struct OffscreenRenderer {
    pub render_state: egui_wgpu::RenderState,
    pub texture: Texture,
    pub context: egui::Context,
    copy_buffer: Option<wgpu::Buffer>,
    image_history: DoubleBuffer,
    bytes_per_row: NonZeroU32,
}

impl OffscreenRenderer {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        width: u32,
        height: u32,
    ) -> Self {
        let texture = Texture::new(
            &device,
            width,
            height,
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            Some("offscreen_render_texture"),
        );
        let renderer = Arc::new(RwLock::new(egui_wgpu::Renderer::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            None,
            1,
        )));

        let mut bytes_per_row = width * 4;
        if bytes_per_row % 256 != 0 {
            bytes_per_row += 256 - (bytes_per_row % 256);
        }

        Self {
            context: egui::Context::default(),
            render_state: egui_wgpu::RenderState {
                device,
                queue,
                target_format: wgpu::TextureFormat::Rgba8UnormSrgb,
                renderer,
            },
            texture,
            copy_buffer: None,
            bytes_per_row: NonZeroU32::new(bytes_per_row).unwrap(),
            image_history: DoubleBuffer::new(),
        }
    }

    pub fn render(&mut self, full_output: egui::FullOutput, copy_texture: bool) {
        let clipped_primitives = self.context.tessellate(full_output.shapes);
        let mut encoder =
            self.render_state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui offscreen encoder"),
                });
        let render_state = &mut self.render_state;

        let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [self.texture.size.width, self.texture.size.height],
            pixels_per_point: 1.0,
        };

        if copy_texture {
            let rows = self.texture.size.height;
            let copy_buffer = render_state.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("egui_copy_buffer"),
                size: (rows * self.bytes_per_row.get()) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                self.texture.handle.as_image_copy(),
                wgpu::ImageCopyBuffer {
                    buffer: &copy_buffer,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(self.bytes_per_row),
                        rows_per_image: None,
                    },
                },
                self.texture.size,
            );
            self.copy_buffer = Some(copy_buffer);
        }

        let user_cmd_bufs = {
            let mut renderer = render_state.renderer.write();
            for (id, image_delta) in &full_output.textures_delta.set {
                renderer.update_texture(
                    &render_state.device,
                    &render_state.queue,
                    *id,
                    image_delta,
                );
            }

            renderer.update_buffers(
                &render_state.device,
                &render_state.queue,
                &mut encoder,
                &clipped_primitives,
                &screen_descriptor,
            )
        };

        {
            let renderer = render_state.renderer.read();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.texture.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
                label: Some("egui_render"),
            });

            renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
        }

        {
            let mut renderer = render_state.renderer.write();
            for id in &full_output.textures_delta.free {
                renderer.free_texture(id);
            }
        }

        let encoded = encoder.finish();
        // Submit the commands: both the main buffer and user-defined ones.
        render_state
            .queue
            .submit(user_cmd_bufs.into_iter().chain(std::iter::once(encoded)));
    }

    pub fn texture_to_image(&mut self, frame_number: u32) -> Result<Image> {
        if let Some(copy_buffer) = self.copy_buffer.take() {
            let (width, height) = (self.texture.size.width, self.texture.size.height);
            let bgr_buffer_len = (width * height * 3) as usize;
            let bgr_buffer = self.image_history.get(bgr_buffer_len);
            let rgba_buffer_slice = copy_buffer.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            rgba_buffer_slice.map_async(wgpu::MapMode::Read, move |r| {
                tx.send(r).unwrap();
            });
            self.render_state.device.poll(wgpu::Maintain::Wait);
            rx.recv().unwrap().unwrap();
            let rgba_buffer_view = rgba_buffer_slice.get_mapped_range();

            unsafe {
                let rgba_mat = Mat::new_size_with_data(
                    cv::core::Size::new(
                        self.texture.size.width as i32,
                        self.texture.size.height as i32,
                    ),
                    cv::core::CV_8UC4,
                    rgba_buffer_view.as_ptr() as *mut _,
                    self.bytes_per_row.get() as usize,
                )?;
                let mut bgr_mat = Mat::new_size_with_data(
                    cv::core::Size::new(width as i32, height as i32),
                    cv::core::CV_8UC3,
                    bgr_buffer.as_ptr() as *mut std::ffi::c_void,
                    cv::core::Mat_AUTO_STEP,
                )?;
                cv::imgproc::cvt_color(&rgba_mat, &mut bgr_mat, cv::imgproc::COLOR_RGBA2BGR, 0)
                    .unwrap();
            }
            let image = Image {
                stream_id: "Annotated".to_string(),
                shm_id: bgr_buffer.id().to_owned(),
                width: self.texture.size.width,
                height: self.texture.size.height,
                frame_number,
            };
            self.copy_buffer = None;
            return Ok(image);
        }
        Err(anyhow::anyhow!("No copy buffer"))
    }

    pub fn transform_events(&self, screen_rect: egui::Rect, events: Vec<egui::Event>) -> RawInput {
        let transform = egui::emath::RectTransform::from_to(
            screen_rect,
            egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(
                    self.texture.size.width as f32,
                    self.texture.size.height as f32,
                ),
            ),
        );
        let events = events
            .into_iter()
            .filter_map(|e| match e {
                egui::Event::PointerMoved(pos) => {
                    Some(egui::Event::PointerMoved(transform.transform_pos(pos)))
                }
                egui::Event::PointerButton {
                    pos,
                    pressed,
                    button,
                    modifiers,
                } => Some(egui::Event::PointerButton {
                    pos: transform.transform_pos(pos),
                    pressed,
                    button,
                    modifiers,
                }),
                egui::Event::Touch {
                    device_id,
                    id,
                    phase,
                    pos,
                    force,
                } => Some(egui::Event::Touch {
                    device_id,
                    id,
                    phase,
                    pos: transform.transform_pos(pos),
                    force,
                }),
                _ => None,
            })
            .collect();
        egui::RawInput {
            events,
            pixels_per_point: Some(1.0),
            ..Default::default()
        }
    }
}
