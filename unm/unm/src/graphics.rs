use std::collections::{HashMap, HashSet, VecDeque};

use glam::{uvec2, vec2, vec3, Mat4, Quat, UVec2, Vec3};
use image::GenericImageView;
use log::*;
use unm_tools::id_map::IdMap;
use wgpu::{
    util::{self, DeviceExt},
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferUsages,
    CommandEncoderDescriptor, Extent3d, IndexFormat, Origin3d, PipelineLayout, RenderPass,
    SamplerDescriptor, ShaderStages, SurfaceError, TexelCopyTextureInfo, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use winit::{dpi::PhysicalSize, window::Window};

#[allow(unused_imports)] // 暂时允许未使用的导入
use crate::{
    camera::{Camera, CameraUniform},
    draw_call::DrawCall,
    game_settings::GameSettings,
    material::{Material, MaterialDescriptor, MaterialHandle},
    msaa::Msaa,
    render_context::RenderContext,
    render_target::{RenderTarget, RenderTargetHandle},
    uniform::{Uniform, UniformDef},
    utils::{BufferType, SizedBuffer},
    vertex::Vertex,
};
use crate::{
    draw_call, get_context, get_quad_context,
    render_command::RenderCommand,
    texture::{Texture2D, Texture2DHandle},
    vertex::calculate_object_center,
};

// 新增的 PassAction 枚举，用于指示渲染通道的加载行为
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PassAction {
    Clear(wgpu::Color), // 使用给定颜色清除
    Load,               // 加载前一个通道的内容
}

impl PassAction {
    pub fn load_op(&self) -> wgpu::LoadOp<wgpu::Color> {
        match *self {
            PassAction::Clear(color) => wgpu::LoadOp::Clear(color),
            PassAction::Load => wgpu::LoadOp::Load,
        }
    }
}

#[allow(dead_code)]
pub struct WgpuState {
    pub(crate) size: PhysicalSize<u32>, // 这应该代表物理窗口的大小
    pub(crate) context: RenderContext,

    global_vertex_buffer: SizedBuffer,
    global_index_buffer: SizedBuffer,

    batch_vertex_buffer: Vec<Vertex>,
    batch_index_buffer: Vec<u32>,

    camera_uniform: CameraUniform,
    camera_buffer: Buffer,
    camera_bind_group: BindGroup,
    camera_bind_group_layout: BindGroupLayout,

    camera: Option<Box<dyn Camera + Send + Sync>>,

    default_render_target: RenderTargetHandle,

    basic_shapes_triangle_mat: MaterialHandle,
    basic_shapes_lines_mat: MaterialHandle,
    basic_shapes_points_mat: MaterialHandle,

    msaa: Msaa,

    pub(crate) render_targets: IdMap<RenderTarget, RenderTargetHandle>,
    pub(crate) materials: IdMap<Material, MaterialHandle>,
    pub(crate) texture2ds: IdMap<Texture2D, Texture2DHandle>,

    current_material: Option<MaterialHandle>,

    render_commands: Vec<RenderCommand>,
    draw_calls: Vec<DrawCall>,

    pub(crate) break_batching: bool,

    max_vertices: usize,
    max_indices: usize,
}

impl WgpuState {
    pub(crate) async fn new(window: &'static Window) -> anyhow::Result<Self> {
        let size: PhysicalSize<u32> = window.inner_size();
        info!("Initializing WGPU for window size: {:?}", size);

        let context = RenderContext::new(window, size).await?;

        let camera_uniform = CameraUniform::new();
        let camera_buffer = context
            .device
            .create_buffer_init(&util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });
        let camera_bind_group_layout: wgpu::BindGroupLayout = context
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });
        let camera_bind_group = context.device.create_bind_group(&BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let max_vertices: usize = 1024 * 1024;
        let max_indices: usize = 1024 * 1024;

        let vertex_buffer = SizedBuffer::new(
            "Mesh Vertex Buffer",
            &context.device,
            max_vertices,
            BufferType::Vertex,
        );

        let index_buffer = SizedBuffer::new(
            "Mesh Index Buffer",
            &context.device,
            max_indices,
            BufferType::Index,
        );

        Ok(Self {
            context,
            size,

            global_vertex_buffer: vertex_buffer,
            global_index_buffer: index_buffer,

            batch_vertex_buffer: Vec::with_capacity(max_vertices),
            batch_index_buffer: Vec::with_capacity(max_indices),

            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_bind_group_layout,

            camera: None,

            default_render_target: RenderTargetHandle::default(), // 将在 `create_default_rt` 中设置

            msaa: Msaa::Off,

            render_targets: IdMap::<RenderTarget, RenderTargetHandle>::new(),
            materials: IdMap::<Material, MaterialHandle>::new(),
            texture2ds: IdMap::<Texture2D, Texture2DHandle>::new(),

            basic_shapes_triangle_mat: MaterialHandle::default(),
            basic_shapes_lines_mat: MaterialHandle::default(),
            basic_shapes_points_mat: MaterialHandle::default(),
            current_material: None,

            render_commands: Vec::with_capacity(200),
            draw_calls: Vec::with_capacity(200),

            break_batching: false,

            max_vertices,
            max_indices,
        })
    }

    pub(crate) async fn create_default_resources(&mut self) {
        self.create_default_rt();

        let basic_shapes_shader_str = include_str!("shaders/BasicShapes.wgsl").to_string();

        self.basic_shapes_triangle_mat = create_material(
            "BasicShapes Triangle".to_owned(),
            basic_shapes_shader_str.clone(),
            MaterialDescriptor::triangle(),
            None,
        )
        .await
        .unwrap_or_default();

        self.current_material = Some(self.basic_shapes_triangle_mat);

        self.basic_shapes_lines_mat = create_material(
            "BasicShapes Lines".to_owned(),
            basic_shapes_shader_str.clone(),
            MaterialDescriptor::lines(),
            None,
        )
        .await
        .unwrap_or_default();

        self.basic_shapes_points_mat = create_material(
            "BasicShapes Points".to_owned(), // 修正标签
            basic_shapes_shader_str.clone(),
            MaterialDescriptor::lines(), // 如果你有 Points 专用的 MaterialDescriptor，请用它
            None,
        )
        .await
        .unwrap_or_default();
    }

    // 窗口大小改变时调用
    pub(crate) fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size == self.size || (new_size.width == 0 || new_size.height == 0) {
            return;
        }

        self.size = new_size;

        // Surface Resize
        self.context.resize(self.size);

        // 重新创建默认 RT，因为其底层的 SwapChain 纹理视图需要更新
        self.create_default_rt();
    }

    pub fn screen_width(&self) -> f32 {
        self.size.width as f32
    }

    pub fn screen_height(&self) -> f32 {
        self.size.height as f32
    }
}

