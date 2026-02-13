use crate::{atlas::SoundAtlas, backend::AudioBackend, clip::{ClipMap, SfxHandle}, mixer::Mixer};

pub(crate) static mut GLOBAL_MIXER: Option<Mixer> = None;
pub(crate) static mut GLOBAL_ATLAS: Option<(SoundAtlas, std::collections::HashMap<SfxHandle, ClipMap>)> = None;

pub struct SfxManager(Box<dyn AudioBackend>);

unsafe impl Send for SfxManager {}
unsafe impl Sync for SfxManager {}

impl SfxManager {
    pub fn new() -> Self {
        #[cfg(target_os = "android")]
        let backend = Box::new(crate::backend::oboe::Player::new());
        #[cfg(not(target_os = "android"))]
        let backend = Box::new(crate::backend::cpal::Player::new());

        Self(backend)
    }

    pub fn maintain_stream(&mut self) {
        self.0.maintain_stream()
    }

    pub fn init_load_sound(&mut self, datas: Vec<Vec<u8>>) -> Option<Vec<SfxHandle>> {
        self.0.init_load_sound(datas)
    }

    pub fn play(&mut self, handle: SfxHandle) {
        self.0.play(handle);
    }
}