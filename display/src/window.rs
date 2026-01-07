use egui::{Context, PlatformOutput, ViewportId};
use egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor};
use egui_winit::State;
use log::{debug, error, info};
use std::sync::Arc;
use wgpu::rwh::{HasWindowHandle, RawWindowHandle};
use wgpu::{
    Backends, Color, CommandEncoderDescriptor, Device, DeviceDescriptor, Instance,
    InstanceDescriptor, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration,
    TextureViewDescriptor,
};
use wgpu::{
    CommandEncoder, LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp,
    TextureView,
};
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Fullscreen, Window};

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum WindowMode {
    /// 窗口化
    #[default]
    Window,
    /// 全屏无边框
    Borderless,
    /// 全屏独占
    Exclusive,
}

pub trait EguiMenu {
    fn render(&mut self, context: &Context);
}

pub struct EguiWindow<'a> {
    pub handle: RawWindowHandle,
    pub inner: Arc<Window>,
    pub surface: Surface<'a>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub context: Context,
    pub state: State,
    pub renderer: Renderer,
    pub mode: WindowMode,
}

impl<'a> EguiWindow<'a> {
    pub fn new(el: &ActiveEventLoop) -> Self {
        info!("Creating EguiWindow");

        let attrs = Window::default_attributes()
            .with_title("IngameIME Application")
            .with_visible(false);
        let window = Arc::new(el.create_window(attrs).unwrap());

        debug!("Init Wgpu instance");
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        debug!("Create surface");
        let surface = instance.create_surface(window.clone()).unwrap();

        debug!("Request adapter");
        let adapter =
            pollster::block_on(instance.request_adapter(&RequestAdapterOptions::default()))
                .unwrap();
        info!("{:?}", adapter.get_info());

        debug!("Request device and queue");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&DeviceDescriptor::default())).unwrap();

        debug!("Acquire default surface configuration");
        let config = surface.get_default_config(&adapter, 0, 0).unwrap();
        debug!("{:?}", config);

        debug!("Create Egui context");
        let context = Context::default();

        debug!("Create Egui state");
        let state = State::new(
            context.clone(),
            ViewportId::default(),
            window.as_ref(),
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        debug!("Create Egui renderer");
        let renderer = Renderer::new(&device, config.format, RendererOptions::default());

        debug!("Get RawWindowHandle");
        let handle = window
            .window_handle()
            .inspect_err(|e| error!("Unable to get RawWindowHandle: {e}"))
            .expect("Unable to get RawWindowHandle")
            .as_raw();

        info!("EguiWindow created");

        Self {
            handle,
            inner: window,
            surface,
            device,
            queue,
            config,
            context,
            state,
            renderer,
            mode: WindowMode::Window,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.config.width != width || self.config.height != height {
            debug!("Resize window to {}x{}", width, height);
            self.config.width = width;
            self.config.height = height;
            // Surface 设置为 0 尺寸会导致崩溃
            if width != 0 && height != 0 {
                self.surface.configure(&self.device, &self.config);
            }
        }
    }

    fn render_egui(
        &mut self,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        ui: &mut dyn EguiMenu,
    ) -> PlatformOutput {
        // 获取输入并更新界面
        let input = self.state.take_egui_input(&self.inner);
        let full_output = self.context.run(input, |context| {
            ui.render(context);
        });

        // 图元信息
        let primitives = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        // 屏幕信息
        let screen_info = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };
        // 更新纹理
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }
        // 更新缓冲区
        self.renderer.update_buffers(
            &self.device,
            &self.queue,
            encoder,
            &primitives,
            &screen_info,
        );
        // 绘制图元
        {
            let mut rpass = encoder
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("Render Egui"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.renderer.render(&mut rpass, &primitives, &screen_info);
        }
        // 释放纹理
        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
        // 检查是否需要重绘
        if self.context.has_requested_repaint() {
            self.inner.request_redraw();
        }

        return full_output.platform_output;
    }

    /// 蓝色背景
    fn render_scene(&mut self, encoder: &mut CommandEncoder, view: &TextureView) {
        let _ = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Render Scene"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Clear(Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }

    pub fn render(&mut self, ui: &mut dyn EguiMenu) -> Option<PlatformOutput> {
        // 渲染 0 尺寸画面会导致崩溃
        if self.config.width == 0 || self.config.height == 0 {
            return None;
        }

        // 获取当前纹理视图
        let output = self.surface.get_current_texture().unwrap();
        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());

        // 初始化命令编码器
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // 场景绘制
        self.render_scene(&mut encoder, &view);

        // Egui绘制
        let platform = self.render_egui(&mut encoder, &view, ui);

        // 提交渲染
        self.queue.submit(Some(encoder.finish()));
        output.present();

        return Some(platform);
    }

    pub fn handler_platform(&mut self, platform: PlatformOutput) {
        self.state.handle_platform_output(&self.inner, platform);
    }

    fn set_window_mode_broderless(&self) {
        self.inner
            .set_fullscreen(Some(Fullscreen::Borderless(None)));
        debug!("Set to Broderless Mode");
    }

    fn set_window_mode_exclusive(&self) {
        if let Some(display) = self.inner.current_monitor() {
            // prefer currently used refresh rate, to prevent black screen
            if let Some(refresh_rate) = display.refresh_rate_millihertz() {
                if let Some(mode) = display
                    .video_modes()
                    .find(|it| it.refresh_rate_millihertz() == refresh_rate)
                    .take()
                {
                    debug!("Set to Exclusive Mode: {mode}, Refresh Rate: {refresh_rate:?}");
                    self.inner.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
                    return;
                }
            }
            // fallback to the first video mode
            if let Some(mode) = display.video_modes().next().take() {
                debug!("Set to Exclusive Mode: {mode}");
                self.inner.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
                return;
            }
        }
        error!("Unable to set to Exclusive Mode, fallback to Broderless Mode");
        self.set_window_mode_broderless();
    }

    pub fn on_window_event(&mut self, event: WindowEvent) {
        if let WindowEvent::Focused(focused) = event {
            // 窗口失去焦点时，取消独占全屏，从而能正常切换窗口
            if self.mode == WindowMode::Exclusive {
                if focused {
                    self.set_window_mode_exclusive();
                } else {
                    self.set_window_mode_broderless();
                }
            }
        }
        if self.state.on_window_event(&self.inner, &event).repaint {
            self.inner.request_redraw();
        }
    }

    pub fn set_window_mode(&mut self, mode: WindowMode) {
        if self.mode != mode {
            self.mode = mode;
            match self.mode {
                WindowMode::Window => {
                    self.inner.set_fullscreen(None);
                    let _ = self.inner.request_inner_size(LogicalSize::new(800, 600));
                    debug!("Set to Window Mode");
                }
                WindowMode::Borderless => {
                    self.set_window_mode_broderless();
                }
                WindowMode::Exclusive => {
                    self.set_window_mode_exclusive();
                }
            }
        }
    }
}
