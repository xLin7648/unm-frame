use core::panic;
use std::fmt::Debug;
use glam::{Mat4, Quat, Vec3, UVec2, EulerRot};
use log::info;

use crate::render_target::RenderTargetHandle; // 引入glam的类型

#[allow(dead_code)]
pub trait Camera: Send + Sync + Debug {
    fn matrix(&self) -> Mat4;
    fn resize(&mut self, size: UVec2);

    fn get_position(&self) -> Vec3;
    fn get_rotation(&self) -> Quat;

    fn set_position(&mut self, position: Vec3);
    fn set_rotation(&mut self, rotation: Quat);
    fn set_rotation_angle(&mut self, angle: Vec3);

    fn get_render_target(&self) -> Option<RenderTargetHandle>;
    fn set_render_target(&mut self, new_rt: Option<RenderTargetHandle>);

    fn get_forward(&self) -> Vec3;
}

#[derive(Debug)]
pub struct BaseCamera {
    pos: Vec3,
    rot: Quat,
    target: Vec3,
    near: f32,
    far: f32,

    render_target: Option<RenderTargetHandle>
}

impl BaseCamera {
    pub fn new(pos: Vec3, near: f32, far: f32) -> Self {
        let mut camera = Self {
            pos,
            near,
            far,
            target: Vec3::ZERO,
            rot: Quat::IDENTITY,
            render_target: None
        };
        camera.update_target();
        camera
    }

    // 设置位置，同时更新目标
    pub fn set_position(&mut self, position: Vec3) {
        self.pos = position;
        self.update_target();
    }

    // 设置旋转，同时更新目标，参数从 Vec3 更改为 Quat
    pub fn set_rotation(&mut self, rotation: Quat) {
        self.rot = rotation;
        self.update_target(); // 更新目标方向
    }

    pub fn set_rotation_angle(&mut self, angle: Vec3) {
        // 将欧拉角转换为四元数
        self.rot = Quat::from_euler(
            EulerRot::XYZ,
            angle.x.to_radians(),
            angle.y.to_radians(),
            angle.z.to_radians(),
        );
        self.update_target(); // 更新目标
    }

    // 更新目标位置
    fn update_target(&mut self) {
        // 在右手坐标系中，默认的“向前”方向通常是负Z轴。
        // 所以这里我们将旋转应用到 Vec3::NEG_Z。
        let direction = self.rot * Vec3::NEG_Z;
        self.target = self.pos + direction;
    }

    pub fn get_view_direction(&self) -> Vec3 {
        // 与 update_target 逻辑一致：右手坐标系中，前向是负 Z
        self.rot * Vec3::NEG_Z
    }
}

impl Default for BaseCamera {
    fn default() -> Self {
        Self::new(Vec3::ZERO, 0.01, 1000.0)
    }
}

#[derive(Debug)]
pub struct Camera3D {
    base: BaseCamera,
    fovy: f32,
    aspect: f32,
}

#[allow(dead_code)]
impl Camera3D {
    pub fn new(base: BaseCamera, fovy: f32) -> Self {
        Self {
            base,
            fovy,
            aspect: 0.0,
        }
    }
}

impl Camera for Camera3D {
    fn matrix(&self) -> Mat4 {
        let base = &self.base;
        let up = base.rot * Vec3::Y; // Y轴作为上方向
        // 使用右手坐标系函数
        let view = Mat4::look_at_rh(base.pos, base.target, up);
        let proj = Mat4::perspective_rh(self.fovy.to_radians(), self.aspect, base.near, base.far);
        proj * view
    }

    fn resize(&mut self, new_size: UVec2) {
        self.aspect = new_size.x as f32 / new_size.y as f32; // 更新宽高比
    }

    fn set_rotation(&mut self, rotation: Quat) {
        self.base.set_rotation(rotation);
    }

    fn set_rotation_angle(&mut self, angle: Vec3) {
        self.base.set_rotation_angle(angle); // 调用 BaseCamera 的方法
    }

    fn set_position(&mut self, position: Vec3) {
        self.base.set_position(position);
    }

