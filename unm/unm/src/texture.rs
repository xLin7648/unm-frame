use log::error;
use unm_tools::id_map::IdMapKey;
use wgpu::{Sampler, Texture, TextureView};

use crate::{get_context, get_quad_context};

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub struct Texture2DHandle(u64);

impl IdMapKey for Texture2DHandle {
    fn from(id: u64) -> Self {
        Texture2DHandle(id)
    }
    fn to(&self) -> u64 {
        self.0
    }
}

pub struct Texture2D {
    texture: Texture,
    texture_view: TextureView,
    sampler: Sampler,
}

impl Texture2D {
    pub(crate) fn new(texture: Texture, texture_view: TextureView, sampler: Sampler) -> Self {
        Self {
            texture,
            texture_view,
            sampler,
        }
    }
}

pub(crate) async fn load_texture(
    file_path: &str,
    label: Option<&str>,
    address_mode: wgpu::AddressMode,
) -> Option<Texture2DHandle> {
    let ctx = get_quad_context();
    match ctx
        .context
        .load_texture(file_path, label, address_mode)
        .await
    {
        Ok(new_texture2d) => Some(ctx.texture2ds.insert(new_texture2d)),
        Err(err) => {
            error!("texture load error: {}", err);
            None
        }
    }
}
