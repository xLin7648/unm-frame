use log::*;
use winit::event_loop::EventLoopBuilder;

use crate::app::WindowCommand;

// ======================= Logger Initialization =======================
pub fn init_logger(max_level: LevelFilter) {
    #[cfg(target_os = "macos")]
    {
        env_logger::builder()
            .filter_level(max_level)
            .parse_default_env()
            .init();
        info!("Logger initialized for macOS.");
    }

    #[cfg(target_os = "windows")]
    {
        env_logger::builder()
            .filter_level(max_level)
            .parse_default_env()
            .init();
        info!("Logger initialized for Windows.");
    }

    #[cfg(target_os = "android")]
    {
        use android_logger::Config;
        android_logger::init_once(Config::default().with_max_level(max_level));
        info!("Logger initialized for Android.");
    }
}

// ======================= EventLoop Builder Configuration =======================
pub fn configure_event_loop_builder(event_loop_builder: &mut EventLoopBuilder<WindowCommand>) {
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        event_loop_builder.with_any_thread(false);
        info!("EventLoopBuilder configured for Windows.");
    }

    #[cfg(target_os = "android")]
    {
        use std::sync::OnceLock;
        use winit::platform::android::EventLoopBuilderExtAndroid;
        use crate::ANDROID_APP;

        // Ensure ANDROID_APP is set before trying to get it
        if ANDROID_APP.get().is_none() {
            // This case should ideally not happen if android_main is called correctly by JNI
            error!("AndroidApp was not set before configuring EventLoopBuilder! This might indicate an issue with android_main.");
            // Handle error or panic, depending on desired behavior
        }
        event_loop_builder.with_android_app(ANDROID_APP.get().expect("AndroidApp not set").clone());
        info!("EventLoopBuilder configured for Android.");
    }
}