use unm_tools::id_map::IdMapKey;

#[derive(Default, Eq, PartialEq, Clone, Copy, Hash, Debug)]
pub struct SfxHandle(pub u64);

unsafe impl Send for SfxHandle {}
unsafe impl Sync for SfxHandle {}

impl IdMapKey for SfxHandle {
    fn from(id: u64) -> Self { SfxHandle(id) }
    fn to(&self) -> u64 { self.0 }
}

#[derive(Clone, Copy)]
pub(crate) struct ClipMap {
    pub data_ptr: *const f32,
    pub frames_count: usize,
}

unsafe impl Send for ClipMap {}
unsafe impl Sync for ClipMap {}