use winit::dpi::{LogicalSize, PhysicalSize, Size};

#[derive(Copy, Clone, Debug)]
pub enum Resolution {
    Physical(u32, u32),
    Logical(u32, u32),
}

impl Resolution {
    pub fn width(&self) -> u32 {
        match self {
            Self::Physical(w, _) => *w,
            Self::Logical(w, _) => *w,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            Self::Physical(_, h) => *h,
            Self::Logical(_, h) => *h,
        }
    }

    pub fn ensure_non_zero(&mut self) -> Resolution {
        const MIN_WINDOW_SIZE: u32 = 1;
        match self {
            Resolution::Physical(w, h) | Resolution::Logical(w, h)
                if *w == 0 || *h == 0 =>
            {
                *w = MIN_WINDOW_SIZE;
                *h = MIN_WINDOW_SIZE;
            }
            _ => (), // 如果大小已经有效，则不执行任何操作
        }

        *self // 返回修改后的 self
    }
}

impl From<Resolution> for Size {
    fn from(res: Resolution) -> Self {
        match res {
            Resolution::Physical(w, h) => Size::Physical(PhysicalSize::new(w, h)),
            Resolution::Logical(w, h) => Size::Logical(LogicalSize::new(w as f64, h as f64)),
        }
    }
}