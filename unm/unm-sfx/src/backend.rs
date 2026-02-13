#[cfg(any(target_os = "android"))]
pub mod oboe;

#[cfg(not(target_os = "android"))]
pub mod cpal;

use crate::clip::SfxHandle;

pub trait AudioBackend {
    // 构建流
    fn build_stream(&mut self) -> anyhow::Result<()>;

    // 检查流是否关闭/失效，如失效并且有音效则重建
    fn maintain_stream(&mut self);

    // 初始化音效
    fn init_load_sound(&mut self, datas: Vec<Vec<u8>>) -> Option<Vec<SfxHandle>>;

    // 尝试播放音效
    fn play(&mut self, handle: SfxHandle);
}