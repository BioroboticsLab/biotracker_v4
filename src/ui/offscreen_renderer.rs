use super::texture::Texture;
use anyhow::Result;
use core::num::NonZeroU32;
use egui::mutex::RwLock;
use egui_wgpu::wgpu;
use libtracker::{message_bus::Client, BufferHistory, ImageData, Message, SharedBuffer, Timestamp};
use std::sync::Arc;

pub struct OffscreenRenderer {
    pub render_state: egui_wgpu::RenderState,
    context: egui::Context,
    texture: Texture,
    copy_buffer: Option<wgpu::Buffer>,
    image_history: BufferHistory,
}

const WIDTH: u32 = 2048;
const HEIGTH: u32 = 2048;

impl OffscreenRenderer {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let texture = Texture::new(
            &device,
            WIDTH,
            HEIGTH,
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
            image_history: BufferHistory::new(),
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
            size_in_pixels: [WIDTH, HEIGTH],
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
                depth_stencil_attachment: None, // FIXME: could be necessary!
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

        let copy_buffer = render_state.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("egui_copy_buffer"),
            size: (WIDTH * HEIGTH * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            self.texture.handle.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &copy_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(NonZeroU32::new(WIDTH * 4).unwrap()),
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

    pub fn post_rendering(&mut self, msg_bus: &Client, pts: &Timestamp) -> Result<()> {
        if let Some(copy_buffer) = self.copy_buffer.take() {
            let mut shared_buffer = SharedBuffer::new((WIDTH * HEIGTH * 4) as usize)?;
            let buffer_slice = copy_buffer.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |r| {
                tx.send(r).unwrap();
            });
            self.render_state.device.poll(wgpu::Maintain::Wait);
            rx.recv().unwrap().unwrap();

            let buffer_view = buffer_slice.get_mapped_range();
            unsafe {
                shared_buffer
                    .as_slice_mut()
                    .copy_from_slice(&(*buffer_view));
            }
            let image_data = ImageData {
                shm_id: shared_buffer.id().to_owned(),
                width: self.texture.size.width,
                height: self.texture.size.height,
                pts: pts.clone(),
            };
            self.image_history.push(shared_buffer);
            msg_bus.send(Message::AnnotatedImage(image_data)).unwrap();
        }
        Ok(())
    }
}
