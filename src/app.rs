use winit::event::WindowEvent;

use crate::renderer::Renderer;

pub struct App<'a> {
    pub renderer: Option<Renderer<'a>>,
}

impl winit::application::ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.renderer = Some(Renderer::new(&event_loop));
        self.renderer.as_mut().unwrap().window.request_redraw();
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                self.renderer
                    .as_mut()
                    .unwrap()
                    .resize_dependent_component_rebuild_needed = true;
            }
            WindowEvent::RedrawRequested => {
                self.renderer.as_mut().unwrap().draw_frame();
                self.renderer.as_mut().unwrap().window.request_redraw();
            }
            _ => (),
        }
    }
}
