use cgmath::{Array, InnerSpace, Matrix4, Point3, SquareMatrix, Vector2, Vector3};
use winit::event::{DeviceEvent, ElementState, KeyboardInput, VirtualKeyCode, WindowEvent};

pub struct Camera {
    speed: f32,
    sensitivity: f32,

    mouse_delta_position: Vector2<f32>,
    view: Matrix4<f32>,
    proj: Matrix4<f32>,
    position: Vector3<f32>,
    front: Vector3<f32>,

    pitch: f32,
    yaw: f32,

    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,

    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    is_shift_pressed: bool,
    is_control_pressed: bool,
    is_alt_pressed: bool,
    is_space_pressed: bool,
    is_c_pressed: bool,
}

impl Camera {
    const UP_DIR: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

    pub fn new(start_position: Vector3<f32>, config: &wgpu::SurfaceConfiguration) -> Self {
        let mut cam = Camera {
            speed: 10.0,
            sensitivity: 5.0,

            mouse_delta_position: Vector2::from_value(0.0),
            // do_sync_mouse: true,
            // current_mouse_position: Vector2::new(0.0, 0.0),
            // previous_mouse_position: current_mouse_position,
            view: Matrix4::identity(),
            proj: Matrix4::identity(),
            position: start_position,
            front: Vector3::new(0.0, 0.0, -1.0),
            pitch: 0.0,
            yaw: 0.0,

            aspect: config.width as f32 / config.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,

            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            is_shift_pressed: false,
            is_control_pressed: false,
            is_alt_pressed: false,
            is_space_pressed: false,
            is_c_pressed: false,
        };

        cam.on_resize(config);

        cam
    }

    pub fn on_resize(&mut self, config: &wgpu::SurfaceConfiguration) {
        self.aspect = config.width as f32 / config.height as f32;
        self.proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
    }

    pub fn update(&mut self, delta_time: f32) {
        let mut speed = self.speed * delta_time;

        if self.is_shift_pressed {
            speed *= 5.0;
        }
        if self.is_control_pressed {
            speed *= 5.0;
        }
        if self.is_alt_pressed {
            speed /= 5.0;
        }

        let previous_position = self.position.clone();

        if self.is_forward_pressed {
            self.position += self.front * speed;
        }
        if self.is_backward_pressed {
            self.position -= self.front * speed;
        }
        if self.is_left_pressed {
            self.position -= self.front.cross(Self::UP_DIR).normalize() * speed;
        }
        if self.is_right_pressed {
            self.position += self.front.cross(Self::UP_DIR).normalize() * speed;
        }

        if self.is_space_pressed {
            self.position.y += speed;
        }
        if self.is_c_pressed {
            self.position.y -= speed;
        }

        let mouse_difference = Vector2::from(self.mouse_delta_position);
        self.mouse_delta_position = Vector2::from_value(0.0);

        if mouse_difference != Vector2::from_value(0.0) || self.position != previous_position {
            self.yaw += mouse_difference.x.to_radians() * self.sensitivity;
            self.pitch -= mouse_difference.y.to_radians() * self.sensitivity;

            if self.pitch > 89.9 {
                self.pitch = 89.9;
            } else if self.pitch < -89.9 {
                self.pitch = -89.9;
            }

            self.calculate_view();
        }
    }

    fn calculate_view(&mut self) {
        let direction = Vector3 {
            x: self.yaw.to_radians().cos() * self.pitch.to_radians().cos(),
            y: self.pitch.to_radians().sin(),
            z: self.yaw.to_radians().sin() * self.pitch.to_radians().cos(),
        };

        self.front = direction.normalize();

        // let right = Self::UP_DIR.cross(direction).normalize();

        // let up = direction.cross(right);

        self.view = Matrix4::look_at_rh(
            Point3::from(Into::<[f32; 3]>::into(self.position)),
            Point3::from(Into::<[f32; 3]>::into(self.position + self.front)),
            Self::UP_DIR,
        );
    }

    pub fn process_window_events(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => {
                let is_pressed = *state == ElementState::Pressed;
                match keycode {
                    VirtualKeyCode::W => {
                        self.is_forward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::A => {
                        self.is_left_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::S => {
                        self.is_backward_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::D => {
                        self.is_right_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::LShift => {
                        self.is_shift_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::LControl => {
                        self.is_control_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::LAlt => {
                        self.is_alt_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::Space => {
                        self.is_space_pressed = is_pressed;
                        true
                    }
                    VirtualKeyCode::C => {
                        self.is_c_pressed = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub fn process_device_events(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                self.mouse_delta_position = Vector2::new(delta.0 as f32, delta.1 as f32);
                true
            }
            _ => false,
        }
    }
}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

// We need this for Rust to store our data correctly for the shaders
#[repr(C)]
// This is so we can store this in a buffer
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    // We can't use cgmath with bytemuck directly so we'll have
    // to convert the Matrix4 into a 4x4 f32 array
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera3d: &Camera) {
        self.view_proj = (OPENGL_TO_WGPU_MATRIX * camera3d.proj * camera3d.view).into();
    }
}
