use crossbeam_queue::ArrayQueue;
use log::*;
use unm_sfx::player::SfxManager;
use std::{
    mem::ManuallyDrop,
    sync::{Arc, mpsc::{self, Sender}},
    time::Duration,
};
use tokio::{runtime::Runtime, task::JoinHandle, time::sleep};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Fullscreen, Icon, Window, WindowAttributes, WindowId},
};

use crate::{
    CONTEXT, WgpuState, game_loop::GameLoop, game_settings::GameSettings, get_quad_context, input::{InputEvent, MouseButtonState, MouseInput, TouchInput}, resolution::Resolution, tools::*
};

/// 渲染线程可以发送给主线程的命令，用于控制窗口行为。
#[derive(Debug)]
pub enum WindowCommand {
    /// 设置窗口全屏模式。`None` 表示退出全屏，`Some(Fullscreen)` 表示设置特定全屏模式。
    SetFullscreen(bool),
    /// 设置窗口图标。
    SetWindowIcon(Icon),
    /// 设置窗口标题。
    SetTitle(String),
    /// 请求重新设置窗口分辨率。这会触发 `WindowEvent::Resized`。
    SetResolution(Resolution),
    // 还可以添加更多命令，例如 SetCursorIcon, SetDecorations 等。

    Quit
}

/// 渲染线程可以接收的命令。
enum WgpuStateCommand {
    /// 调整渲染器大小。
    Resize(PhysicalSize<u32>),
    /// 关闭渲染线程。
    Close,
}

/// 应用程序的主结构，管理 winit 窗口、WGPU 状态和渲染线程。
pub struct App {
    /// 对窗口的静态引用。使用 `ManuallyDrop` 管理生命周期。
    window_ref: Option<&'static Window>,
    /// 包含窗口的 `Box`，用于在 `Drop` 时安全地回收内存。
    window_box: Option<ManuallyDrop<Box<Window>>>,

    /// 用于向渲染线程发送命令的发送者。
    render_command_sender: Option<Sender<WgpuStateCommand>>,
    /// 从渲染线程向 winit 事件循环发送 `UserEvent` 的代理。
    event_proxy: EventLoopProxy<WindowCommand>,

    /// 渲染线程的 Tokio `JoinHandle`。
    render_thread_handle: Option<JoinHandle<()>>,

    /// 应用程序的 Tokio 运行时。
    runtime: Option<Runtime>,

    event_loop: Option<EventLoop<WindowCommand>>,

    max_level: LevelFilter,

    /// 游戏的实例
    game: Option<Box<dyn GameLoop>>,

    /// 用于从主线程向渲染线程发送鼠标事件的队列。
    mouse_event_sender: Arc<ArrayQueue<InputEvent>>, // 添加鼠标事件发送队列
}

impl App {
    pub fn new(game: impl GameLoop + 'static) -> Self {
        let mut event_loop_builder = EventLoop::<WindowCommand>::with_user_event();
        platform_specific::configure_event_loop_builder(&mut event_loop_builder);

        let event_loop = event_loop_builder
            .build()
            .expect("Failed to build event loop");

        let event_loop_proxy: EventLoopProxy<WindowCommand> = event_loop.create_proxy();
        event_loop.set_control_flow(ControlFlow::Poll);

        Self {
            window_ref: None,
            window_box: None,
            render_command_sender: None,
            event_proxy: event_loop_proxy,
            render_thread_handle: None,
            runtime: None,

            event_loop: Some(event_loop),
            max_level: LevelFilter::Info,

            game: Some(Box::new(game)),

            mouse_event_sender: Arc::new(ArrayQueue::new(128)), // 初始化队列，大小可调整
        }
    }

    pub fn set_logger_max_level(mut self, max_level: LevelFilter) -> Self {
        self.max_level = max_level;
        self
    }

    pub fn run(&mut self) {
        platform_specific::init_logger(self.max_level);
        if let Some(event_loop) = self.event_loop.take() {
            let _ = event_loop.run_app(self);
        }
    }

