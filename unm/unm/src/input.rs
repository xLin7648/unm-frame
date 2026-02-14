// src/input.rs
use std::collections::HashMap; // 需要引入HashMap来存储多个Touch
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use winit::event::MouseButton;

/// 定义鼠标按钮状态，用于表示某个按钮当前是否被按下。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButtonState {
    Pressed,
    Released,
}

/// 模仿Unity的TouchPhase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Began,      // 当手指第一次触碰屏幕时
    Moved,      // 当手指在屏幕上移动时
    Stationary, // 当手指在屏幕上停留但没有移动时（可选，winit通常不会直接提供）
    Ended,      // 当手指离开屏幕时
    Cancelled,  // 当系统取消触控时（例如，来电）
}

impl From<winit::event::TouchPhase> for TouchPhase {
    fn from(phase: winit::event::TouchPhase) -> Self {
        match phase {
            winit::event::TouchPhase::Started => TouchPhase::Began,
            winit::event::TouchPhase::Moved => TouchPhase::Moved,
            winit::event::TouchPhase::Ended => TouchPhase::Ended,
            winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
            // winit没有直接对应的Stationary，可以根据Moved事件前后位置判断，或者忽略
            _ => TouchPhase::Ended, // 默认处理，或者根据你的需求调整
        }
    }
}

/// 模仿Unity的Touch结构体
#[derive(Debug, Clone, Copy)]
pub struct Touch {
    pub id: u64, // 触控点的唯一标识符
    pub x: f32,
    pub y: f32,
    pub phase: TouchPhase,
    // Add more fields if needed, e.g., tapCount

    // 上一帧的位置，用于计算delta或判断Stationary
    pub prev_x: f32,
    pub prev_y: f32,
    // delta_x, delta_y 可以在 get_touch_delta_position 实时计算

    // 关键：如果这一帧 busy (是 Began)，先把后续状态存起来
    pub pending_phase: Option<TouchPhase>,
}

/// 定义需要从主线程发送到渲染线程的鼠标和触控事件。
#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    // 将MouseEvent更名为InputEvent，包含更多类型
    /// 鼠标按钮被按下或释放
    MouseButton {
        button: MouseButton,
        state: MouseButtonState,
    },
    /// 触控事件 (类似 winit::event::Touch)
    Touch(winit::event::Touch), // 鼠标移动事件（可选，如果需要）
                                // CursorMoved {
                                //     x: f64,
                                //     y: f64,
                                // },
}

/// 渲染线程中用于查询鼠标按键状态的结构体。
#[derive(Debug, Default)]
pub struct MouseInput {
    // ... 保持不变
    // 当前帧的鼠标按钮状态
    left_button_current: bool,
    right_button_current: bool,
    middle_button_current: bool,
    // ... 其他按钮

    // 上一帧的鼠标按钮状态
    left_button_previous: bool,
    right_button_previous: bool,
    middle_button_previous: bool,
    // ... 其他按钮
}

impl MouseInput {
    pub fn new() -> Self {
        MouseInput::default()
    }

    /// 在每一帧开始时调用，更新 `previous` 状态。
    /// 必须在处理新的 `InputEvent` 之前调用。
    pub fn begin_frame(&mut self) {
        self.left_button_previous = self.left_button_current;
        self.right_button_previous = self.right_button_current;
        self.middle_button_previous = self.middle_button_current;
    }

    /// 检查鼠标左键是否当前被按下 (类似 GetMouseButton)。
    pub fn get_mouse_button(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => self.left_button_current,
            MouseButton::Right => self.right_button_current,
            MouseButton::Middle => self.middle_button_current,
            _ => false, // 暂时不支持其他按钮
        }
    }

    /// 检查鼠标左键是否在当前帧被按下 (类似 GetMouseButtonDown)。
    pub fn get_mouse_button_down(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => self.left_button_current && !self.left_button_previous,
            MouseButton::Right => self.right_button_current && !self.right_button_previous,
            MouseButton::Middle => self.middle_button_current && !self.middle_button_previous,
            _ => false,
        }
    }

    /// 检查鼠标左键是否在当前帧被释放 (类似 GetMouseButtonUp)。
    pub fn get_mouse_button_up(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => !self.left_button_current && self.left_button_previous,
            MouseButton::Right => !self.right_button_current && self.right_button_previous,
            MouseButton::Middle => !self.middle_button_current && self.middle_button_previous,
            _ => false,
        }
    }

    /// 内部方法，根据接收到的事件更新鼠标状态。
    /// 这个方法只更新 `_current` 状态。
    pub fn update_button_state(&mut self, button: MouseButton, state: MouseButtonState) {
        match button {
            MouseButton::Left => self.left_button_current = state == MouseButtonState::Pressed,
            MouseButton::Right => self.right_button_current = state == MouseButtonState::Pressed,
            MouseButton::Middle => self.middle_button_current = state == MouseButtonState::Pressed,
            _ => {}
        }
    }
}

