use async_trait::async_trait;
use unm_sfx::player::SfxManager;
use crate::{game_settings::GameSettings, graphics::WgpuState, input::{MouseInput, TouchInput}, tools::TimeManager};

#[async_trait]
pub trait GameLoop: Send {
    async fn start(
        &mut self,
        game_settings: &mut GameSettings,
        sfx_manager: &mut SfxManager
    );

    async fn update(
        &mut self,
        game_settings: &mut GameSettings,
        time_manager: &TimeManager,
        sfx_manager: &mut SfxManager,
        mouse_input: &MouseInput,
        touch_input: &TouchInput,
    );
}