    /// 初始化窗口和 WGPU 状态，并启动渲染线程。
    /// 第一次 `resumed` 回调时调用。
    fn initialize_app_components(&mut self, event_loop: &ActiveEventLoop) {
        if self.render_command_sender.is_some() {
            info!("Window and WGPU already initialized.");
            return;
        }

        info!("Initializing window and WGPU state...");

        match event_loop.create_window(WindowAttributes::default()) {
            Ok(window) => match self.setup_window_and_render_thread(window) {
                Err(e) => {
                    error!("Failed to create render: {:?}", e);
                    event_loop.exit();
                }
                _ => {}
            },
            Err(e) => {
                error!("Failed to create window: {:?}", e);
                event_loop.exit();
            }
        }
    }

    /// 设置窗口引用、WGPU 状态和渲染线程。
    fn setup_window_and_render_thread(&mut self, window: Window) -> anyhow::Result<()> {
        // 将 Box<Window> 泄漏，并获取其 &'static 引用
        let window_box = Box::new(window);
        let window_ref: &'static Window = Box::leak(window_box);

        // 存储 window_ref 和 ManuallyDrop 的 window_box 用于在 Drop 时清理
        self.window_ref = Some(window_ref);
        self.window_box = Some(ManuallyDrop::new(unsafe {
            Box::from_raw(window_ref as *const _ as *mut _)
        }));

        let wgpu_state_initial = pollster::block_on(WgpuState::new(window_ref))?;
        unsafe { CONTEXT = Some(wgpu_state_initial) };

        // 创建渲染命令频道
        let (render_command_sender, render_command_receiver) = mpsc::channel();
        self.render_command_sender = Some(render_command_sender);

        let mouse_event_queue = Arc::clone(&self.mouse_event_sender);

        // 初始化 Tokio 运行时（如果尚未初始化）
        self.runtime = Some(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2) // 根据需要调整线程数
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime"),
        );
        let runtime_handle = self.runtime.as_ref().unwrap().handle().clone();

        let event_proxy = self.event_proxy.clone();
        let game = self
            .game
            .take()
            .expect("Game loop instance should be present when starting render thread"); // 获取 game 实例

