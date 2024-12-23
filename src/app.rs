use winit::event::{DeviceEvent, WindowEvent};

use crate::renderer::{camera::{self, CameraController}, Renderer};

pub struct App {
    pub renderer: Option<Renderer>,
    pub camera: Option<camera::Camera>,
    pub camera_controller: Option<CameraController>,
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.renderer = Some(Renderer::new(&event_loop));
        self.camera = Some(camera::Camera::new());
        self.camera_controller = Some(CameraController::new(0.01, 0.01));
        self.renderer.as_mut().unwrap().window.request_redraw();
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                let camera_controller = self.camera_controller.as_mut().unwrap();
                camera_controller.mouse_delta_x += delta.0 as f32;
                camera_controller.mouse_delta_y += delta.1 as f32;
            }
            _ => (),
        }
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
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } => {
                use winit::keyboard::{KeyCode, PhysicalKey};
                let is_pressed = event.state.is_pressed();
                let camera_controller = self.camera_controller.as_mut().unwrap();
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyA) | PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        camera_controller.left_pressed = is_pressed;
                    }
                    PhysicalKey::Code(KeyCode::KeyD) | PhysicalKey::Code(KeyCode::ArrowRight) => {
                        camera_controller.right_pressed = is_pressed;
                    }
                    PhysicalKey::Code(KeyCode::KeyS) | PhysicalKey::Code(KeyCode::ArrowDown) => {
                        camera_controller.backward_pressed = is_pressed;
                    }
                    PhysicalKey::Code(KeyCode::KeyW) | PhysicalKey::Code(KeyCode::ArrowUp) => {
                        camera_controller.forward_pressed = is_pressed;
                    }
                    _ => (),
                }
            }
            WindowEvent::RedrawRequested => {
                self.camera_controller.as_mut().unwrap().update_camera(self.camera.as_mut().unwrap());
                self.renderer.as_mut().unwrap().draw_frame(self.camera.as_ref().unwrap());
                self.renderer.as_mut().unwrap().window.request_redraw();
            }
            _ => (),
        }
    }
}
