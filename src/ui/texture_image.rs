use egui_wgpu::wgpu;

pub struct TextureImage {
    pub width: u32,
    pub height: u32,
    handle: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub egui_id: egui::TextureId,
}

fn initialize_texture(
    render_state: &egui_wgpu::RenderState,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView, egui::TextureId) {
    let device = &render_state.device;
    let mut renderer = render_state.renderer.write();
    let handle = device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: Some("egui_texture"),
    });
    let view = handle.create_view(&wgpu::TextureViewDescriptor::default());
    let egui_id =
        renderer.register_native_texture(&render_state.device, &view, wgpu::FilterMode::Nearest);
    (handle, view, egui_id)
}

impl TextureImage {
    pub fn new(render_state: &egui_wgpu::RenderState, width: u32, height: u32) -> Self {
        let (handle, view, egui_id) = initialize_texture(render_state, width, height);
        Self {
            width,
            height,
            handle,
            view,
            egui_id,
        }
    }

    pub fn update(
        &mut self,
        render_state: &egui_wgpu::RenderState,
        width: u32,
        height: u32,
        data: &[u8],
    ) {
        if width != self.width || height != self.height {
            let (handle, view, egui_id) = initialize_texture(render_state, width, height);
            self.handle = handle;
            self.view = view;
            self.egui_id = egui_id;
            self.width = width;
            self.height = height;
        }
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
                bytes_per_row: std::num::NonZeroU32::new(4 * width),
                rows_per_image: std::num::NonZeroU32::new(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let mut renderer = render_state.renderer.write();
        renderer.update_egui_texture_from_wgpu_texture(
            &render_state.device,
            &self.view,
            wgpu::FilterMode::Nearest,
            self.egui_id,
        );
    }

    pub fn show(&self, ui: &mut egui::Ui, scale: f32) {
        let aspect_ratio = self.height as f32 / self.width as f32;
        let width = ui.available_width() * scale;
        let height = width * aspect_ratio;
        ui.add(egui::Image::new(self.egui_id, [width, height]));
    }
}
