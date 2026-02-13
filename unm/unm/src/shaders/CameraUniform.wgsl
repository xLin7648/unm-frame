@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct CameraUniform {
    view_proj: mat4x4<f32>,
};