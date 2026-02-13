use std::{time::{Duration, Instant}};

#[derive(Clone)]
pub struct TimeManager {
    start_time: Instant,
    current_time: Duration,
    delta_time: Duration,
    fps: f32,  // 改为f32保持类型一致
    frame_times: [f32; 20],  // 帧时间环形缓冲区
    frame_index: usize,
    last_update: Instant,
    
    pub(crate) sleep_end: Instant,
    pub(crate) sleep_timer: SleepTimer,
}

#[derive(Default, Clone)]
pub(crate) struct SleepTimer {
    pub oversleep: Duration,
    pub frametime: Duration,
}

#[allow(dead_code)]
impl TimeManager {
    pub(crate) fn new() -> Self {
        let start_time = Instant::now();
        Self {
            start_time,
            current_time: Duration::ZERO,
            delta_time: Duration::ZERO,
            fps: 0.0,
            frame_times: [0.0; 20],
            frame_index: 0,
            last_update: start_time,
            sleep_end: Instant::now(),
            sleep_timer: SleepTimer::default(),
        }
    }

    pub(crate) fn update(&mut self) {
        let now = Instant::now();
        
        // 计算增量时间
        self.delta_time = now.duration_since(self.last_update);
        self.last_update = now;
        self.current_time = now.duration_since(self.start_time);
        
        // 更新帧时间缓冲区
        let delta_secs = self.delta_time.as_secs_f32();
        self.frame_times[self.frame_index] = delta_secs;
        self.frame_index = (self.frame_index + 1) % self.frame_times.len();
        
        // 计算平均FPS（基于最近N帧）
        let total_time: f32 = self.frame_times.iter().sum();
        self.fps = if total_time > 0.0 {
            self.frame_times.len() as f32 / total_time
        } else {
            0.0
        };
    }

    // 获取当前时间 (秒)
    pub fn get_time(&self) -> f32 {
        self.current_time.as_secs_f32()
    }

    // 获取增量时间 (秒)
    pub fn get_delta_time(&self) -> f32 {
        self.delta_time.as_secs_f32()
    }

    // 获取平均FPS
    pub fn get_fps(&self) -> u32 {
        self.fps.round() as u32
    }

    pub fn print_time_data(&self) {
        println!(
            "FPS: {}(avg) | DeltaTime: {:.6} | Time: {:.3}s",
            self.fps.round() as u32,
            self.delta_time.as_secs_f32(),
            self.current_time.as_secs_f32(),
        );
    }
}