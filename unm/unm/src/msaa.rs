#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum Msaa {
    Off = 1,
    Sample2 = 2,
    #[default]
    Sample4 = 4,
    Sample8 = 8,
}

// 实现 From Trait，使其返回对应的 u32 值
impl From<Msaa> for u32 {
    fn from(msaa: Msaa) -> Self {
        msaa as u32
    }
}