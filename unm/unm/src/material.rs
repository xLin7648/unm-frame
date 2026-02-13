use log::error;
use unm_tools::id_map::IdMapKey;

use std::{collections::HashMap, num::NonZeroU64};

use wgpu::{
    BindGroupLayout, BindingType, BlendComponent, BlendFactor, BlendOperation, BlendState, BufferBindingType, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face, PipelineCompilationOptions, PipelineLayout, PolygonMode, PrimitiveTopology, RenderPipeline, ShaderModule, ShaderStages, StencilState, TextureFormat, naga::{self, Module, valid::ModuleInfo}
};

use crate::{get_quad_context, msaa::Msaa, render_context::RenderContext, texture::Texture2DHandle, uniform::*, vertex::Vertex};

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct MaterialHandle(u64);

impl IdMapKey for MaterialHandle {
    fn from(id: u64) -> Self {
        MaterialHandle(id)
    }
    fn to(&self) -> u64 {
        self.0
    }
}

impl MaterialHandle {
    pub fn is_depth_enabled(&self) -> bool {
        let ctx = get_quad_context();
        if let Some(mat) = ctx.materials.get_mut(*self) {
            mat.material_descriptor.is_depth_enabled()
        } else {
            false
        }
    }

    pub fn is_stencil_enabled(&self) -> bool {
        let ctx = get_quad_context();
        if let Some(mat) = ctx.materials.get_mut(*self) {
            mat.material_descriptor.is_stencil_enabled()
        } else {
            false
        }
    }

    pub fn should_render_as_transparent(&self) -> bool {
        let ctx = get_quad_context();
        if let Some(mat) = ctx.materials.get_mut(*self) {
            mat.material_descriptor.should_render_as_transparent()
        } else {
            // 根据handle未获取到材质
            // 视为错误
            false
        }
    }

    pub(crate) fn get_all_uniform(&self) -> Option<HashMap<String, Uniform>>
    {
        let ctx = get_quad_context();
        if let Some(mat) = ctx.materials.get_mut(*self) {
            Some(mat.current_uniform_values.clone())
        } else {
            None
        }
    }

    pub fn set_uniform<T>(&self, name: &str, value: T)
    where
        T: Into<Uniform>,
    {
        let ctx = get_quad_context();
        if let Some(mat) = ctx.materials.get_mut(*self) {
            ctx.break_batching = true;
            mat.set_uniform(name, value);
        }
    }

    pub fn set_texture<T>(&self, name: &str, texture: Texture2DHandle)
    {
        let ctx = get_quad_context();
        if let Some(mat) = ctx.materials.get_mut(*self) {
            ctx.break_batching = true;
            // mat.set_uniform(name, value);
        }
    }
}

// ====================================================================
// 修改 Material 结构体以存储 UBO 相关信息
// = ==================================================================
pub(crate) struct Material {
    pub(crate) name: String,
    pub(crate) pipeline: RenderPipeline,
    pub(crate) shader: ShaderModule, // 公开方便外部访问
    pub(crate) material_descriptor: MaterialDescriptor, // 公开方便外部访问
    pub(crate) uniform_defs: Option<HashMap<String, UniformDef>>, // Uniform 定义 (这个现在主要用于反射和初始化，可能不会直接在运行时使用)

    // *** 新增: 存储用户设置的 Uniform 值 ***
    pub(crate) current_uniform_values: HashMap<String, Uniform>,
    // pub(crate) current_texture_values: HashMap<String, Option<Texture2DHandle>>,

    // UBO 相关字段
    pub(crate) user_uniform_ubo: Option<wgpu::Buffer>, // 存储用户 Uniform 的 UBO 缓冲区
    pub(crate) uniform_layout: Option<UniformLayout>, // 存储每个 Uniform 的偏移量和大小
    pub(crate) user_uniform_bind_group: Option<wgpu::BindGroup>, // 存储用户 Uniform 的 BindGroup
    pub(crate) user_uniform_bind_group_layout: Option<wgpu::BindGroupLayout>, // 存储用户 Uniform 的 BindGroupLayout
    pub(crate) total_ubo_size: usize, // 整个 UBO 的总大小
}

