use super::texture::Texture;
use crate::biotracker::{protocol::*, DoubleBuffer, SharedBuffer};
use anyhow::Result;
use core::num::NonZeroU32;
use egui::mutex::RwLock;
use egui_wgpu::wgpu;
use std::sync::Arc;

pub struct OffscreenRenderer {
    pub render_state: egui_wgpu::RenderState,
    pub texture: Texture,
    context: egui::Context,
    copy_buffer: Option<wgpu::Buffer>,
    copy_buffer_row_padding: Option<u32>,
    image_history: DoubleBuffer,
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
            copy_buffer_row_padding: None,
            image_history: DoubleBuffer::new(),
        }
    }

    pub fn render(&mut self, run_ui: impl FnOnce(&egui::Context)) {
        let full_output = self.context.run(egui::RawInput::default(), |ui| {
            run_ui(ui);
        });
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

        let rows = self.texture.size.height;
        let mut bytes_per_row = self.texture.size.width * 4;
        if bytes_per_row % 256 != 0 {
            let row_padding = 256 - (bytes_per_row % 256);
            self.copy_buffer_row_padding = Some(row_padding);
            bytes_per_row += row_padding;
        } else {
            self.copy_buffer_row_padding = None;
        }
        let copy_buffer = render_state.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("egui_copy_buffer"),
            size: (rows * bytes_per_row) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            self.texture.handle.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &copy_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(NonZeroU32::new(bytes_per_row).unwrap()),
                    rows_per_image: None,
                },
            },
            self.texture.size,
        );
        self.copy_buffer = Some(copy_buffer);

        let encoded = encoder.finish();
        // Submit the commands: both the main buffer and user-defined ones.
        render_state
            .queue
            .submit(user_cmd_bufs.into_iter().chain(std::iter::once(encoded)));
    }

    pub fn texture_to_image(&mut self, frame_number: u32) -> Result<Image> {
        if let Some(copy_buffer) = self.copy_buffer.take() {
            let mut shared_buffer = SharedBuffer::new(
                (self.texture.size.width * self.texture.size.height * 4) as usize,
            )?;
            let buffer_slice = copy_buffer.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |r| {
                tx.send(r).unwrap();
            });
            self.render_state.device.poll(wgpu::Maintain::Wait);
            rx.recv().unwrap().unwrap();

            let buffer_view = buffer_slice.get_mapped_range();
            unsafe {
                if let Some(row_padding) = self.copy_buffer_row_padding.take() {
                    let dest = bytemuck::cast_slice_mut::<u8, u32>(shared_buffer.as_slice_mut());
                    let src = bytemuck::cast_slice::<u8, u32>(&(*buffer_view));
                    let (width, height) = (self.texture.size.width, self.texture.size.height);
                    let channels = std::mem::size_of::<u32>() as u32;
                    for row in 0..height {
                        for col in 0..width {
                            let src_idx = row * (width + row_padding / channels) + col;
                            let dest_idx = row * width + col;
                            dest[dest_idx as usize] = src[src_idx as usize];
                        }
                    }
                } else {
                    shared_buffer
                        .as_slice_mut()
                        .copy_from_slice(&(*buffer_view));
                }
            }
            let image = Image {
                stream_id: "Annotated".to_string(),
                shm_id: shared_buffer.id().to_owned(),
                width: self.texture.size.width,
                height: self.texture.size.height,
                frame_number,
            };
            self.image_history.push(shared_buffer);
            self.copy_buffer = None;
            return Ok(image);
        }
        Err(anyhow::anyhow!("No copy buffer"))
    }
}
