#[cfg(target_os = "android")]
use std::sync::OnceLock;

use log::LevelFilter;

mod app;
mod graphics;
mod resolution;
mod game_loop;
mod game_settings;
mod msaa;
mod vertex;
mod camera;
mod tools;
mod my_game;
mod render_target;
mod material;
mod utils;
mod render_context;
mod uniform;
mod draw_call;
mod texture;
mod render_command;
mod input;

use crate::{ graphics::*, my_game::MyGame, render_context::RenderContext };

static mut CONTEXT: Option<WgpuState> = None;

pub(crate) fn get_quad_context() -> &'static mut WgpuState {
    unsafe { CONTEXT.as_mut().unwrap_or_else(|| panic!()) }
}

pub(crate) fn get_context() -> &'static mut RenderContext {
    unsafe {
        assert!(CONTEXT.is_some());
    }

    unsafe { &mut CONTEXT.as_mut().unwrap().context }
}

// ======================= Android specific =======================
#[cfg(target_os = "android")]
pub static ANDROID_APP: OnceLock<winit::platform::android::activity::AndroidApp> = OnceLock::new();

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: winit::platform::android::activity::AndroidApp) {
    let _ = ANDROID_APP.set(android_app);
    main();
}

// ================================================================

pub fn main() {
    app::App::new(MyGame::default())
        .set_logger_max_level(LevelFilter::Info)
        .run();
}