use log::{debug, info};
use std::sync::Arc;
use wgpu::{
    Backends, Color, CommandEncoderDescriptor, Device, DeviceDescriptor, Instance,
    InstanceDescriptor, LoadOp, Operations, Queue, RenderPassColorAttachment, RenderPassDescriptor,
    RequestAdapterOptions, StoreOp, Surface, SurfaceConfiguration, TextureViewDescriptor,
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct Wgpu {
    window: Arc<Window>,
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    size_changed: bool,
}

impl Wgpu {
    async fn new(target: Arc<Window>) -> Self {
        info!("Initializing WGPU instance");
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });
        info!("{:?}", instance);

        info!("Creating surface");
        let surface = instance.create_surface(target.clone()).unwrap();
        info!("{:?}", surface);

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
        info!("{:?}", device);
        info!("{:?}", queue);

        info!("Acquiring surface configuration");
        let config = surface
            .get_default_config(
                &adapter,
                target.inner_size().width,
                target.inner_size().height,
            )
            .unwrap();
        info!("{:?}", config);

        info!("WGPU initialization complete");

        Self {
            window: target,
            surface,
            device,
            queue,
            config,
            size_changed: true,
        }
    }

    fn render(&mut self) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }

        if self.size_changed {
            self.surface.configure(&self.device, &self.config);
            self.size_changed = false;
            debug!(
                "Surface size changed to: {}x{}",
                self.config.width, self.config.height
            );
        }

        debug!("Starting render pass");

        let output = self.surface.get_current_texture().unwrap();
        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
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
        self.queue.submit(Some(encoder.finish()));

        debug!("Presenting frame");

        self.window.pre_present_notify();
        output.present();

        debug!("Render pass complete");
    }
}

#[derive(Default)]
struct WgpuAppHandler {
    app: Option<Wgpu>,
}

impl ApplicationHandler for WgpuAppHandler {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        let attrs = Window::default_attributes().with_title("LearnWgpu");
        let window = Arc::new(el.create_window(attrs).unwrap());
        let app = pollster::block_on(Wgpu::new(window));
        self.app.replace(app);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let app = self.app.as_mut().unwrap();

        match event {
            WindowEvent::CloseRequested => {
                el.exit();
                info!("Exiting application");
            }
            WindowEvent::Resized(size) => {
                let config = &mut app.config;
                config.width = size.width;
                config.height = size.height;
                app.size_changed = true;
            }
            WindowEvent::RedrawRequested => {
                app.render();
                app.window.request_redraw();
            }
            WindowEvent::KeyboardInput { .. } => {}
            _ => (),
        }
    }
}

fn main() -> Result<(), impl std::error::Error> {
    env_logger::init();
    info!("Starting application");

    let el = EventLoop::new().unwrap();
    let mut app = WgpuAppHandler::default();
    el.run_app(&mut app)
}
