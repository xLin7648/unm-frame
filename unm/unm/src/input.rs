// src/input.rs
use std::collections::HashMap; // 需要引入HashMap来存储多个Touch
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
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
}

/// 定义需要从主线程发送到渲染线程的鼠标和触控事件。
#[derive(Debug, Clone, Copy)]
pub enum InputEvent { // 将MouseEvent更名为InputEvent，包含更多类型
    /// 鼠标按钮被按下或释放
    MouseButton {
        button: MouseButton,
        state: MouseButtonState,
    },
    /// 触控事件 (类似 winit::event::Touch)
    Touch(winit::event::Touch)
    // 鼠标移动事件（可选，如果需要）
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

    /// 在处理本帧的 `WinitTouch` 事件之前调用。
    /// 这个方法负责：
    /// 1. 移除上一帧标记为 Ended 或 Cancelled 的触控点。
    /// 2. 更新其余触控点的 phase (若无新事件则变为 Stationary) 和 prev_x/prev_y。
    pub fn begin_frame(&mut self) {
        // 移除上一帧标记为 Ended 或 Cancelled 的触控点
        self.active_touches.retain(|_id, touch| {
            !(touch.phase == TouchPhase::Ended || touch.phase == TouchPhase::Cancelled)
        });

        // 遍历剩余的活跃触控点，更新它们的阶段和上一帧位置
        for touch in self.active_touches.values_mut() {
            // 将当前位置保存为上一帧位置，为计算delta做准备
            touch.prev_x = touch.x;
            touch.prev_y = touch.y;

            // 如果该触控点在上一帧是 Began 或 Moved，并且本帧没有新的 Moved 事件来更新它，
            // 那么它将被视为 Stationary。
            // Started 事件会设置为 Began，Moved 事件会设置为 Moved，因此这里处理的是那些没有新事件的
            if touch.phase == TouchPhase::Began || touch.phase == TouchPhase::Moved {
                touch.phase = TouchPhase::Stationary;
            }
            // 如果已经是 Stationary，则保持 Stationary
        }
    }

    /// 根据接收到的 `winit::event::Touch` 事件更新内部的触控状态。
    /// 这个方法会创建新的触控点，或更新现有触控点的信息和阶段。
    pub fn update_touch_event(&mut self, winit_touch: &winit::event::Touch) {
        let id = winit_touch.id;
        let x = winit_touch.location.x as f32;
        let y = winit_touch.location.y as f32;

        // 获取或创建触控点
        let touch_entry = self.active_touches.entry(id).or_insert_with(|| Touch {
            id,
            x,
            y, // 此时x,y是当前位置
            phase: TouchPhase::Began, // 会在下面根据winit_touch.phase更新
            prev_x: x, // 对新创建的触控点，prev_x/y与当前x/y相同
            prev_y: y,
        });

        // 更新触控点状态
        match winit_touch.phase {
            winit::event::TouchPhase::Started => {
                // 如果是Started，说明这是新触控
                touch_entry.phase = TouchPhase::Began;
                touch_entry.x = x; // 确保位置更新
                touch_entry.y = y;
                // prev_x, prev_y 保持为起始位置，在下一帧begin_frame会被覆盖
            }
            winit::event::TouchPhase::Moved => {
                // 更新现有触控点的位置和阶段
                if touch_entry.phase != TouchPhase::Began {
                    touch_entry.phase = TouchPhase::Moved;
                }
                touch_entry.x = x;
                touch_entry.y = y;
                // prev_x, prev_y 会在 begin_frame 中被更新
            }
            winit::event::TouchPhase::Ended => {
                // 标记为结束，这一帧内仍然可见，但在下一帧的 begin_frame 中会被移除
                touch_entry.phase = TouchPhase::Ended;
                touch_entry.x = x; // 确保结束位置是最新的
                touch_entry.y = y;
            }
            winit::event::TouchPhase::Cancelled => {
                // 标记为取消，这一帧内仍然可见，但在下一帧的 begin_frame 中会被移除
                touch_entry.phase = TouchPhase::Cancelled;
                touch_entry.x = x; // 确保取消位置是最新的
                touch_entry.y = y;
            }
            _ => {} // 忽略其他阶段（如 ForceChange）
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