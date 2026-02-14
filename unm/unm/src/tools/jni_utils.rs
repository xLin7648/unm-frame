use crate::ANDROID_APP;
use jni::{ JavaVM, objects::JObject };

pub fn get_refresh_rate() -> f32 {
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

pub fn call_game_ready() {
    let Some(app) = ANDROID_APP.get() else {
        return;
    };

    unsafe {
        let vm = JavaVM::from_raw(app.vm_as_ptr() as *mut _).expect("Failed to get JVM");
        let mut env = vm.attach_current_thread().expect("Failed to attach thread");
        let activity = JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject);

        env.call_method(&activity, "GameReady", "()V", &[]).ok();
    }
}