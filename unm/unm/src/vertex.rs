use glam::{Vec2, Vec3};
use wgpu::Color;

#[repr(C)] // 确保内存布局与 C 兼容
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3], // X, Y, Z
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    pub fn new(pos: Vec3, uv: Vec2, color: Color) -> Self {
        Self {
            position: pos.to_array(),
            uv: uv.to_array(),
            color: [
                color.r as f32,
                color.g as f32,
                color.b as f32,
                color.a as f32,
            ],
        }
    }
}

impl Vertex {
    // 使用宏自动计算偏移量和属性
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3, // shader_location 0
        1 => Float32x2, // shader_location 1
        2 => Float32x4, // shader_location 2
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub fn calculate_object_center(vertices: &[Vertex]) -> glam::Vec3 {
    if vertices.is_empty() {
        return glam::Vec3::ZERO; // 或您认为合适的默认值
    }

    let mut sum_position = glam::Vec3::ZERO;
    for vertex in vertices {
        sum_position += glam::Vec3::from_slice(&vertex.position);
    }
    sum_position / (vertices.len() as f32)
}
