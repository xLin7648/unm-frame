use std::collections::HashMap;

use crate::{material::MaterialHandle, render_target::RenderTargetHandle, uniform::Uniform, vertex::Vertex};

pub(crate) struct RenderCommand {
    pub(crate) id: u32,
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u32>,

    pub(crate) mat_handle: MaterialHandle,
    pub(crate) uniforms: Option<HashMap<String, Uniform>>,

    pub(crate) render_target: RenderTargetHandle,
    pub(crate) render_queue: u32,
    pub(crate) depth: f32,
}

impl RenderCommand {
    pub fn new(
        id: u32,
        vertices: &[Vertex],
        indices: &[u32],
        mat_handle: MaterialHandle,
        render_target: RenderTargetHandle,
        z_order: u32,
        depth: f32
    ) -> Self {
        Self {
            id,
            render_queue: z_order,
            vertices: vertices.to_vec(),
            indices: indices.to_vec(),
            uniforms: mat_handle.get_all_uniform(),

            depth,
            mat_handle,
            render_target,
        }
    }
}