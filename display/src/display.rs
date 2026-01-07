use egui::{Context, Visuals};

use crate::window::EguiMenu;

#[derive(Default)]
pub struct IngameImeMenu {
    text: String,
}

impl EguiMenu for IngameImeMenu {
    fn render(&mut self, context: &Context) {
        context.set_visuals(Visuals::light());

        egui::Window::new("IngameIME").show(&context, |ui| {
            ui.label("Text input with IME support");
            ui.text_edit_multiline(&mut self.text);
        });
    }
}
