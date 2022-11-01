pub struct TextureImage {
    pub size: wgpu::Extent3d,
    handle: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub egui_id: egui::TextureId,
}

impl TextureImage {
    pub fn new(render_state: &egui_wgpu::RenderState, size: wgpu::Extent3d) -> Self {
        let device = &render_state.device;
        let mut renderer = render_state.renderer.write();
        let handle = device.create_texture(&wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("egui_texture"),
        });
        let view = handle.create_view(&wgpu::TextureViewDescriptor::default());
        let egui_id = renderer.register_native_texture(
            &render_state.device,
            &view,
            wgpu::FilterMode::Nearest,
        );
        Self {
            size,
            handle,
            view,
            egui_id,
        }
    }

    pub fn update(&self, render_state: &egui_wgpu::RenderState, data: &[u8]) {
        render_state.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.handle,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * self.size.width),
                rows_per_image: std::num::NonZeroU32::new(self.size.height),
            },
            self.size,
        );
        let mut renderer = render_state.renderer.write();
        renderer.update_egui_texture_from_wgpu_texture(
            &render_state.device,
            &self.view,
            wgpu::FilterMode::Nearest,
            self.egui_id,
        );
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let aspect_ratio = self.size.height as f32 / self.size.width as f32;
        let width = ui.available_width();
        let height = width * aspect_ratio;
        ui.add(egui::Image::new(self.egui_id, [width, height]));
    }
}