    fn get_render_target(&self) -> Option<RenderTargetHandle> {
        self.base.render_target
    }

    fn set_render_target(&mut self, new_rt: Option<RenderTargetHandle>) {
        self.base.render_target = new_rt;
    }

    fn get_position(&self) -> Vec3 {
        self.base.pos
    }

    fn get_rotation(&self) -> Quat {
        self.base.rot
    }

    fn get_forward(&self) -> Vec3 {
        self.base.get_view_direction()
    }
}

// 假设 Rect 结构体定义如下，为了编译通过，我添加了默认实现
#[derive(Debug, Default, Copy, Clone)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}


#[derive(Debug)]
pub struct Camera2D {
    base: BaseCamera,
    rect: Rect,
    size: UVec2,
}

#[allow(dead_code)]
impl Camera2D {
    pub fn new(mut base: BaseCamera, size: UVec2) -> Self {
        // 对于2D相机，通常我们将它放置在 (0,0,Z) 处，朝向负Z轴
        // 并且通常不旋转，或者只围绕Z轴旋转。
        // 为了与右手坐标系渲染保持一致，通常相机看向负Z轴。
        // 如果你的2D场景是XY平面，那么相机的位置和方向需要仔细考虑。
        // 简单起见，我们假设2D相机也看向负Z轴，并且其“位置”只影响XY平面的平移。
        base.set_rotation(Quat::IDENTITY); // 2D相机通常不旋转

        let mut camera2d = Self {
            base,
            size,
            rect: Rect::default(),
        };
        camera2d.resize(size); // 在初始化时调用 resize 设置 rect
        camera2d
    }
}

impl Camera for Camera2D {
    fn matrix(&self) -> Mat4 {
        let base = &self.base;
        let up = base.rot * Vec3::Y; // Y轴仍然是上方向

        // 使用右手坐标系函数
        let view = Mat4::look_at_rh(base.pos, base.target, up);

        // orthographic_rh 的参数是 (left, right, bottom, top, near, far)
        // 注意，在右手坐标系中，near和far通常表示距离相机的绝对值。
        // 如果你的2D场景的Y轴通常向上，X轴向右，那么left, right, bottom, top应该相应设置。
        let proj = Mat4::orthographic_rh(
            self.rect.x,      // left
            self.rect.y,      // right
            self.rect.w,      // bottom
            self.rect.h,      // top
            base.near,
            base.far,
        );
        proj * view
    }

    fn resize(&mut self, size: UVec2) {
        self.size = size;

        let (half_width, half_height) = (self.size.x as f32 / 2.0, self.size.y as f32 / 2.0);

        // 调整 rect 的值以适应右手坐标系的正交投影
        // left, right, bottom, top
        self.rect = Rect {
            x: -half_width,  // left
            y:  half_width,  // right
            w: -half_height, // bottom (通常Y轴向上，所以画布底部是负值)
            h:  half_height, // top (画布顶部是正值)
        };
    }

    fn set_position(&mut self, position: Vec3) {
        self.base.set_position(position);
    }

    fn set_rotation(&mut self, rotation: Quat) {
        self.base.set_rotation(rotation);
    }

    fn set_rotation_angle(&mut self, angle: Vec3) {
        self.base.set_rotation_angle(angle); // 调用 BaseCamera 的方法
    }

    fn get_render_target(&self) -> Option<RenderTargetHandle> {
        self.base.render_target
    }

    fn set_render_target(&mut self, new_rt: Option<RenderTargetHandle>) {
        self.base.render_target = new_rt;
    }

    fn get_position(&self) -> Vec3 {
        self.base.pos
    }

    fn get_rotation(&self) -> Quat {
        self.base.rot
    }

    fn get_forward(&self) -> Vec3 {
        self.base.get_view_direction()
    }
}

// 用于相机的统一缓存
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

#[allow(dead_code)]
impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    pub fn update_matrix(&mut self, matrix: Mat4) {
        self.view_proj = matrix.to_cols_array_2d();
    }
}