use log::{debug, info};
use std::{error::Error, sync::Arc};
use wgpu::{
    Backends, Color, CommandEncoderDescriptor, Device, DeviceDescriptor, Instance,
    InstanceDescriptor, LoadOp, Operations, Queue, RenderPassColorAttachment, RenderPassDescriptor,
    RequestAdapterOptions, StoreOp, Surface, SurfaceConfiguration, SurfaceTarget,
    TextureViewDescriptor,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Fullscreen, Window, WindowId},
};

struct Wgpu<'a> {
    surface: Surface<'a>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
}

impl<'a> Wgpu<'a> {
    async fn new(target: SurfaceTarget<'a>) -> Self {
        info!("Initializing WGPU instance");
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        info!("Creating surface");
        let surface = instance.create_surface(target).unwrap();

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
            surface,
            device,
            queue,
            config,
        }
    }

    /// Resize the surface to the given width and height.
    fn resize(&mut self, width: u32, height: u32) {
        if self.config.width != width || self.config.height != height {
            debug!("Resizing surface to {}x{}", width, height);
            self.config.width = width;
            self.config.height = height;
            if width != 0 && height != 0 {
                self.surface.configure(&self.device, &self.config);
            }
        }
    }
}

#[derive(Default)]
struct WinitApp<'a> {
    wgpu: Option<Wgpu<'a>>,
    window: Option<Arc<Window>>,
}

impl<'a> ApplicationHandler for WinitApp<'a> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        info!("Creating application window");
        let attrs = Window::default_attributes()
            .with_title("LearnWgpu")
            .with_visible(false);
        let window = Arc::new(el.create_window(attrs).unwrap());

        info!("Creating WGPU for window");
        let wgpu = pollster::block_on(Wgpu::new(window.clone().into()));

        info!("Showing application window");
        window.set_visible(true);

        self.wgpu = Some(wgpu);
        self.window = Some(window);
        info!("Application resumed");
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                el.exit();
                info!("Exiting application");
            }
            WindowEvent::Resized(size) => {
                if let Some(wgpu) = &mut self.wgpu {
                    wgpu.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(wgpu) = &mut self.wgpu {
                    if wgpu.config.width == 0 || wgpu.config.height == 0 {
                        return;
                    }

                    let output = wgpu.surface.get_current_texture().unwrap();
                    let view = output
                        .texture
                        .create_view(&TextureViewDescriptor::default());

                    let mut encoder =
                        wgpu.device
                            .create_command_encoder(&CommandEncoderDescriptor {
                                label: Some("Render Encoder"),
                            });

                    let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                        label: Some("Render Pass"),
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
                    drop(render_pass);
                    wgpu.queue.submit(Some(encoder.finish()));
                    output.present();
                }
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
                if let Some(window) = &self.window {
                    if window.fullscreen().is_none() {
                        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    } else {
                        window.set_fullscreen(None);
                    }
                }
            }
            _ => (),
        }
    }
}

fn main() -> Result<(), impl Error> {
    env_logger::init();
    info!("Starting application");

    let el = EventLoop::new().unwrap();
    let mut app = WinitApp::default();
    el.run_app(&mut app)
}
