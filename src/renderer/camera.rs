use std::f32::consts::PI;

use nalgebra::{Matrix4, Perspective3, Point3, Vector3};

// all angles are in radians
#[derive(Debug)]
pub struct Camera {
    pub position: Point3<f32>,
    // angle off of the vertical axis
    // radians
    pub phi: f32,
    // angle clockwise about the vertical axis
    // radians
    pub theta: f32,
    up: Vector3<f32>,
    aspect_ratio: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}
#[rustfmt::skip]
pub const MODEL_MATRIX: Matrix4<f32> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
);

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Point3::new(0.0, 0.0, 0.0),
            phi: PI / 4.0,
            theta: 0.0,
            up: Vector3::y_axis().into_inner(),
            aspect_ratio: 4.0 / 3.0,
            fovy: 45.0,
            znear: 0.01,
            zfar: 100.0,
        }
    }
    fn forward(&self) -> Vector3<f32> {
        let forward = Vector3::new(
            self.phi.sin() * self.theta.sin(),
            self.phi.cos(),
            self.phi.sin() * self.theta.cos(),
        );
        dbg!(forward);
        forward
    }
    pub fn view_matrix(&self) -> Matrix4<f32> {
        Matrix4::look_at_rh(&self.position, &(self.position + self.forward()), &self.up)
    }
    pub fn projection_matrix(&self) -> Matrix4<f32> {
        Perspective3::new(self.aspect_ratio, self.fovy, self.znear, self.zfar).into_inner()
    }
}

#[derive(Debug)]
pub struct CameraController {
    pub speed: f32,
    pub mouse_sens: f32,
    pub mouse_delta: (f32, f32),
    pub forward_pressed: bool,
    pub backward_pressed: bool,
    pub left_pressed: bool,
    pub right_pressed: bool,
}

impl CameraController {
    pub fn new(speed: f32, mouse_sens: f32) -> Self {
        Self {
            speed,
            mouse_sens,
            mouse_delta: (0.0, 0.0),
            forward_pressed: false,
            backward_pressed: false,
            left_pressed: false,
            right_pressed: false,
        }
    }

    pub fn update_camera(&mut self, camera: &mut Camera) {
        let forward = camera.forward();
        let right = forward.cross(&Vector3::y_axis());
        if self.forward_pressed {
            camera.position += forward * self.speed;
        }
        if self.backward_pressed {
            camera.position -= forward * self.speed;
        }
        if self.left_pressed {
            camera.position -= right * self.speed;
        }
        if self.right_pressed {
            camera.position += right * self.speed;
        }
        camera.theta += self.mouse_delta.0 * self.mouse_sens;
        camera.phi -= self.mouse_delta.1 * self.mouse_sens;
        self.mouse_delta = (0.0, 0.0);
    }
}
