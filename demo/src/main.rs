use egui::{Context, ViewportId, Visuals};
use egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor};
use egui_winit::State;
use log::{debug, info};
use std::{error::Error, sync::Arc};
use wgpu::{
    Backends, Color, CommandEncoderDescriptor, Device, DeviceDescriptor, Instance,
    InstanceDescriptor, LoadOp, Operations, Queue, RenderPassColorAttachment, RenderPassDescriptor,
    RequestAdapterOptions, StoreOp, Surface, SurfaceConfiguration, TextureViewDescriptor,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Fullscreen, Window, WindowId},
};

struct Wgpu<'a> {
    window: Arc<Window>,
    surface: Surface<'a>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
}

impl<'a> Wgpu<'a> {
    async fn new(window: Arc<Window>) -> Self {
        info!("Initializing WGPU instance");
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        info!("Creating surface");
        let surface = instance.create_surface(window.clone()).unwrap();

        info!("Requesting adapter");
        let adapter = instance
            .request_adapter(&RequestAdapterOptions::default())
            .await
            .unwrap();
        info!("{:?}", adapter.get_info());

        info!("Requesting device and queue");
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor::default())
            .await
            .unwrap();

        info!("Acquiring default surface configuration");
        let config = surface.get_default_config(&adapter, 0, 0).unwrap();
        debug!("{:?}", config);

        info!("WGPU initialization complete");

        Self {
            window,
            surface,
            device,
            queue,
            config,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if self.config.width != width || self.config.height != height {
            debug!("Resizing surface to {}x{}", width, height);
            self.config.width = width;
            self.config.height = height;
            // Surface 设置为 0 尺寸会导致崩溃
            if width != 0 && height != 0 {
                self.surface.configure(&self.device, &self.config);
            }
        }
    }
}

trait EguiMenu {
    fn render(&mut self, context: &Context);
}

struct Egui {
    context: Context,
    state: State,
    renderer: Renderer,
}

impl Egui {
    fn new(wgpu: &Wgpu) -> Self {
        info!("Creating egui context");
        let context = Context::default();

        info!("Creating egui state");
        let state = State::new(
            context.clone(),
            ViewportId::default(),
            wgpu.window.as_ref(),
            Some(wgpu.window.scale_factor() as f32),
            None,
            None,
        );

        info!("Creating egui renderer");
        let renderer = Renderer::new(&wgpu.device, wgpu.config.format, RendererOptions::default());

        Self {
            context,
            state,
            renderer,
        }
    }

    fn render(
        &mut self,
        wgpu: &Wgpu,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        ui: &mut dyn EguiMenu,
    ) {
        // 获取输入并更新界面
        let input = self.state.take_egui_input(&wgpu.window);
        let full_output = self.context.run(input, |context| {
            ui.render(context);
        });

        // 处理平台输出
        self.state
            .handle_platform_output(&wgpu.window, full_output.platform_output);

        // 图元信息
        let primitives = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        // 屏幕信息
        let screen_info = ScreenDescriptor {
            size_in_pixels: [wgpu.config.width, wgpu.config.height],
            pixels_per_point: full_output.pixels_per_point,
        };
        // 更新纹理
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(&wgpu.device, &wgpu.queue, *id, image_delta);
        }
        // 更新缓冲区
        self.renderer.update_buffers(
            &wgpu.device,
            &wgpu.queue,
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
            wgpu.window.request_redraw();
        }
    }
}

#[derive(Default)]
struct IngameImeMenu {
    text: String,
}

impl EguiMenu for IngameImeMenu {
    fn render(&mut self, context: &Context) {
        context.set_visuals(Visuals::light());

        egui::Window::new("IngameIME").show(&context, |ui| {
            ui.label("Text input with IME support.");
            if ui.text_edit_multiline(&mut self.text).has_focus() {}
        });
    }
}

struct WinitApp<'a> {
    wgpu: Wgpu<'a>,
    egui: Egui,
    menu: IngameImeMenu,
}

impl WinitApp<'_> {
    fn new(el: &ActiveEventLoop) -> Self {
        info!("Creating application window");
        let attrs = Window::default_attributes()
            .with_title("LearnWgpu")
            .with_visible(false);
        let window = Arc::new(el.create_window(attrs).unwrap());

        info!("Creating WGPU for window");
        let wgpu = pollster::block_on(Wgpu::new(window.clone().into()));

        info!("Creating egui");
        let egui = Egui::new(&wgpu);

        info!("Creating IngameIME menu");
        let menu = IngameImeMenu::default();

        info!("Showing application window");
        window.set_visible(true);

        info!("WinitApp initialized");
        Self { wgpu, egui, menu }
    }
}

#[derive(Default)]
struct WinitAppHandler<'a> {
    app: Option<WinitApp<'a>>,
}

impl<'a> ApplicationHandler for WinitAppHandler<'a> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.app = Some(WinitApp::new(el));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let app = self.app.as_mut().unwrap();
        let wgpu = &mut app.wgpu;
        let egui = &mut app.egui;
        let menu = &mut app.menu;
        let window = wgpu.window.as_ref();

        match event {
            WindowEvent::CloseRequested => {
                el.exit();
                info!("Exiting application");
            }
            WindowEvent::Resized(size) => {
                wgpu.resize(size.width, size.height);
            }
            WindowEvent::RedrawRequested => {
                // 渲染 0 尺寸画面会导致崩溃
                if wgpu.config.width == 0 || wgpu.config.height == 0 {
                    return;
                }

                // 获取当前纹理视图
                let output = wgpu.surface.get_current_texture().unwrap();
                let view = output
                    .texture
                    .create_view(&TextureViewDescriptor::default());

                // 初始化命令编码器
                let mut encoder = wgpu
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

                // 场景绘制
                {
                    let rpass = encoder.begin_render_pass(&RenderPassDescriptor {
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
                    drop(rpass);
                }

                // Egui 绘制
                {
                    egui.render(wgpu, &mut encoder, &view, menu);
                }

                wgpu.queue.submit(Some(encoder.finish()));
                output.present();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::F11),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                if window.fullscreen().is_none() {
                    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                } else {
                    window.set_fullscreen(None);
                }
            }
            event => {
                if egui.state.on_window_event(&window, &event).repaint {
                    window.request_redraw();
                }
            }
        }
    }
}

fn main() -> Result<(), impl Error> {
    env_logger::init();
    info!("Starting application");

    let el = EventLoop::new().unwrap();
    let mut app = WinitAppHandler::default();
    el.run_app(&mut app)
}