// RT 部分
impl WgpuState {
    fn create_default_rt(&mut self) {
        let current_size = uvec2(self.size.width, self.size.height);
        if let Some(rt) = self.render_targets.get_mut(self.default_render_target) {
            rt.rebuild_with_size_and_msaa(&self.context, current_size, self.msaa);
        } else {
            self.default_render_target = self.create_render_target(current_size);
        }
    }

    pub fn create_render_target(&mut self, size: UVec2) -> RenderTargetHandle {
        self.render_targets
            .insert(RenderTarget::new(&self.context, size, self.msaa))
    }

    pub(crate) fn get_active_render_target(&self) -> RenderTargetHandle {
        self.camera
            .as_ref()
            .and_then(|cam| cam.get_render_target())
            .unwrap_or(self.default_render_target)
    }
}

// Camera 部分
impl WgpuState {
    #[rustfmt::skip]
    fn pixel_perfect_projection_matrix(&self, size: UVec2) -> Mat4 {
        // 假设 size 是窗口的物理尺寸 (例如 1280, 720)
        let half_width = size.x as f32 / 2.0;
        let half_height = size.y as f32 / 2.0;

        let up_vec         = Vec3::Y;     // Y 轴向上为正
        let camera_pos     = Vec3::ZERO;  // 相机位于窗口中心，Z=0
        let look_direction = Vec3::NEG_Z; // 相机看向负 Z 轴 (即看向屏幕内部)

        let view = Mat4::look_at_rh(camera_pos, camera_pos + look_direction, up_vec);

        // 正交投影的边界
        // left/right/bottom/top 定义了视图体的范围
        // 以 (0,0) 为中心，X轴从 -half_width 到 half_width
        // Y轴从 -half_height 到 half_height (向上为正)
        let left   = -half_width;
        let right  =  half_width;
        let bottom = -half_height; // Y 轴负方向，屏幕下半部分
        let top    =  half_height; // Y 轴正方向，屏幕上半部分

        let near = -100.0; // 近裁剪平面，距离相机 -100.0 单位
        let far  =  100.0; // 远裁剪平面，距离相机  100.0 单位

        let proj = Mat4::orthographic_rh(
            left,
            right,
            bottom,
            top,
            near,
            far,
        );

        proj * view // 乘以 view 矩阵以创建最终的 ViewProjection 矩阵。
    }