        // 在 Tokio 运行时上启动渲染任务
        let render_thread_handle = runtime_handle.spawn(async move {
            Self::render_loop(
                render_command_receiver,
                event_proxy.clone(),
                mouse_event_queue, // 传递鼠标事件队列
                window_ref, // 传递 &'static Window
                game,       // 传递游戏实例
            ).await;
        });
        self.render_thread_handle = Some(render_thread_handle);
        Ok(())
    }

    /// 渲染线程的主循环逻辑。
    async fn render_loop(
        wgpu_state_receiver: mpsc::Receiver<WgpuStateCommand>,
        event_proxy: EventLoopProxy<WindowCommand>,
        input_event_receiver: Arc<ArrayQueue<InputEvent>>, // 接收鼠标事件队列
        window_ref: &'static Window,
        mut game: Box<dyn GameLoop>,
    ) {
        let mut sfx_manager = SfxManager::new();
        let mut mouse_input = MouseInput::new();
        let mut touch_input = TouchInput::new();

        let wgpu_state = get_quad_context();
        wgpu_state.create_default_resources().await;

        let mut game_settings = GameSettings::new(event_proxy);
        game.start(&mut game_settings, &mut sfx_manager).await;

        wgpu_state.end_frame(&mut game_settings);

        // 移动端优化：当窗口过小时降低渲染频率
        let sleep_rate_limit: Duration = Duration::from_secs(1);
        let mut time_manager = TimeManager::new();

        loop {
            let mut new_size: Option<PhysicalSize<u32>> = None;
            while let Ok(command) = wgpu_state_receiver.try_recv() {
                match command {
                    WgpuStateCommand::Resize(size) => {
                        new_size = Some(size);
                        game_settings.current_window_size = size;
                    }
                    WgpuStateCommand::Close => {
                        info!("Render thread received close command. Exiting render loop.");
                        return;
                    }
                }
            }

            mouse_input.begin_frame();
            touch_input.begin_frame();

            // 处理鼠标事件队列
            while let Some(event) = input_event_receiver.pop() {
                match event {
                    InputEvent::MouseButton { button, state } => {
                        mouse_input.update_button_state(button, state);
                    }
                    InputEvent::Touch(touch) => {
                        touch_input.update_touch_event(&touch);
                    },
                }
            }

            let current_window_size = game_settings.get_window_size();

            // 如果处于后台运行模式且窗口过小，则暂停渲染
            if !game_settings.get_background_run_mode()
                && (current_window_size.width <= 1 || current_window_size.height <= 1)
            {
                sleep(sleep_rate_limit).await;
                tokio::task::yield_now().await;
                continue;
            }

            if let Some(new_size) = new_size {
                wgpu_state.resize(new_size);
                window_ref.request_redraw();
            }

            // 更新时间管理器并打印时间数据
            time_manager.update();
            // time_manager.print_time_data();

            // 渲染前操作
            wgpu_state.prepare_for_new_frame();

            {
                // 游戏逻辑
                game.update(&mut game_settings, &time_manager, &mut sfx_manager, &mouse_input, &touch_input).await;
            }

            wgpu_state.draw();
            // 执行 WGPU 渲染
            match wgpu_state.render() {
                Ok(_) => {}
                Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => { // 添加 Outdated 处理
                    // Surface 丢失或过时，通常需要重新配置。
                    // 虽然你在 loop 开头已经 resize 了，但这里再次触发 resize 也是安全的，或者不仅由于大小改变，
                    // 某些驱动行为也要求重新 configure。
                    if wgpu_state.size.width > 0 && wgpu_state.size.height > 0 {
                        wgpu_state.resize(wgpu_state.size);
                    }
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    error!("Render thread: Out of GPU memory...");
                    panic!("Out of GPU memory");
                }
                Err(e) => {
                    warn!("Render error: {:?}", e); // 打印其他错误，看看是否有 timeouts
                }
            }
            wgpu_state.end_frame(&mut game_settings);
            sfx_manager.maintain_stream();

            //tokio::task::yield_now().await; // 仅让出时间片，不长时间休眠
            //进行帧率限制
            if new_size.is_some() {
                tokio::task::yield_now().await; // 仅让出时间片，不长时间休眠
            } else {
                framerate_limiter(window_ref, &mut time_manager, &game_settings);//.await;
            }
        }
    }
}

/// [`App`] 的 `Drop` 实现，负责清理资源。
impl Drop for App {
    fn drop(&mut self) {
        info!("Dropping App: Sending close command to render task.");
        if let Some(sender) = self.render_command_sender.take() {
            // 在退出前告诉渲染线程关闭
            let _ = sender.send(WgpuStateCommand::Close);
        }

        // 等待渲染线程结束（如果它还在运行）
        if let Some(_) = self.render_thread_handle.take() {
            // 如果渲染线程是 Tokio 任务，这里无法同步等待其完成，
            // 除非使用 `block_on` (不推荐在 drop 中使用) 或在 `shutdown_background` 中处理。
            // `shutdown_background` 会尝试优雅关闭所有任务。
        }

        // 停止 Tokio 运行时。这将尝试优雅地关闭所有在运行时上创建的异步任务。
        if let Some(runtime) = self.runtime.take() {
            info!("Dropping App: Shutting down Tokio runtime.");
            runtime.shutdown_background();
        }

        // 回收 Box::leak 的内存
        if let Some(mut boxed_window) = self.window_box.take() {
            unsafe {
                ManuallyDrop::drop(&mut boxed_window);
            }
        }
        println!("Application about to close, cleaning up resources.");
    }
}

