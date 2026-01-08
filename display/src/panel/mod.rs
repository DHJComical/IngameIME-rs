pub mod font_config;

use egui::{Context, Visuals};
use log::info;

use crate::window::EguiMenu as EguiPanel;

pub struct IngameImePanel {
    // 文本框中的文本
    text: String,
}

impl EguiPanel for IngameImePanel {
    fn render(&mut self, context: &Context) {
        context.set_visuals(Visuals::light());

        egui::Window::new("IngameIME").show(&context, |ui| {
            // 文本框
            ui.label("Text input with IME support");
            ui.text_edit_multiline(&mut self.text);
        });
    }
}

impl IngameImePanel {
    pub fn new(context: &Context) -> Self {
        info!("Creating IngameImeMenu");

        info!("IngameImeMenu Created");

        Self {
            text: String::new(),
        }
    }
}