    pub fn set_camera<C>(&mut self, new_camera: Option<C>)
    where
        C: Camera + Send + Sync + 'static,
    {
        self.draw();

        self.camera =
            new_camera.map(|cam| Box::new(cam) as Box<dyn Camera + Send + Sync + 'static>);
    }
}

// Material 部分
pub async fn create_material(
    name: String,
    shader_str: String,
    material_descriptor: MaterialDescriptor,
    uniform_defs: Option<HashMap<String, UniformDef>>,
) -> Option<MaterialHandle> {
    let ctx = get_quad_context();
    match Material::new(
        &ctx.context,
        &ctx.camera_bind_group_layout,
        ctx.msaa,
        name,
        shader_str,
        material_descriptor,
        uniform_defs,
    )
    .await
    {
        Ok(new_mat) => Some(ctx.materials.insert(new_mat)),
        Err(err) => {
            error!("material create error: {}", err);
            None
        }
    }
}

pub fn set_material(new_mat: MaterialHandle) {
    let ctx = get_quad_context();
    if let Some(current_mat_handle) = ctx.current_material {
        if current_mat_handle == new_mat {
            return;
        }
    }

    ctx.break_batching = true;
    ctx.current_material = Some(new_mat);
}

// Renderer
impl WgpuState {
    // 渲染逻辑 - 这个方法现在只负责呈现最终结果，不再进行实际绘制。
    // 它应该只处理默认渲染目标的解析和呈现。
    pub(crate) fn render(&mut self) -> Result<(), SurfaceError> {
        let context = &self.context;
        let output = context.surface.get_current_texture()?;

        if let Some(rt) = self.render_targets.get(self.default_render_target) {
            let mut encoder =
                context
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Final Render Encoder (Resolve & Present)"),
                    });

            if let Some(msaa_view) = &rt.msaa_texture_view {
                let _resolve_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("DefaultRT Msaa Resolve Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: msaa_view,
                        resolve_target: Some(&rt.resolve_texture_view),
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            }

            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &rt.resolve_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &output.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rt.size.into(),
            );

            context.queue.submit(std::iter::once(encoder.finish()));
        }

