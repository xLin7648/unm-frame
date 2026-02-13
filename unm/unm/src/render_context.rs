use anyhow::{Context, Ok};
use image::GenericImageView;
use log::info;
use wgpu::{Adapter, Backends, Device, Extent3d, Instance, InstanceDescriptor, Limits, Origin3d, Queue, RequestAdapterOptions, SamplerDescriptor, Surface, SurfaceConfiguration, TexelCopyTextureInfo, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor};
use winit::{dpi::PhysicalSize, window::Window};

use crate::texture::Texture2D;

pub(crate) struct RenderContext {
    pub(crate) instance: Instance,
    pub(crate) surface: Surface<'static>,
    pub(crate) adapter: Adapter,
    pub(crate) device: Device,
    pub(crate) queue: Queue,
    pub(crate) config: SurfaceConfiguration,
}

impl RenderContext {
    pub(crate) async fn new(
        window: &'static Window,
        size: PhysicalSize<u32>
    ) -> anyhow::Result<Self> {
        // 1. 创建 WGPU 实例
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });
        info!("WGPU Instance created.");

        // 2. 创建 Surface
        // create_surface 返回 Result<Surface, SurfaceError>
        let surface = instance
            .create_surface(window)
            .context("Failed to create WGPU surface from window")?; // 使用 .context() 添加上下文
        info!("WGPU Surface created.");

        // 3. 请求 Adapter
        // request_adapter 返回 Option<Adapter>
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find an appropriate WGPU adapter")?; // 使用 .context() 适用于 Option
        info!("WGPU Adapter requested: {:?}", adapter.get_info());

        // 4. 请求 Device 和 Queue
        // request_device 返回 Result<(Device, Queue), RequestDeviceError>
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Primary WGPU Device"),
                    memory_hints: wgpu::MemoryHints::default(),
                    required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                    // 注意：required_limits 可能需要与适配器的实际限制进行协商。
                    // 理想情况下，您应该检查这些限制是否得到支持，或者使用 Limits::default()。
                    // 如果您的应用程序特定需求，并且确定这些限制会被支持，可以保留。
                    required_limits: wgpu::Limits {
                        max_texture_dimension_2d: 4096,
                        ..Limits::downlevel_defaults()
                    },
                    ..Default::default()
                }
            )
            .await
            .context("Failed to create WGPU device and queue")?; // 使用 .context() 添加上下文

        info!("WGPU Device and Queue created.");

        // 5. 配置 Surface
        let surface_caps = surface.get_capabilities(&adapter);

        info!("present_modes: {:?}", surface_caps.present_modes);

        // 检查 formats 是否为空，避免 panic
        let formats = surface_caps.formats;
        let mut surface_format = *formats
            .first()
            .context("No supported surface formats found for WGPU surface")?; // 如果 formats 为空，这里会返回 Err

        // 遍历查找 sRGB 格式
        for available_format in formats {
            if available_format == TextureFormat::Rgba8UnormSrgb
                || available_format == TextureFormat::Bgra8UnormSrgb
            {
                surface_format = available_format;
                break;
            }
        }
        info!("Selected surface format: {:?}", surface_format);

        let alpha_mode = *surface_caps.alpha_modes
            .first()
            .context("No supported alpha modes found for surface")?;

        let view_formats = if !surface_format.is_srgb() {
            vec![surface_format.add_srgb_suffix()]
        } else {
            vec![]
        };

        // 确保 width 和 height 至少为 1，以防窗口大小为 0 导致 WGPU 错误
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            present_mode: wgpu::PresentMode::Mailbox,
            desired_maximum_frame_latency: 2,
            width: size.width.max(1),
            height: size.height.max(1),

            format: surface_format,
            view_formats,
            alpha_mode,
        };

        surface.configure(&device, &config);
        info!("WGPU Surface configured.");

        Ok(Self {
            instance,
            surface,
            adapter,
            device,
            queue,
            config,
        })
    }

    // 窗口大小改变时调用
    pub(crate) fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    // 辅助函数，负责将图像文件加载为 wgpu::Texture
    pub(crate) async fn load_texture(
        &mut self,
        file_path: &str,
        label: Option<&str>,
        address_mode: wgpu::AddressMode,
    ) -> anyhow::Result<Texture2D> {
        // 1. 异步加载图像文件 (使用 tokio::fs)
        // 如果你不是在tokio环境下运行 main 函数，或者不想异步加载，
        // 可以直接使用 std::fs::read 或 image::open
        let img_bytes = tokio::fs::read(file_path).await?;
        let img = image::load_from_memory(&img_bytes)?;

        // 2. 将图像数据转换为所需的 RGBA8 格式
        // 这里我们假设图像是RGBA8，如果不是，`to_rgba8()` 会进行转换
        // wgpu 通常希望纹理是预乘 alpha 的，但这里只是简单地读取。
        let rgba_image = img.to_rgba8();
        let dimensions = img.dimensions(); // 获取图像的宽度和高度

        // 3. 定义纹理大小
        let texture_size = Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1, // 对于2D纹理，深度或层数为1
        };

        // 4. 创建 wgpu 纹理
        let texture = self.device.create_texture(&TextureDescriptor {
            label,
            size: texture_size,
            mip_level_count: 1,                    // 暂不生成 mipmap
            sample_count: 1,                       // 不使用多重采样
            dimension: TextureDimension::D2,       // 2D 纹理
            format: TextureFormat::Rgba8UnormSrgb, // 统一使用 RGBA8U norm sRGB 格式
            // 纹理用途：用于复制目标（上传数据），采样器使用，渲染目标（如果需要渲染到它上面）
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 5. 上传图像数据到纹理
        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO, // 从纹理的 (0,0,0) 开始复制
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_image, // 图像的原始字节数据
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                // 像素行字节长度，必须是 WGPU_COPY_BYTES_PER_ROW_ALIGNMENT 的倍数 (256 字节)
                // `Some(width * 4)` 是指每行像素的字节数 (4个字节/像素 (RGBA8))
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size, // 复制整个纹理大小的数据
        );

        // 6. 创建 TextureView
        let texture_view = texture.create_view(&TextureViewDescriptor::default());

        // 7. 创建 Sampler
        let sampler = self.device.create_sampler(&SamplerDescriptor {
            label: Some("Texture Sampler"),
            // 纹理缩小过滤方式：线性插值
            mag_filter: wgpu::FilterMode::Linear,
            // 纹理放大过滤方式：线性插值
            min_filter: wgpu::FilterMode::Linear,
            // mipmap 采样方式：最近邻 (因为我们只有一个 mip level)
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            // 环绕模式：重复
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        Ok(Texture2D::new(texture, texture_view, sampler))
    }
}

pub async fn load_texture(
    file_path: &str,
    label: Option<&str>
) {

}