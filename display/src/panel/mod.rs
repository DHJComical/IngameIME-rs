pub mod render;

use egui::{Button, Context};
use log::info;

use crate::{panel::render::RenderConfigPanel, window::EguiMenu as EguiPanel};

#[derive(PartialEq, Eq, Clone, Copy)]
enum ActiveTab {
    General,
    Render,
    Interface,
}

pub struct IngameImePanel {
    active: ActiveTab,
    render: RenderConfigPanel,
}

impl EguiPanel for IngameImePanel {
    fn render(&mut self, context: &Context) {
        egui::Window::new("IngameIME").show(&context, |ui| {
            ui.horizontal(|ui| {
                // 亮暗主题
                if self.render.config.dark_mode {
                    if ui
                        .add(Button::new("☀").frame(false))
                        .on_hover_text("Switch to light mode")
                        .clicked()
                    {
                        self.render.set_theme(false, ui);
                    }
                } else {
                    if ui
                        .add(Button::new("🌙").frame(false))
                        .on_hover_text("Switch to dark mode")
                        .clicked()
                    {
                        self.render.set_theme(true, ui);
                    }
                }

                ui.separator();

                // 选项卡
                if ui
                    .selectable_label(self.active == ActiveTab::General, "General")
                    .clicked()
                {
                    self.active = ActiveTab::General;
                }
                if ui
                    .selectable_label(self.active == ActiveTab::Render, "Render")
                    .clicked()
                {
                    self.active = ActiveTab::Render;
                }
                if ui
                    .selectable_label(self.active == ActiveTab::Interface, "Interface")
                    .clicked()
                {
                    self.active = ActiveTab::Interface;
                }
            });

            ui.separator();

            match self.active {
                ActiveTab::General => {
                    ui.label("General Settings");
                }
                ActiveTab::Render => {
                    self.render.render(ui);
                }
                ActiveTab::Interface => {
                    ui.label("Interface Settings");
                }
            }
        });
    }
}

impl IngameImePanel {
    pub fn new(context: &Context) -> Self {
        info!("Creating IngameImeMenu");

        let font_config = RenderConfigPanel::new();

        info!("IngameImeMenu Created");

        Self {
            active: ActiveTab::General,
            render: font_config,
        }
    }
}