        // 呈现 SurfaceTexture
        output.present();
        Ok(())
    }

    pub(crate) fn clear_draw_calls(&mut self) {
        self.draw_calls.clear();
    }

    pub(crate) fn reset(&mut self) {
        self.clear_draw_calls();
    }

    pub(crate) fn prepare_for_new_frame(&mut self) {
        self.reset();
        self.clear_background(wgpu::Color::BLACK);
    }

    pub(crate) fn end_frame(&mut self, game_settings: &mut GameSettings) {
        // ... MSAA 更改处理 ...
        if let Some(new_msaa) = game_settings.new_msaa {
            if self.msaa == new_msaa {
                game_settings.new_msaa = None; // 已经相同，无需操作
                return;
            }

            self.msaa = new_msaa;
            game_settings.msaa = new_msaa; // 保存新的 MSAA 设置

            // 使用新的 MSAA 设置重新创建所有渲染目标
            self.render_targets.iter_mut().for_each(|(_, rt_ref)| {
                rt_ref.re_create(&self.context, self.msaa);
            });

            // 使用新的 MSAA 设置重建所有材质的管线
            self.materials.iter_mut().for_each(|(_, mat_ref)| {
                mat_ref.rebuild_pipeline(&self.context, &self.camera_bind_group_layout, self.msaa);
            });
        }

        game_settings.new_msaa = None;
    }

    pub fn clear_background(&mut self, color: wgpu::Color) {
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Clear Background Encoder"),
                });
        {
            // 获取渲染目标实例。
            let render_target = self
                .render_targets
                .get(self.get_active_render_target())
                .expect("RenderTarget not found for handle");

            // 确定用于渲染的视图和解析视图。
            let (view_to_render_to, resolve_target_view) =
                if render_target.msaa_texture_view.is_some() {
                    (
                        render_target.msaa_texture_view.as_ref().unwrap(),
                        Some(&render_target.resolve_texture_view),
                    )
                } else {
                    (&render_target.resolve_texture_view, None)
                };

            // 配置深度/模板附件
            let depth_stencil_attachment =
                render_target.depth_texture_view.as_ref().map(|depth_view| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0), // 清除深度到 1.0 (最远)
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None, // 如果需要，配置模板
                    }
                });

            // 创建 `wgpu::RenderPass`
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Active Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view_to_render_to,
                    resolve_target: resolve_target_view,
                    ops: wgpu::Operations {
                        load: PassAction::Clear(color).load_op(),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment, //depth_stencil_attachment_desc,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.context.queue.submit(std::iter::once(encoder.finish()));

        self.render_commands.clear();
    }

    pub(crate) fn draw(&mut self) {
        self.geometry();

        // 1. 全局数据上传（整帧一次）
        if !self.batch_vertex_buffer.is_empty() {
            self.global_vertex_buffer.ensure_size_and_copy(
                &self.context.device,
                &self.context.queue,
                bytemuck::cast_slice(&self.batch_vertex_buffer),
            );
        }
        if !self.batch_index_buffer.is_empty() {
            self.global_index_buffer.ensure_size_and_copy(
                &self.context.device,
                &self.context.queue,
                bytemuck::cast_slice(&self.batch_index_buffer),
            );
        }

        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Draw Encoder"),
                });

        // 状态追踪
        let mut cleared_targets = HashSet::new();
        let mut current_rt_handle = None;
        // 关键：将 RenderPass 放在 Option 中以延长生命周期并允许手动 Drop
        let mut render_pass: Option<wgpu::RenderPass> = None;

        for dc in &self.draw_calls {
            let rt_handle = dc.render_target;

            // --- 检查是否需要切换 RenderPass ---
            if current_rt_handle != Some(rt_handle) {
                // 1. 显式销毁旧的 Pass（释放对 encoder 的借用）
                render_pass = None;

                // 2. 准备新的 Pass 环境
                if let Some(render_target) = self.render_targets.get(rt_handle) {
                    let is_first_usage = cleared_targets.insert(rt_handle);

                    // 确定视图
                    let (view, resolve) = if render_target.msaa_texture_view.is_some() {
                        (
                            render_target.msaa_texture_view.as_ref().unwrap(),
                            Some(&render_target.resolve_texture_view),
                        )
                    } else {
                        (&render_target.resolve_texture_view, None)
                    };

                    // 确定深度负载
                    let depth_stencil_attachment =
                        render_target.depth_texture_view.as_ref().map(|depth_view| {
                            wgpu::RenderPassDepthStencilAttachment {
                                view: depth_view,
                                depth_ops: Some(wgpu::Operations {
                                    load: if is_first_usage {
                                        wgpu::LoadOp::Clear(1.0)
                                    } else {
                                        wgpu::LoadOp::Load
                                    },
                                    store: wgpu::StoreOp::Store,
                                }),
                                stencil_ops: None, // 如有特需可按同样逻辑配置
                            }
                        });

                    if depth_stencil_attachment.is_none() {
                        error!("RenderTarget DepthTexture Lost. ID: {}", rt_handle);
                        continue;
                    }

                    // 更新相机 (因为 RT 变了，投影矩阵可能需要变)
                    let rt_size = uvec2(render_target.size.width, render_target.size.height);
                    let proj = if let Some(camera) = self.camera.as_mut() {
                        camera.resize(rt_size);
                        camera.matrix()
                    } else {
                        self.pixel_perfect_projection_matrix(rt_size)
                    };
                    self.camera_uniform.update_matrix(proj);
                    self.context.queue.write_buffer(
                        &self.camera_buffer,
                        0,
                        bytemuck::cast_slice(&[self.camera_uniform]),
                    );

                    // 3. 开启新的 RenderPass
                    let mut new_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Batched Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view,
                            resolve_target: resolve,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment,
                        ..Default::default()
                    });

                    // 4. 初始化新 Pass 的全局绑定
                    new_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    new_pass.set_vertex_buffer(0, self.global_vertex_buffer.buffer.slice(..));
                    new_pass.set_index_buffer(
                        self.global_index_buffer.buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );

                    render_pass = Some(new_pass);
                    current_rt_handle = Some(rt_handle);
                }
            }

            // --- 执行绘制 ---
            if let (Some(pass), Some(mat)) =
                (render_pass.as_mut(), self.materials.get(dc.mat_handle))
            {
                pass.set_pipeline(&mat.pipeline);

                if mat.user_uniform_bind_group.is_some() {
                    // 每次切换材质时尝试更新和绑定
                    if let Ok(_) = mat.update_user_uniforms(&self.context) {
                        pass.set_bind_group(1, mat.user_uniform_bind_group.as_ref().unwrap(), &[]);
                    }
                }

                let index_start = dc.indices_start as u32;
                let index_end = (dc.indices_start + dc.indices_count) as u32;
                pass.draw_indexed(index_start..index_end, dc.vertices_start as i32, 0..1);
            }
        }

        // 释放最后一个 pass
        render_pass = None;

        self.context.queue.submit(std::iter::once(encoder.finish()));

        self.draw_calls.clear();
        self.batch_index_buffer.clear();
        self.batch_vertex_buffer.clear();
    }

    pub(crate) fn record_draw_command(
        &mut self,
        _vertices: &[Vertex],
        _indices: &[u32],
        z_order: u32,
    ) {
        let command_id = self.render_commands.len() as u32;
        let render_target = self.get_active_render_target();
        let mat_handle = self
            .current_material
            .unwrap_or(self.basic_shapes_triangle_mat);

        let depth = if mat_handle.is_depth_enabled() {
            let obj_world_center = calculate_object_center(_vertices);
            let (camera_position, camera_forward) = if let Some(cam) = self.camera.as_ref() {
                (cam.get_position(), cam.get_forward())
            } else {
                (Vec3::ZERO, Quat::IDENTITY * Vec3::NEG_Z)
            };

            // 从摄像机指向物体的向量
            let to_obj = obj_world_center - camera_position;

            // 使用点积 (Dot Product) 计算投影距离
            // 这就是物体在摄像机观察轴线上的 Z 深度
            to_obj.dot(camera_forward)
        } else {
            0f32
        };

        self.render_commands.push(RenderCommand {
            id: command_id,
            vertices: _vertices.to_vec(),
            indices: _indices.to_vec(),
            mat_handle,
            uniforms: None, // 示例
            render_target,
            render_queue: z_order,
            depth,
        });
    }

    pub(crate) fn geometry(&mut self) {
        self.sort_render_commands();

        if self.render_commands.is_empty() {
            return;
        }

        // 1. 初始化第一个 DrawCall，使用第一个命令的数据
        let first_cmd = &self.render_commands[0];

        // 同样对第一个命令的数据进行截断校准
        let v_limit = self.max_vertices.min(first_cmd.vertices.len());
        let i_limit = self.max_indices.min(first_cmd.indices.len());

        let mut current_draw_call = DrawCall {
            vertices_start: self.batch_vertex_buffer.len(), // 应该是当前 buffer 的末尾
            indices_start: self.batch_index_buffer.len(),
            vertices_count: v_limit,
            indices_count: i_limit,
            mat_handle: first_cmd.mat_handle,
            uniforms: first_cmd.uniforms.clone(),
            render_target: first_cmd.render_target,
        };

        // 将第一个命令的数据写入全局缓冲
        let vertex_offset = self.batch_vertex_buffer.len() as u32;
        self.batch_vertex_buffer
            .extend_from_slice(&first_cmd.vertices[0..v_limit]);
        for &idx in (&first_cmd.indices[0..i_limit]).iter() {
            self.batch_index_buffer.push(idx + vertex_offset);
        }

        // 2. 从第二个命令开始遍历 (skip 1)
        for cmd in self.render_commands.iter().skip(1) {
            let v_len = cmd.vertices.len().min(self.max_vertices);
            let i_len = cmd.indices.len().min(self.max_indices);

            let is_state_compatible = cmd.render_target == current_draw_call.render_target
                && cmd.mat_handle == current_draw_call.mat_handle
                && cmd.uniforms == current_draw_call.uniforms;

            let has_space = (current_draw_call.vertices_count + v_len <= self.max_vertices)
                && (current_draw_call.indices_count + i_len <= self.max_indices);

            if !is_state_compatible || !has_space {
                // 保存旧的，开启新的
                self.draw_calls.push(current_draw_call);

                current_draw_call = DrawCall {
                    vertices_start: self.batch_vertex_buffer.len(),
                    indices_start: self.batch_index_buffer.len(),
                    vertices_count: 0,
                    indices_count: 0,
                    mat_handle: cmd.mat_handle,
                    uniforms: cmd.uniforms.clone(),
                    render_target: cmd.render_target,
                };
            }

            // 写入数据
            let current_v_offset = self.batch_vertex_buffer.len() as u32;
            self.batch_vertex_buffer
                .extend_from_slice(&cmd.vertices[0..v_len]);
            for &idx in (&cmd.indices[0..i_len]).iter() {
                self.batch_index_buffer.push(idx + current_v_offset);
            }

            current_draw_call.vertices_count += v_len;
            current_draw_call.indices_count += i_len;
        }

        // 3. 压入最后一个 DrawCall
        self.draw_calls.push(current_draw_call);
        self.render_commands.clear();
    }

    pub fn sort_render_commands(&mut self) {
        self.render_commands.sort_by(|a, b| {
            // 1. 渲染目标 (Render Target)
            let target_cmp = a.render_target.cmp(&b.render_target);
            if target_cmp != std::cmp::Ordering::Equal {
                return target_cmp;
            }

            // 2. 渲染队列 (Render Queue)
            // 按照 render_queue 升序排序 (小的先渲染)
            let queue_cmp = a.render_queue.cmp(&b.render_queue);
            if queue_cmp != std::cmp::Ordering::Equal {
                return queue_cmp;
            }

            // --- 在相同的 Render Target 和 Render Queue 内部进行排序 ---

            // 3. 透明性判断和深度排序
            let a_is_transparent = a.mat_handle.should_render_as_transparent();
            let b_is_transparent = b.mat_handle.should_render_as_transparent();

            let depth_cmp = if a_is_transparent && b_is_transparent {
                // 如果两者都是透明：从远到近 (递减顺序)
                // b.depth - a.depth 得到负值是升序，正值是降序
                // 这里用 partial_cmp 确保浮点数比较的安全性
                b.depth
                    .partial_cmp(&a.depth)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else if !a_is_transparent && !b_is_transparent {
                // 如果两者都是不透明：从近到远 (递增顺序)
                a.depth
                    .partial_cmp(&b.depth)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                // 一个透明一个不透明：
                // 这种情况应该很少发生，因为通常会在不同的 render_queue 范围内。
                // 如果确实发生了，通常应该让不透明的先渲染。
                // 但是，如果 render_queue 设计得好，这个 else 分支几乎不会被调用
                // 因为透明和不透明物体会先被 render_queue 分开。
                // 如果它们在同一个 render_queue 比如 2500，且一个透明一个不透明，
                // 那你可能需要强制不透明先渲染。
                if a_is_transparent {
                    std::cmp::Ordering::Greater // a 是透明，b 不透明，a 后渲染
                } else {
                    std::cmp::Ordering::Less // b 是透明，a 不透明，b 后渲染
                }
            };

            if depth_cmp != std::cmp::Ordering::Equal {
                return depth_cmp;
            }

            // 4. 材质/Shader (Material Handle)
            // 避免频繁切换材质状态
            let mat_cmp = a.mat_handle.cmp(&b.mat_handle); // 假设 MaterialHandle 实现了 Ord
            if mat_cmp != std::cmp::Ordering::Equal {
                return mat_cmp;
            }

            // 5. 原始 ID 作为最终的决胜键 (提供稳定性)
            a.id.cmp(&b.id)
        });
    }
}

