use crate::window;

pub struct EguiContext {
    pub platform: egui_winit_platform::Platform,
    renderpass: egui_wgpu_backend::RenderPass,
    start_time: std::time::Instant,
    tdelta: std::option::Option<egui::TexturesDelta>,
}

impl EguiContext {
    pub fn new(window: &window::Window) -> EguiContext {
        // We use the egui_winit_platform crate as the platform.
        let platform =
            egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
                physical_width: window.surface_config.width,
                physical_height: window.surface_config.height,
                scale_factor: window.window.scale_factor(),
                font_definitions: egui::FontDefinitions::default(),
                style: Default::default(),
            });
        // We use the egui_wgpu_backend crate as the render backend.
        let renderpass =
            egui_wgpu_backend::RenderPass::new(&window.device, window.surface_config.format, 1);
        let start_time = std::time::Instant::now();
        EguiContext {
            platform,
            renderpass,
            start_time,
            tdelta: None,
        }
    }

    pub fn begin_frame(&mut self) -> egui::Context {
        self.platform
            .update_time(self.start_time.elapsed().as_secs_f64());
        // Begin to draw the UI frame.
        self.platform.begin_frame();
        // Draw the demo application.
        self.platform.context()
    }

    pub fn end_frame(&mut self) {
        if let Some(tdelta) = self.tdelta.take() {
            self.renderpass
                .remove_textures(tdelta)
                .expect("remove texture ok");
        }
    }

    pub fn render(
        &mut self,
        window: &window::Window,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
    ) {
        // End the UI frame. We could now handle the output and draw the UI with the backend.
        let full_output = self.platform.end_frame(Some(&window.window));
        let paint_jobs = self.platform.context().tessellate(full_output.shapes);

        // Upload all resources for the GPU.
        let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
            physical_width: window.surface_config.width,
            physical_height: window.surface_config.height,
            scale_factor: window.window.scale_factor() as f32,
        };
        let tdelta = full_output.textures_delta;
        self.renderpass
            .add_textures(&window.device, &window.queue, &tdelta)
            .expect("add texture ok");
        self.tdelta = Some(tdelta);
        self.renderpass.update_buffers(
            &window.device,
            &window.queue,
            &paint_jobs,
            &screen_descriptor,
        );

        // Record all render passes.
        self.renderpass
            .execute(
                encoder,
                &output_view,
                &paint_jobs,
                &screen_descriptor,
                Some(wgpu::Color::BLACK),
            )
            .unwrap();
    }
}
