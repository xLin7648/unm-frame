use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Icon};

use crate::{app::WindowCommand, msaa::Msaa, resolution::Resolution};

pub struct GameSettings {
    event_loop: EventLoopProxy<WindowCommand>,
    target_fps: i32,
    background_run_mode: bool,
    pub(crate) current_window_size: PhysicalSize<u32>,
    pub(crate) msaa: Msaa,
    pub(crate) new_msaa: Option<Msaa>,
}

#[allow(dead_code)]
impl GameSettings {
    pub fn new(event_loop: EventLoopProxy<WindowCommand>) -> Self {
        Self { 
            target_fps: 0,
            event_loop: event_loop,
            background_run_mode: false,
            current_window_size: PhysicalSize::new(1, 1),
            msaa: Msaa::Sample4,
            new_msaa: Some(Msaa::Sample4)
        }
    }

    // setter
    pub fn set_title(&self, title: String) {
        self.event_loop.send_event(WindowCommand::SetTitle(title)).ok();
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.event_loop.send_event(WindowCommand::SetFullscreen(fullscreen)).ok();
    }

    pub fn set_resolution(&self, resolution: Resolution) {
        self.event_loop.send_event(WindowCommand::SetResolution(resolution)).ok();
    }

    pub fn set_window_icon(&self, icon: Icon) {
        self.event_loop.send_event(WindowCommand::SetWindowIcon(icon)).ok();
    }

    // <= 0: v-sync enable
    pub fn set_target_fps(&mut self, new_target_fps: i32) {
        self.target_fps = new_target_fps;
    }

    pub fn set_background_run_mode(&mut self, background_run_mode: bool) {
        self.background_run_mode = background_run_mode;
    }

    pub fn set_msaa(&mut self, msaa: Msaa) {
        self.new_msaa = Some(msaa);
    }

    // getter
    pub fn get_target_fps(&self) -> i32 {
        self.target_fps
    }

    pub fn get_background_run_mode(&self) -> bool {
        self.background_run_mode
    }

    pub fn get_window_size(&self) -> PhysicalSize<u32> {
        self.current_window_size
    }

    pub fn get_msaa(&self) -> Msaa {
        self.msaa
    }
}