// 简易绘制部分
impl WgpuState {
    #[rustfmt::skip]
    pub fn draw_rectangle(
        &mut self,
        center_x: f32, // 矩形“逻辑”上的中心点或参考点
        center_y: f32,
        width: f32,
        height: f32,
        color: wgpu::Color,
        z_order: u32,
        pivot: glam::Vec2, // 新增参数：pivot，表示轴心点，范围通常是[0.0, 1.0]
    ) {
        // 首先计算矩形在没有考虑Pivot时的“理论”左下角和右上角
        // 这里的center_x, center_y将作为pivot点的实际坐标

        // 计算Pivot点相对于矩形宽高的偏移量
        let pivot_offset_x = width * pivot.x;
        let pivot_offset_y = height * pivot.y;

        // 根据“逻辑中心点”(center_x, center_y) 和 pivot 算出矩形左下角的真实坐标
        // 矩形左下角 = (逻辑中心x - (pivot.x * width)), (逻辑中心y - (pivot.y * height))
        let actual_bottom_left_x = center_x - pivot_offset_x;
        let actual_bottom_left_y = center_y - pivot_offset_y;

        // 然后根据实际的左下角和宽高，计算出所有顶点坐标
        let left   = actual_bottom_left_x;
        let right  = actual_bottom_left_x + width;
        let bottom = actual_bottom_left_y;
        let top    = actual_bottom_left_y + height;

        // 顶点定义 (沿用之前的约定：0=TL, 1=TR, 2=BR, 3=BL)
        let vertices = [
            Vertex::new(vec3(left , top   , 0.0), vec2(0.0, 0.0), color), // 0: Top-left
            Vertex::new(vec3(right, top   , 0.0), vec2(1.0, 0.0), color), // 1: Top-right
            Vertex::new(vec3(right, bottom, 0.0), vec2(1.0, 1.0), color), // 2: Bottom-right
            Vertex::new(vec3(left , bottom, 0.0), vec2(0.0, 1.0), color), // 3: Bottom-left
        ];

        // 三角形1: (3)BL -> (2)BR -> (0)TL  (逆时针)
        // 三角形2: (0)TL -> (2)BR -> (1)TR  (逆时针)
        let indices: [u32; 6] = [3, 2, 0, 0, 2, 1];

        self.record_draw_command(&vertices, &indices, z_order);
    }
}