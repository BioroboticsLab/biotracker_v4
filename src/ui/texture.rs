use egui::mutex::RwLock;
use egui_wgpu::wgpu;
use std::sync::Arc;

pub struct Texture {
    pub size: wgpu::Extent3d,
    pub view: wgpu::TextureView,
    pub handle: wgpu::Texture,
}

impl Texture {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let handle = device.create_texture(&wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage,
            label,
        });
        let view = handle.create_view(&wgpu::TextureViewDescriptor::default());
        Self { size, view, handle }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, width: u32, height: u32, data: &[u8]) {
        assert!(width == self.size.width && height == self.size.height);
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.handle,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * width),
                rows_per_image: std::num::NonZeroU32::new(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn egui_register(
        &self,
        device: &wgpu::Device,
        renderer: &Arc<RwLock<egui_wgpu::Renderer>>,
    ) -> egui::TextureId {
        let mut renderer = renderer.write();
        let egui_id =
            renderer.register_native_texture(device, &self.view, wgpu::FilterMode::Nearest);
        renderer.update_egui_texture_from_wgpu_texture(
            &device,
            &self.view,
            wgpu::FilterMode::Nearest,
            egui_id,
        );
        egui_id
    }
}
