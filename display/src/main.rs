#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod display;
mod utils;
mod window;

use std::error::Error;

use ingameime_core::interface::{imm32::Imm32InputContext, lib::InputContext};
use log::{debug, info};
use wgpu::rwh::RawWindowHandle;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowId,
};

use crate::{
    display::IngameImeMenu,
    window::{EguiWindow, WindowMode},
};

struct IngameImeApp<'a> {
    window: EguiWindow<'a>,
    ime: Box<dyn InputContext>,
    menu: IngameImeMenu,
}

impl IngameImeApp<'_> {
    fn new(el: &ActiveEventLoop) -> Self {
        info!("IngameImeApp starting");

        let window = EguiWindow::new(el);

        debug!("Create InputContext");
        let ime = match window.handle {
            RawWindowHandle::Win32(handle) => Imm32InputContext::new(handle.hwnd, true).unwrap(),
            _ => {
                panic!("Unsupported platform");
            }
        };

        debug!("Create Ui");
        let menu = IngameImeMenu::new(&window.context);

        debug!("Show window");
        window.inner.set_visible(true);

        info!("IngameImeApp started");
        Self {
            window,
            ime,
            menu,
        }
    }
}

#[derive(Default)]
struct AppHandler<'a> {
    app: Option<IngameImeApp<'a>>,
}

impl<'a> ApplicationHandler for AppHandler<'a> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.app = Some(IngameImeApp::new(el));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let app = self.app.as_mut().unwrap();
        let window = &mut app.window;
        let menu = &mut app.menu;

        match event {
            WindowEvent::CloseRequested => {
                el.exit();
                info!("Exiting application");
            }
            WindowEvent::Resized(size) => {
                window.resize(size.width, size.height);
            }
            WindowEvent::RedrawRequested => {
                if let Some(mut platform) = window.render(menu) {
                    if let Some(ime) = platform.ime {
                        app.ime.set_activated(true);

                        let rect = ime.cursor_rect;
                        app.ime.set_preedit_rect(
                            rect.left() as i32,
                            rect.top() as i32,
                            rect.width() as i32,
                            rect.height() as i32,
                        );
                    } else {
                        app.ime.set_activated(false);
                    }
                    platform.ime = None;
                    window.handler_platform(platform);
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Released,
                        ..
                    },
                ..
            } => window.set_window_mode(WindowMode::Window),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::F11),
                        state: ElementState::Released,
                        ..
                    },
                ..
            } => match window.mode {
                WindowMode::Window => window.set_window_mode(WindowMode::Exclusive),
                WindowMode::Borderless | WindowMode::Exclusive => {
                    window.set_window_mode(WindowMode::Window)
                }
            },
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::F12),
                        state: ElementState::Released,
                        ..
                    },
                ..
            } => match window.mode {
                WindowMode::Window => window.set_window_mode(WindowMode::Borderless),
                WindowMode::Borderless | WindowMode::Exclusive => {
                    window.set_window_mode(WindowMode::Window)
                }
            },
            event => {
                window.on_window_event(event);
            }
        }
    }
}

fn main() -> Result<(), impl Error> {
    env_logger::init();
    info!("Starting application");

    let el = EventLoop::new().unwrap();
    let mut app = AppHandler::default();
    el.run_app(&mut app)
}