/// [`ApplicationHandler`] 的实现，处理 winit 事件。
impl ApplicationHandler<WindowCommand> for App {
    /// 处理自定义用户事件。这些事件从其他线程发送到 winit 事件循环。
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: WindowCommand) {
        let window = self
            .window_ref
            .expect("Window should be initialized before processing user events");

        match event {
            WindowCommand::SetFullscreen(fullscreen) => {
                let mode = if fullscreen {
                    Some(Fullscreen::Borderless(None))
                } else {
                    None
                };

                window.set_fullscreen(mode);
            }
            WindowCommand::SetWindowIcon(icon) => {
                window.set_window_icon(Some(icon));
            }
            WindowCommand::SetTitle(title) => {
                window.set_title(&title);
            }
            WindowCommand::SetResolution(mut new_size) => {
                let _ = window.request_inner_size(new_size.ensure_non_zero());
            }
            WindowCommand::Quit => {
                _event_loop.exit();
            }
        }
    }

    /// 当应用程序从暂停状态恢复时调用。
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.render_command_sender.is_none() {
            info!("Application resumed, initializing window and WGPU...");
            self.initialize_app_components(event_loop);
        } else {
            info!("Application resumed. Window and WGPU already initialized.");
        }
    }

    /// 处理窗口事件。
    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = self
            .window_ref
            .expect("Window should be initialized before processing window events");
        let sender = self
            .render_command_sender
            .as_ref()
            .expect("Render command sender should be initialized for window events");

        let input_event_sender = self.mouse_event_sender.as_ref();
        if window_id != window.id() {
            return;
        }

        match event {
            WindowEvent::Resized(new_size) => {
                let width = new_size.width.max(1);
                let height = new_size.height.max(1);
                // 向渲染线程发送调整大小命令
                let _ = sender.send(WgpuStateCommand::Resize(PhysicalSize::new(width, height)));
            }
            WindowEvent::CloseRequested => {
                info!("Window close requested. Exiting application.");
                // 通知渲染线程关闭
                let _ = sender.send(WgpuStateCommand::Close);
                _event_loop.exit();
            }
            WindowEvent::MouseInput {
                state,
                button,
                ..
            } => {
                let button_state = match state {
                    winit::event::ElementState::Pressed => MouseButtonState::Pressed,
                    winit::event::ElementState::Released => MouseButtonState::Released,
                };
                // 将鼠标事件发送给渲染线程
                if let Err(e) = input_event_sender.push(InputEvent::MouseButton { button, state: button_state }) {
                    warn!("Failed to send mouse event to render thread: {:?}", e);
                }
            }
            WindowEvent::Touch(touch) => {
                let button_state = match touch.phase {
                    winit::event::TouchPhase::Started => MouseButtonState::Pressed,
                    winit::event::TouchPhase::Ended | winit::event::TouchPhase::Cancelled => MouseButtonState::Released,
                    _ => return, // 对于Moved或其他阶段，如果我们只关心按下/抬起，则直接返回
                };

                // 手机触摸通常没有“右键”或“中键”的概念，
                // 我们将其映射为左键 (MouseButton::Left)
                let button = winit::event::MouseButton::Left;

                if let Err(e) = input_event_sender.push(InputEvent::MouseButton { button, state: button_state }) {
                    warn!("Failed to send touch event as mouse event to render thread: {:?}", e);
                }

                // info!("DEBUG: Received Raw Event - ID: {:?}, Phase: {:?}", touch.id, touch.phase); // 这里的 phase 是 winit 的

                // 直接发送原始的Touch事件到渲染线程
                if let Err(e) = input_event_sender.push(InputEvent::Touch(touch)) {
                    warn!("Failed to send touch event to render thread: {:?}", e);
                }
            }
            _ => {}
        }
    }

    /// 当应用程序即将退出时调用。
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        info!("Application exiting. Sending close command to render thread.");
        if let Some(sender) = self.render_command_sender.take() {
            let _ = sender.send(WgpuStateCommand::Close);
        }

        // 清除窗口引用
        self.window_ref = None;

        println!("Application exiting gracefully. Resources will be cleaned up.");
    }
}
