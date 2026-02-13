use std::collections::HashMap;

use crate::{get_context, get_quad_context, material::MaterialHandle, render_command::RenderCommand, render_target::{RenderTarget, RenderTargetHandle}, uniform::Uniform};

#[derive(Default)]
pub struct DrawCall {
    pub vertices_count: usize,
    pub indices_count: usize,
    pub vertices_start: usize,
    pub indices_start: usize,

    pub mat_handle: MaterialHandle,
    pub uniforms: Option<HashMap<String, Uniform>>,

    pub render_target: RenderTargetHandle
}

impl DrawCall {
    pub fn new(command: RenderCommand) -> DrawCall {
        DrawCall {
            vertices_start: 0,
            indices_start: 0,
            vertices_count: 0,
            indices_count: 0,
            // viewport: None,
            // clip: None,
            // texture,
            // model,
            // draw_mode,
            mat_handle: command.mat_handle,
            uniforms: command.uniforms,
            // render_pass,
            // capture: false,

            render_target: command.render_target
        }
    }
}