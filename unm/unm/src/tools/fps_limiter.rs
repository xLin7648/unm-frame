// https://github.com/aevyrie/bevy_framepace/blob/main/src/lib.rs
// MIT License

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::time::{Duration, Instant};
use log::info;
use winit::window::Window;

use crate::{game_settings::GameSettings, tools::TimeManager};

#[cfg(target_os = "android")]
pub fn get_refresh_rate() -> f32 {
    use crate::ANDROID_APP;
    // 修复：显式导入 JObject
    use jni::objects::JObject;
    use jni::JavaVM;

    let Some(app) = ANDROID_APP.get() else {
        return 120.0;
    };

    unsafe {
        let vm = JavaVM::from_raw(app.vm_as_ptr() as *mut _).expect("Failed to get JVM");
        let mut env = vm.attach_current_thread().expect("Failed to attach thread");
        let activity = JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject);

        // 修复：显式调用方法并处理返回值
        // 获取 Result 后，先得到 JValue，再转成 f32
        match env.call_method(&activity, "getRefreshRate", "()F", &[]) {
            Ok(val) => {
                // val 是 JValueGen 枚举，调用 f() 会尝试转为 f32
                // 为解决 type annotations needed，我们可以直接匹配或者显式指定
                val.f().unwrap_or(120.0)
            },
            Err(e) => {
                eprintln!("JNI error: {:?}", e);
                120.0
            }
        }
    }
}

/// 内部逻辑：根据平台选择获取刷新率的方式
fn get_platform_refresh_rate(window: &Window) -> f64 {
    #[cfg(target_os = "android")]
    {
        // 优先使用你写的 Android JNI 获取方式
        get_refresh_rate() as f64
    }

    #[cfg(not(target_os = "android"))]
    {
        // 其他平台（Windows, macOS, Linux）使用 winit 标准接口
        window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .map(|mhz| mhz as f32 / 1000.0)
            .unwrap_or(120.0) as f64 - 0.5
    }
}

pub fn detect_frametime(window: &Window) -> Duration {
    let refresh_rate = get_platform_refresh_rate(window);
    Duration::from_secs_f64(1.0 / refresh_rate)
}

#[allow(dead_code)]
pub fn framerate_limiter(
    window: &'static Window,
    timer: &mut TimeManager,
    game_settings: &GameSettings
) {
    let target_fps = game_settings.get_target_fps();
    let limit = if target_fps > 0 {
        Duration::from_secs_f64(1.0 / target_fps as f64)
    } else {
        detect_frametime(window)
    };

    let frame_time = timer.sleep_end.elapsed();
    let oversleep = timer.sleep_timer.oversleep;

    let sleep_time = limit.saturating_sub(frame_time + oversleep);
    spin_sleep::sleep(sleep_time);

    let frame_time_total = timer.sleep_end.elapsed();
    timer.sleep_end = Instant::now();

    timer.sleep_timer.frametime = frame_time;
    timer.sleep_timer.oversleep = frame_time_total.saturating_sub(limit);
}

#[allow(unused_variables)]
pub async fn framerate_limiter_tokio(
    window: &'static Window,
    timer: &mut TimeManager,
    game_settings: &GameSettings
) {
    let target_fps = game_settings.get_target_fps();
    let limit = if target_fps > 0 {
        Duration::from_secs_f64(1.0 / target_fps as f64)
    } else {
        detect_frametime(window)
    };

    let frame_time = timer.sleep_end.elapsed();
    let oversleep = timer.sleep_timer.oversleep;

    let sleep_time = limit.saturating_sub(frame_time + oversleep);
    tokio::time::sleep(sleep_time).await;

    let frame_time_total = timer.sleep_end.elapsed();
    timer.sleep_end = Instant::now();

    timer.sleep_timer.frametime = frame_time;
    timer.sleep_timer.oversleep = frame_time_total.saturating_sub(limit);
}