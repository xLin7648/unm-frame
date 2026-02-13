use std::time::{SystemTime, UNIX_EPOCH};

use crate::get_quad_context;
use crate::input::{MouseInput, TouchInput, TouchPhase};
use async_trait::async_trait;
use glam::{uvec2, vec2, vec3, Vec3};
use log::info;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};
use unm_sfx::clip::SfxHandle;
use unm_sfx::player::SfxManager;
use winit::event::MouseButton;
// 这里非常重要！
use crate::{
    camera::{self, BaseCamera, Camera2D, Camera3D},
    game_loop::GameLoop,
    game_settings::GameSettings,
    graphics::WgpuState,
    material::{MaterialDescriptor, MaterialHandle},
    msaa::Msaa,
    render_target::RenderTargetHandle,
    resolution::Resolution,
    tools::TimeManager,
};

#[allow(dead_code)]
pub struct MyGame {
    m1: f32,
    m2: f32,

    handle: SfxHandle,
}

impl Default for MyGame {
    fn default() -> Self {
        Self {
            m1: 0.,
            m2: 0.,
            handle: SfxHandle::default(),
        }
    }
}

#[async_trait]
impl GameLoop for MyGame {
    async fn start(&mut self, game_settings: &mut GameSettings, sfx_manager: &mut SfxManager) {
        game_settings.set_msaa(Msaa::Off);
        game_settings.set_resolution(Resolution::Physical(1280, 720));
        // game_settings.set_target_fps(120);

        let cam = Camera2D::new(
            BaseCamera::new(vec3(0., 0., -100.0), -1000.0, 1000.0),
            uvec2(1280, 720),
        );
        get_quad_context().set_camera(Some(cam));

        // let file_path = "D:/HitSong0.wav";
        // println!("正在加载文件: {}", file_path);

        // let mut file = File::open(file_path).await.map_err(|e| {
        //     anyhow::anyhow!("无法打开文件 {}, 错误: {}", file_path, e)
        // }).unwrap();

        // let mut buffer = Vec::new();
        // file.read_to_end(&mut buffer).await;

        let buffer = include_bytes!("assets/HitSong0.wav");

        if let Some(handles) = sfx_manager.init_load_sound(vec![buffer.to_vec()]) {
            self.handle = handles[0];
        }
    }

    async fn update(
        &mut self,
        game_settings: &mut GameSettings,
        time_manager: &TimeManager,
        sfx_manager: &mut SfxManager,
        mouse_input: &MouseInput,
        touch_input: &TouchInput,
    ) {
        let render = get_quad_context();

        render.draw_rectangle(
            -50.0,
            0.0,
            100.0,
            100.0,
            wgpu::Color::RED,
            0,
            vec2(0.5, 0.5),
        );

        //render.clear_background(wgpu::Color::WHITE);

        render.draw_rectangle(
            0.0,
            0.0,
            100.0,
            100.0,
            wgpu::Color::GREEN,
            1,
            vec2(0.5, 0.5),
        );

        for touch in touch_input.get_touches() {
            // 只有当这根手指是刚按下（Began）的那一帧
            if touch.phase == TouchPhase::Began {
                // info!(
                //     "New finger detected! Total count: {}",
                //     touch_input.get_touch_count()
                // );
                match SystemTime::now().duration_since(UNIX_EPOCH) {
                    Ok(n) => {
                        let secs = n.as_secs();
                        let nanos = n.subsec_nanos();
                        println!("精确部分: {}s + {}ns", secs, nanos);
                    }
                    Err(_) => panic!("SystemTime before UNIX EPOCH!"),
                }
                sfx_manager.play(self.handle); // 每增加一根手指响一次
            }

            render.draw_rectangle(
                50.0,
                0.0,
                100.0,
                100.0,
                wgpu::Color::BLUE,
                0,
                vec2(0.5, 0.0),
            );
        }
    }
}
