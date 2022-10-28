mod gui;
mod window;

use winit::event::Event::*;
use winit::event_loop::ControlFlow;

fn main() {
    let event_loop =
        winit::event_loop::EventLoopBuilder::<window::RedrawEvent>::with_user_event().build();
    let mut window = window::Window::new(&event_loop);
    let mut egui = gui::EguiContext::new(&window);
    let mut demo_app = egui_demo_lib::DemoWindows::default();
    event_loop.run(move |event, _, control_flow| {
        // Pass the winit events to the platform integration.
        egui.platform.handle_event(&event);

        match event {
            RedrawRequested(..) => {
                let output_frame = match window.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Outdated) => {
                        // This error occurs when the app is minimized on Windows.
                        // Silently return here to prevent spamming the console with:
                        // "The underlying surface has changed, and therefore the swap chain must be updated"
                        return;
                    }
                    Err(e) => {
                        eprintln!("Dropped frame with error: {}", e);
                        return;
                    }
                };
                let output_view = output_frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    window
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("encoder"),
                        });

                let ui = egui.begin_frame();
                demo_app.ui(&ui);
                egui.render(&window, &mut encoder, &output_view);

                // Submit the commands.
                window.queue.submit(std::iter::once(encoder.finish()));

                // Redraw egui
                output_frame.present();
                egui.end_frame();
            }
            MainEventsCleared | UserEvent(window::RedrawEvent::RequestRedraw) => {
                window.window.request_redraw();
            }
            WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::Resized(size) => {
                    window.resize(size);
                }
                winit::event::WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            _ => (),
        }
    });
}
