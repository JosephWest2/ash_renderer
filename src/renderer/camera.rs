use nalgebra::{Matrix4, Point3, Vector3};

// all angles are in radians
pub struct Camera {
    position: Point3<f32>,
    pitch: f32,
    yaw: f32,
    up: Vector3<f32>,
    aspect_ratio: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Point3::new(0.0, 0.0, 0.0),
            pitch: 0.0,
            yaw: 0.0,
            up: Vector3::y_axis().into_inner(),
            aspect_ratio: 16.0 / 9.0,
            fovy: 45.0,
            znear: 0.01,
            zfar: 100.0,
        }
    }
    fn camera_forward(&self) -> Vector3<f32> {
        Vector3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            -1.0 * self.pitch.cos() * self.yaw.cos(),
        )
    }
    fn view_projection_matrix(&self) -> Matrix4<f32> {
        let view = nalgebra::Matrix4::look_at_rh(
            &self.position,
            &(self.position + self.camera_forward()),
            &self.up,
        );
        let proj = nalgebra::Perspective3::new(self.aspect_ratio, self.fovy, self.znear, self.zfar);
        proj.as_matrix() * view
    }
}