impl Material {
    pub(crate) async fn new(
        context: &RenderContext,
        camera_bind_group_layout: &BindGroupLayout,
        sample_count: Msaa,
        name: String,
        shader_str: String,
        material_descriptor: MaterialDescriptor,
        uniform_defs: Option<HashMap<String, UniformDef>>, // 保持不变，用于初始化
    ) -> Result<Material, wgpu::Error> {
        let error_scope = context.device.push_error_scope(wgpu::ErrorFilter::Validation);

        let shader = context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{0} Shader", name)),
            source: wgpu::ShaderSource::Wgsl(shader_str.into()),
        });

        let mut current_uniform_values = HashMap::new(); // 初始化为空

        // 首次构建管线
        let (
            pipeline,
            user_uniform_ubo,
            uniform_layout,
            user_uniform_bind_group,
            user_uniform_bind_group_layout,
            total_ubo_size,
        ) = Self::create_render_pipeline(
            context,
            camera_bind_group_layout,
            sample_count,
            &name,
            &shader,
            &material_descriptor,
            &uniform_defs, // 仍然传递 uniform_defs 以便初始化 UBO
            &mut current_uniform_values, // 传递可变引用，`create_render_pipeline` 会用默认值填充它
        );

        if let Some(err) = error_scope.pop().await {
            Err(err)
        } else {
            Ok(Material {
                name,
                pipeline,
                shader,
                material_descriptor,
                uniform_defs, // 仍然存储 uniform_defs，以便 rebuild_pipeline 或未来其他用途
                current_uniform_values, // *** 存储初始化后的值 ***
                user_uniform_ubo,
                uniform_layout,
                user_uniform_bind_group,
                user_uniform_bind_group_layout,
                total_ubo_size,
            })
        }
    }

    // 辅助函数，用于根据给定的参数创建渲染管线
    // 返回值也需要修改以返回 UBO 相关信息
    fn create_render_pipeline(
        context: &RenderContext,
        camera_bind_group_layout_fixed: &BindGroupLayout, // 重命名，以示区分
        sample_count: Msaa,
        name: &str,
        shader: &wgpu::ShaderModule,
        material_descriptor: &MaterialDescriptor,
        uniform_defs: &Option<HashMap<String, UniformDef>>, // 用于获取默认值
        current_uniform_values: &mut HashMap<String, Uniform>, // 新增参数：用于填充 Material 自身的 current_uniform_values
    ) -> (
        wgpu::RenderPipeline,
        Option<wgpu::Buffer>,
        Option<UniformLayout>,
        Option<wgpu::BindGroup>,
        Option<wgpu::BindGroupLayout>,
        usize, // total_ubo_size
    ) {
        let mut user_uniform_ubo: Option<wgpu::Buffer> = None;
        let mut uniform_layout: Option<UniformLayout> = None;
        let mut user_uniform_bind_group: Option<wgpu::BindGroup> = None;
        let mut user_uniform_bind_group_layout: Option<wgpu::BindGroupLayout> = None;
        let mut total_ubo_size: usize = 0;

        let mut bind_group_layouts_for_pipeline = vec![camera_bind_group_layout_fixed];

        if let Some(uniform_defs_map) = uniform_defs {
            let (calculated_layout, calculated_total_size) =
                // 暂时使用 clone()，或者可以考虑让 calculate_uniform_offsets_and_total_size 接受引用
                calculate_uniform_offsets_and_total_size(uniform_defs_map);

            total_ubo_size = calculated_total_size;
            uniform_layout = Some(calculated_layout.clone()); // 克隆一份，因为下面要用

            if total_ubo_size > 0 { // 只有当有 Uniform 时才创建 UBO
                // 创建一个大的 UBO 缓冲区
                let ubo_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("{}_UserUniformUBO", name)),
                    size: total_ubo_size as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                // user_uniform_ubo = Some(ubo_buffer); // 先不存，稍后写入初始数据

                // 从 uniform_defs 中填充 initial_ubo_data 并添加到 current_uniform_values
                let mut initial_ubo_data = vec![0u8; total_ubo_size];
                for (uniform_name, (offset, size)) in calculated_layout.iter() {
                    if let Some(def_value) = uniform_defs_map.get(uniform_name) {
                        let uniform_variant_value = def_value.to_uniform_value();

                        // 填充 initial_ubo_data
                        let bytes = uniform_to_bytes(&uniform_variant_value);
                        if bytes.len() != *size {
                            error!("Warning: Default uniform '{}' byte length mismatch. Expected {}, got {}", uniform_name, size, bytes.len());
                        } else {
                            initial_ubo_data[*offset..*offset + *size].copy_from_slice(&bytes);
                            // 同时将默认值存入 Material 的 current_uniform_values
                            current_uniform_values.insert(uniform_name.clone(), uniform_variant_value);
                        }
                    }
                    // 如果 uniform_defs 没有这个 Uniform 的默认值，它将保持 initial_ubo_data 中的零。
                }

                // 将初始化的数据写入 UBO
                context.queue.write_buffer(&ubo_buffer, 0, &initial_ubo_data);
                user_uniform_ubo = Some(ubo_buffer); // 现在可以存储了

                // 创建用户自定义 Uniform 的 BindGroupLayout
                let created_user_layout = context.device.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor {
                        label: Some(&format!("{}_UserUniformLayout", name)),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: ShaderStages::VERTEX_FRAGMENT,
                                ty: BindingType::Buffer {
                                    ty: BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: Some(NonZeroU64::new(total_ubo_size as u64).expect("UBO size should not be zero")),
                                },
                                count: None,
                            },
                        ],
                    },
                );

                // 1. 将创建的 BindGroupLayout 的所有权赋给 Material 自身的字段 (或 Option)
                user_uniform_bind_group_layout = Some(created_user_layout);

                // 获取对已存储的 BindGroupLayout 的引用，用于创建 BindGroup
                let bind_group_layout_ref = user_uniform_bind_group_layout.as_ref().unwrap();

                // 创建用户自定义 Uniform 的 BindGroup
                let bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("{}_UserUniformBindGroup", name)),
                    layout: bind_group_layout_ref, // 使用引用
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0, // 修改为 binding 0，因为 Camera 绑定 0 也需要注意
                            resource: user_uniform_ubo.as_ref().unwrap().as_entire_binding(),
                        },
                    ],
                });
                user_uniform_bind_group = Some(bind_group);

                // 2. 为了将其添加到渲染管线的布局中，需要一个独立的 BindGroupLayout 实例的所有权，所以克隆
                bind_group_layouts_for_pipeline.push(bind_group_layout_ref);
            }
        } // end of if let Some(uniform_defs_map) = uniform_defs
        // 确保即使 uniform_defs 为 None，total_ubo_size 和 uniform_layout 也能被正确初始化（例如为None/0）

        let render_pipeline_layout = context
            .device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("{0} Pipeline Layout", name)),
                bind_group_layouts: &bind_group_layouts_for_pipeline, // 动态绑定布局
                ..Default::default()
            });

        let pipeline = context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{0} Pipeline", name)),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"), // 假设顶点着色器入口点是 vs_main
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"), // 假设片元着色器入口点是 fs_main
                targets: &[Some(wgpu::ColorTargetState {
                    format: context.config.format,
                    blend: Some(BlendState {
                        color: material_descriptor.color_blend,
                        alpha: material_descriptor.alpha_blend,
                    }),
                    write_mask: material_descriptor.color_write,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: material_descriptor.primitive_type.into(),
                polygon_mode: material_descriptor.primitive_type.into(),
                cull_mode: Some(material_descriptor.cull_mode),
                front_face: wgpu::FrontFace::Ccw,
                strip_index_format: None,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(material_descriptor.depth_stencil.clone()), // 假设没有深度或模板缓冲区
            multisample: wgpu::MultisampleState {
                count: sample_count.into(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            cache: None,
            multiview_mask: None,
        });

        (
            pipeline,
            user_uniform_ubo,
            uniform_layout,
            user_uniform_bind_group,
            user_uniform_bind_group_layout,
            total_ubo_size,
        )
    }

    /// 使用 Material 自身的数据重建渲染管线。
    ///
    /// 当 `wgpu::SurfaceConfiguration` 发生变化时，例如窗口大小改变，
    /// 通常需要重建管线。
    ///
    /// # 参数
    /// - `context`: WGPU 上下文。
    /// - `camera_bind_group_layout_fixed`: 传入的相机 BindGroupLayout。
    /// - `sample_count`: MSAA 采样数。
    pub(crate) fn rebuild_pipeline(
        &mut self,
        context: &RenderContext,
        camera_bind_group_layout_fixed: &BindGroupLayout, // 注意这里也是固定的相机布局
        sample_count: Msaa,
    ) {
        // 重建管线时，仍然需要当前的 uniform_values 来初始化 UBO，
        // 同时在创建过程中会再次用到 uniform_defs 来推断布局和默认值。
        let (
            pipeline,
            user_uniform_ubo,
            uniform_layout,
            user_uniform_bind_group,
            user_uniform_bind_group_layout,
            total_ubo_size,
        ) = Self::create_render_pipeline(
            context,
            camera_bind_group_layout_fixed,
            sample_count,
            &self.name,
            &self.shader,
            &self.material_descriptor,
            &self.uniform_defs,
            &mut self.current_uniform_values, // 传入自身可变引用
        );

        self.pipeline = pipeline;
        self.user_uniform_ubo = user_uniform_ubo;
        self.uniform_layout = uniform_layout;
        self.user_uniform_bind_group = user_uniform_bind_group;
        self.user_uniform_bind_group_layout = user_uniform_bind_group_layout;
        self.total_ubo_size = total_ubo_size;
    }

    // ====================================================================
    // 新增：设置 Uniform 值并准备更新 UBO 的方法
    // = ==================================================================
    /// 设置一个 Uniform 的值。
    /// 这个方法会更新 Material 内部存储的 `current_uniform_values`。
    ///
    /// 注意：调用此方法并不会立即将数据上传到 GPU。
    /// 你需要在渲染前调用 `update_user_uniforms` 来同步 GPU 缓冲区。
    pub(crate) fn set_uniform<T>(&mut self, name: &str, value: T)
    where
        T: Into<Uniform>, // 允许传入原始类型，如 f32，然后转换为 Uniform 枚举
    {
        // 检查 uniform_layout 以确保这个 uniform 是存在的
        if let Some(uniform_layout) = &self.uniform_layout {
            if !uniform_layout.contains_key(name) {
                error!("Uniform '{}' not found in material's shader.", name);
                return;
            }
        } else {
            // 如果 uniform_layout 都没有，说明这个 Material 不支持任何用户 Uniform
            error!("Material '{}' does not support user uniforms.", self.name);
            return;
        }

        self.current_uniform_values.insert(name.to_string(), value.into());
    }


    // ====================================================================
    // 新增：更新用户 Uniform 的方法
    // 这个方法现在将使用 Material 内部存储的 uniform 值
    // = ==================================================================
    pub(crate) fn update_user_uniforms(
        &self,
        context: &RenderContext,
    ) -> anyhow::Result<()> {
        if self.user_uniform_ubo.is_none() || self.uniform_layout.is_none() {
            // 没有用户 Uniform 或没有 UBO 创建
            return Ok(());
        }

        let ubo_buffer = self.user_uniform_ubo.as_ref().unwrap();
        let uniform_layout = self.uniform_layout.as_ref().unwrap();

        // 创建一个临时的 UBO 数据缓冲区，用于一次性写入
        let mut ubo_data = vec![0u8; self.total_ubo_size];

        for (uniform_name, (offset, size)) in uniform_layout.iter() {
            // 尝试从 Material 自身的 `current_uniform_values` 中获取值
            if let Some(value) = self.current_uniform_values.get(uniform_name) {
                // 将 Uniform 值转换为字节，并复制到 ubo_data
                let bytes = uniform_to_bytes(value);
                // 确保 bytes 的长度匹配预期的 size
                if bytes.len() != *size {
                    return Err(anyhow::anyhow!(
                        "Uniform '{}' byte length mismatch. Expected {}, got {}",
                        uniform_name, size, bytes.len()
                    ));
                }
                ubo_data[*offset..*offset + *size].copy_from_slice(&bytes);
            }
            // else 分支: 如果 `current_uniform_values` 中没有这个 Uniform 的值（这种情况理论上不应该发生，
            // 因为 initial_ubo_data 和 current_uniform_values 在 create_render_pipeline 时已被填充），
            // 那么它会保持 ubo_data 初始化时的零值。
        }

        // 一次性将整个 UBO 数据上传到 GPU
        context.queue.write_buffer(ubo_buffer, 0, &ubo_data);
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PrimitiveType {
    Triangles,
    Lines,
    Points,
}

impl From<PrimitiveType> for PrimitiveTopology {
    fn from(primitive_type: PrimitiveType) -> Self {
        match primitive_type {
            PrimitiveType::Triangles => PrimitiveTopology::TriangleList,
            PrimitiveType::Lines => PrimitiveTopology::LineList,
            PrimitiveType::Points => PrimitiveTopology::PointList,
        }
    }
}

impl From<PrimitiveType> for PolygonMode {
    fn from(primitive_type: PrimitiveType) -> Self {
        match primitive_type {
            PrimitiveType::Triangles => PolygonMode::Fill,
            PrimitiveType::Lines => PolygonMode::Fill,
            PrimitiveType::Points => PolygonMode::Point,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct MaterialDescriptor {
    pub color_blend: BlendComponent,
    pub alpha_blend: BlendComponent,
    pub color_write: ColorWrites,

    pub depth_stencil: DepthStencilState,

    pub primitive_type: PrimitiveType,
    pub cull_mode: Face,
}

impl Default for MaterialDescriptor {
    fn default() -> Self {
        Self {
            color_blend: BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha_blend: BlendComponent::OVER,
            color_write: ColorWrites::ALL,
            depth_stencil: DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            },
            primitive_type: PrimitiveType::Triangles,
            cull_mode: Face::Back
        }
    }
}

pub(crate) fn is_blending_active(blend_component: &BlendComponent) -> bool {
    !(blend_component.src_factor == BlendFactor::One &&
      blend_component.dst_factor == BlendFactor::Zero &&
      blend_component.operation == BlendOperation::Add)
}

impl MaterialDescriptor {
    pub fn is_depth_enabled(&self) -> bool {
        self.depth_stencil.is_depth_enabled()
    }

    pub fn is_stencil_enabled(&self) -> bool {
        self.depth_stencil.stencil.is_enabled()
    }

    pub fn should_render_as_transparent(&self) -> bool {
        let color_blending = is_blending_active(&self.color_blend);
        let alpha_blending = is_blending_active(&self.alpha_blend);

        color_blending || alpha_blending
    }

    pub fn triangle() -> Self {
        Self {
            primitive_type: PrimitiveType::Triangles,
            ..Default::default()
        }
    }

    pub fn lines() -> Self {
        Self {
            primitive_type: PrimitiveType::Lines,
            ..Default::default()
        }
    }

    pub fn points() -> Self {
        Self {
            primitive_type: PrimitiveType::Points,
            ..Default::default()
        }
    }
}