/// 渲染线程中用于查询触控事件的结构体。
#[derive(Debug, Default)]
pub struct TouchInput {
    // 存储所有当前活跃的触控点，key是touch id
    active_touches: HashMap<u64, Touch>,
}

impl TouchInput {
    pub fn new() -> Self {
        TouchInput::default()
    }

    pub fn begin_frame(&mut self) {
        // 1. 移除上一帧已经完成寿命的点
        self.active_touches.retain(|_id, touch| {
            !(touch.phase == TouchPhase::Ended || touch.phase == TouchPhase::Cancelled)
        });

        // 2. 状态平滑过渡
        for touch in self.active_touches.values_mut() {
            touch.prev_x = touch.x;
            touch.prev_y = touch.y;

            // 如果有挂起的（延迟的）状态，现在应用它
            if let Some(pending) = touch.pending_phase.take() {
                touch.phase = pending;
            } else {
                // 常规状态切换
                match touch.phase {
                    TouchPhase::Began => touch.phase = TouchPhase::Stationary,
                    TouchPhase::Moved => touch.phase = TouchPhase::Stationary,
                    _ => {}
                }
            }
        }
    }

    pub fn update_touch_event(&mut self, winit_touch: &winit::event::Touch) {
        let id = winit_touch.id;
        let x = winit_touch.location.x as f32;
        let y = winit_touch.location.y as f32;
        let phase = winit_touch.phase;

        if !self.active_touches.contains_key(&id) {
            self.active_touches.insert(
                id,
                Touch {
                    id,
                    x,
                    y,
                    phase: TouchPhase::Began, // 初始必为 Began
                    prev_x: x,
                    prev_y: y,
                    pending_phase: None,
                },
            );
            return;
        }

        if let Some(touch) = self.active_touches.get_mut(&id) {
            // 更新最新坐标
            touch.x = x;
            touch.y = y;

            let new_phase = match phase {
                winit::event::TouchPhase::Started => TouchPhase::Began,
                winit::event::TouchPhase::Moved => TouchPhase::Moved,
                winit::event::TouchPhase::Ended => TouchPhase::Ended,
                winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
            };

            // 如果当前已经是 Began，不要覆盖它，把新状态存入 pending
            if touch.phase == TouchPhase::Began {
                // 如果 Began 后面跟着 Ended，确保 pending 记录的是 Ended
                if new_phase != TouchPhase::Began {
                    touch.pending_phase = Some(new_phase);
                }
            } else {
                // 如果不是 Began 帧，正常更新状态
                touch.phase = new_phase;
            }
        }
    }

    /// 获取当前所有活跃的触控点。类似于Unity的 Input.touches。
    pub fn get_touches(&self) -> Vec<&Touch> {
        self.active_touches.values().collect() // 返回所有活跃触控点的引用
    }
    // 注意：这里返回的是 Vec<&Touch>，如果需要拥有所有权，则改为 Vec<Touch> 和 .cloned()

    /// 根据ID获取某个特定的触控点。
    pub fn get_touch_by_id(&self, id: u64) -> Option<&Touch> {
        self.active_touches.get(&id)
    }

    /// 获取当前帧的触控点数量。
    pub fn get_touch_count(&self) -> usize {
        self.active_touches.len()
    }

    // 辅助方法：获取 delta_position （类似于 Unity）
    pub fn get_touch_delta_position(&self, touch_id: u64) -> Option<(f32, f32)> {
        if let Some(touch) = self.active_touches.get(&touch_id) {
            Some((touch.x - touch.prev_x, touch.y - touch.prev_y))
        } else {
            None
        }
    }
}
