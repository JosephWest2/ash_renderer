use std::env;

use winit::event_loop::{ControlFlow, EventLoop};

mod app;
mod renderer;
mod shaders;
mod test;


fn main() {
    env::set_var("RUST_BACKTRACE", "full");





    let mut app = app::App { renderer: None };
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    _ = event_loop.run_app(&mut app);
}
