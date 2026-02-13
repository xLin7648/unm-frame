use std::fmt::Display;

use glam::UVec2;
use unm_tools::id_map::IdMapKey;
use wgpu::{Extent3d, TextureDescriptor, TextureDimension, TextureUsages, TextureViewDescriptor, TextureFormat};

use crate::{msaa::Msaa, render_context::RenderContext};

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct RenderTargetHandle(u64);

impl Display for RenderTargetHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl IdMapKey for RenderTargetHandle {
    fn from(id: u64) -> Self {
        RenderTargetHandle(id)
    }
    fn to(&self) -> u64 {
        self.0
    }
}

#[allow(dead_code)]
pub(crate) struct RenderTarget {
    // Resolve 纹理 (单采样)
    pub(crate) resolve_texture: wgpu::Texture,
    pub(crate) resolve_texture_view: wgpu::TextureView,

    // MSAA 纹理
    pub(crate) msaa_texture: Option<wgpu::Texture>,
    pub(crate) msaa_texture_view: Option<wgpu::TextureView>,

    // 其他可能需要的成员，例如深度纹理（如果你的render pass需要深度测试）
    pub(crate) depth_texture: Option<wgpu::Texture>,
    pub(crate) depth_texture_view: Option<wgpu::TextureView>,

    pub(crate) size: Extent3d,
    pub(crate) format: TextureFormat,
}

impl RenderTarget {
    pub(crate) fn new(
        context: &RenderContext,
        size: UVec2,
        sample_count: Msaa,
    ) -> Self {
        let size_extent = Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        };
        let format = context.config.format;

        // 1. 创建 Resolve 纹理 (单采样) - 只在 new 的时候创建一次
        let resolve_texture_descriptor = TextureDescriptor {
            label: Some("Resolve Render Target Texture"),
            size: size_extent,
            mip_level_count: 1,
            sample_count: 1, // 关键：单采样
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let resolve_texture = context.device.create_texture(&resolve_texture_descriptor);
        let resolve_texture_view = resolve_texture.create_view(&TextureViewDescriptor::default());

        // 2. 创建 MSAA 和 Depth 纹理 (可能需要多采样)
        let (msaa_texture, msaa_texture_view, depth_texture, depth_texture_view) =
            Self::create_msaa_and_depth_textures(context, size_extent, format, sample_count);

        Self {
            resolve_texture,
            resolve_texture_view,
            msaa_texture,
            msaa_texture_view,
            depth_texture,
            depth_texture_view,
            size: size_extent,
            format,
        }
    }

    // 辅助函数：专门用于创建 MSAA 纹理和深度纹理
    fn create_msaa_and_depth_textures(
        context: &RenderContext,
        size: Extent3d,
        format: TextureFormat,
        sample_count: Msaa,
    ) -> (Option<wgpu::Texture>, Option<wgpu::TextureView>, Option<wgpu::Texture>, Option<wgpu::TextureView>) {
        let mut msaa_texture: Option<wgpu::Texture> = None;
        let mut msaa_texture_view: Option<wgpu::TextureView> = None;

        if sample_count != Msaa::Off {
            let msaa_texture_descriptor = TextureDescriptor {
                label: Some("MSAA Render Target Texture"),
                size,
                mip_level_count: 1,
                sample_count: sample_count.into(),
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
                view_formats: &[],
            };
            let d_texture = context.device.create_texture(&msaa_texture_descriptor);
            let d_texture_view = d_texture.create_view(&TextureViewDescriptor::default());

            msaa_texture = Some(d_texture);
            msaa_texture_view = Some(d_texture_view);
        }

        let depth_texture_descriptor = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: sample_count.into(),
            dimension: TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let d_texture = context.device.create_texture(&depth_texture_descriptor);
        let d_texture_view = d_texture.create_view(&wgpu::TextureViewDescriptor::default());

        (msaa_texture, msaa_texture_view, Some(d_texture), Some(d_texture_view))
    }

    /// 重建 RenderTarget 的纹理，通常是当 MSAA 设置改变时调用。
    /// resolut_texture 不会被重建，因为它总是单采样。
    pub(crate) fn re_create(
        &mut self,
        context: &RenderContext,
        new_msaa: Msaa,
    ) {
        let (new_msaa_texture, new_msaa_texture_view, new_depth_texture, new_depth_texture_view) =
        Self::create_msaa_and_depth_textures(context, self.size, self.format, new_msaa);

        // 替换字段
        self.msaa_texture = new_msaa_texture;
        self.msaa_texture_view = new_msaa_texture_view;
        self.depth_texture = new_depth_texture;
        self.depth_texture_view = new_depth_texture_view;
    }

    // 如果您也需要一个同时处理尺寸变化的 rebuild 方法，可以这样实现
    pub(crate) fn rebuild_with_size_and_msaa(
        &mut self,
        context: &RenderContext,
        new_size: UVec2,
        new_msaa: Msaa,
    ) {
        if self.size.width == new_size.x && self.size.height == new_size.y {
            return;
        }

        let new_size_extent = Extent3d {
            width: new_size.x,
            height: new_size.y,
            depth_or_array_layers: 1,
        };

        // 创建新的 resolve 纹理
        let new_resolve_texture_descriptor = TextureDescriptor {
            label: Some("Resolve Render Target Texture"),
            size: new_size_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        self.resolve_texture = context.device.create_texture(&new_resolve_texture_descriptor);
        self.resolve_texture_view = self.resolve_texture.create_view(&TextureViewDescriptor::default());

        // 创建新的 MSAA 和 Depth 纹理
        let (new_msaa_texture, new_msaa_texture_view, new_depth_texture, new_depth_texture_view) =
            Self::create_msaa_and_depth_textures(context, new_size_extent, self.format, new_msaa);

        self.msaa_texture = new_msaa_texture;
        self.msaa_texture_view = new_msaa_texture_view;
        self.depth_texture = new_depth_texture;
        self.depth_texture_view = new_depth_texture_view;
        self.size = new_size_extent;
    }
}
