use std::f32::consts::PI;

use nalgebra::{Matrix4, Perspective3, Point3, Vector3};

// all angles are in radians
#[derive(Debug)]
pub struct Camera {
    pub position: Point3<f32>,
    // angle off of the vertical axis, 0 is up
    // radians
    pub phi: f32,
    // angle counterclockwise about the vertical axis, 0 is in the z direction
    // radians
    pub theta: f32,
    up: Vector3<f32>,
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
            phi: PI / 2.0,
            theta: 0.0,
            up: Vector3::y_axis().scale(-1.0),
            fovy: 45.0,
            znear: 0.01,
            zfar: 100.0,
        }
    }
    fn forward(&self) -> Vector3<f32> {
        let forward = Vector3::new(
            self.phi.sin() * self.theta.sin(),
            -1.0 * self.phi.cos(),
            self.phi.sin() * self.theta.cos(),
        );
        forward
    }
    pub fn view_matrix(&self) -> Matrix4<f32> {
        let look_at =
            Matrix4::look_at_rh(&self.position, &(self.position + self.forward()), &self.up);
        #[rustfmt::skip]
        let negative_y = Matrix4::new(
            1.0, 0.0, 0.0, 0.0,
            0.0, -1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        );
        negative_y * look_at
    }
    pub fn projection_matrix(&self, aspect_ratio: f32) -> Matrix4<f32> {
        Perspective3::new(aspect_ratio, self.fovy, self.znear, self.zfar).to_homogeneous()
    }
}

#[derive(Debug)]
pub struct CameraController {
    pub speed: f32,
    pub mouse_sens: f32,
    pub mouse_delta_x: f32,
    pub mouse_delta_y: f32,
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
            mouse_delta_x: 0.0,
            mouse_delta_y: 0.0,
            forward_pressed: false,
            backward_pressed: false,
            left_pressed: false,
            right_pressed: false,
        }
    }

    pub fn update_camera(&mut self, camera: &mut Camera) {
        let forward = camera.forward();
        let right = forward.cross(&Vector3::y_axis().scale(-1.0));
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
        camera.theta += self.mouse_delta_x * self.mouse_sens;
        camera.phi += self.mouse_delta_y * self.mouse_sens;
        self.mouse_delta_x = 0.0;
        self.mouse_delta_y = 0.0;
    }
}
