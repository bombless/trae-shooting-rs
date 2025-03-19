use winit::event::{WindowEvent, KeyboardInput, ElementState, VirtualKeyCode};
use gilrs::{GamepadId, EventType, Button, Axis};
use std::time::Duration;
use glam::{Vec3, Mat4, Quat};
use std::f32::consts::PI;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    view_position: [f32; 4],
}

// Manual implementation of bytemuck traits
unsafe impl bytemuck::Pod for CameraUniform {}
unsafe impl bytemuck::Zeroable for CameraUniform {}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            view_position: [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera, aspect: f32) {
        self.view_position = [camera.position.x, camera.position.y, camera.position.z, 1.0];
        let view = camera.calc_view();
        let proj = camera.calc_projection(aspect);
        self.view_proj = (proj * view).to_cols_array_2d();
    }
}

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // Horizontal rotation (left/right)
    pub pitch: f32,  // Vertical rotation (up/down)
}

impl Camera {
    pub fn new(position: (f32, f32, f32), yaw: f32, pitch: f32) -> Self {
        Self {
            position: Vec3::new(position.0, position.1, position.2),
            yaw,
            pitch,
        }
    }

    pub fn calc_view(&self) -> Mat4 {
        // First rotate around Y axis (yaw)
        let yaw_rotation = Quat::from_rotation_y(self.yaw);
        
        // Then rotate around X axis (pitch)
        let pitch_rotation = Quat::from_rotation_x(self.pitch);
        
        // Combine rotations
        let rotation = yaw_rotation * pitch_rotation;
        
        // Calculate view matrix
        let view = Mat4::from_rotation_translation(
            rotation,
            self.position,
        );
        
        // Invert the view matrix
        view.inverse()
    }

    pub fn calc_projection(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(
            70.0 * (PI / 180.0), // 70 degree FOV
            aspect,
            0.1,  // near plane
            100.0, // far plane
        )
    }
}

pub struct CameraController {
    speed: f32,
    sensitivity: f32,
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    // Controller state
    left_stick_x: f32,
    left_stick_y: f32,
    right_stick_x: f32,
    right_stick_y: f32,
    mouse_move_x: f32,
    mouse_move_y: f32,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            speed,
            sensitivity,
            forward: false,
            backward: false,
            left: false,
            right: false,
            left_stick_x: 0.0,
            left_stick_y: 0.0,
            right_stick_x: 0.0,
            right_stick_y: 0.0,
            mouse_move_x: 0.0,
            mouse_move_y: 0.0,
        }
    }

    pub fn process_keyboard(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    state,
                    virtual_keycode: Some(keycode),
                    ..
                },
                ..
            } => {
                let is_pressed = *state == ElementState::Pressed;
                match keycode {
                    VirtualKeyCode::W => {
                        self.forward = is_pressed;
                        true
                    }
                    VirtualKeyCode::S => {
                        self.backward = is_pressed;
                        true
                    }
                    VirtualKeyCode::A => {
                        self.left = is_pressed;
                        true
                    }
                    VirtualKeyCode::D => {
                        self.right = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, dx: f64, dy: f64) {
        // Convert to f32 and apply sensitivity
        let dx = dx as f32 * self.sensitivity;
        let dy = dy as f32 * self.sensitivity;
        
        // Update camera rotation (yaw and pitch will be applied to the camera in update_camera)
        self.mouse_move_x = -dx * 0.7; // Invert X axis to fix reversed mouse direction
        self.mouse_move_y = -dy * 0.7; // Invert Y axis for intuitive control
    }

    pub fn process_controller(&mut self, _id: &GamepadId, event: &EventType) {
        match event {
            EventType::ButtonPressed(button, _) => {
                match button {
                    Button::DPadUp => self.forward = true,
                    Button::DPadDown => self.backward = true,
                    Button::DPadLeft => self.left = true,
                    Button::DPadRight => self.right = true,
                    _ => {},
                }
            },
            EventType::ButtonReleased(button, _) => {
                match button {
                    Button::DPadUp => self.forward = false,
                    Button::DPadDown => self.backward = false,
                    Button::DPadLeft => self.left = false,
                    Button::DPadRight => self.right = false,
                    _ => {},
                }
            },
            EventType::AxisChanged(axis, value, _) => {
                match axis {
                    Axis::LeftStickX => self.left_stick_x = *value,
                    Axis::LeftStickY => self.left_stick_y = *value,
                    Axis::RightStickX => {
                        let dx = *value;  // 将摇杆值转换为类似鼠标的增量
                        self.right_stick_x = -dx * self.sensitivity * 0.7;
                    },
                    Axis::RightStickY => {
                        let dy = *value;
                        self.right_stick_y = dy * self.sensitivity * 0.7;
                    },
                    _ => {},
                }
            },
            _ => {},
        }
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt: Duration) {
        // Convert duration to seconds for smooth movement
        let dt = dt.as_secs_f32();
        
        // Calculate forward and right vectors based on camera's current orientation
        let forward = Vec3::new(
            camera.yaw.sin(),
            0.0,
            camera.yaw.cos(),
        ).normalize();
        
        let right = Vec3::new(
            (camera.yaw - PI/2.0).sin(),
            0.0,
            (camera.yaw - PI/2.0).cos(),
        ).normalize();
        
        // Process keyboard/D-pad movement
        if self.forward {
            camera.position -= forward * self.speed * dt;
        }
        if self.backward {
            camera.position += forward * self.speed * dt;
        }
        if self.right {
            camera.position -= right * self.speed * dt;
        }
        if self.left {
            camera.position += right * self.speed * dt;
        }
        
        // Process controller left stick movement
        if self.left_stick_x.abs() > 0.1 || self.left_stick_y.abs() > 0.1 {
            camera.position -= right * self.left_stick_x * self.speed * dt;
            camera.position -= forward * self.left_stick_y * self.speed * dt;
        }
        
        // Process mouse/controller right stick for camera rotation
        camera.yaw += self.right_stick_x * self.sensitivity * dt * 2.0;
        camera.pitch += self.right_stick_y * self.sensitivity * dt * 2.0;
        camera.yaw += self.mouse_move_x * self.sensitivity * dt * 2.0;
        camera.pitch += self.mouse_move_y * self.sensitivity * dt * 2.0;
        
        self.mouse_move_x = 0.0;
        self.mouse_move_y = 0.0;
        
        // Clamp pitch to avoid camera flipping
        camera.pitch = camera.pitch.clamp(-PI/2.0 + 0.1, PI/2.0 - 0.1);
        
        // Ensure camera doesn't go below the floor
        if camera.position.y < 1.0 {
            camera.position.y = 1.0;
        }
    }
}