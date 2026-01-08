pub mod render_config;

use egui::{Button, Context, Visuals};
use log::info;

use crate::{panel::render_config::RenderConfig, window::EguiMenu as EguiPanel};

#[derive(PartialEq, Eq, Clone, Copy)]
enum ActiveTab {
    General,
    Render,
    Interface,
}

pub struct IngameImePanel {
    active_tab: ActiveTab,
    render_config: RenderConfig,
}

impl EguiPanel for IngameImePanel {
    fn render(&mut self, context: &Context) {
        egui::Window::new("IngameIME").show(&context, |ui| {
            ui.horizontal(|ui| {
                // 亮暗主题
                if self.render_config.style.visuals.dark_mode {
                    if ui
                        .add(Button::new("☀").frame(false))
                        .on_hover_text("Switch to light mode")
                        .clicked()
                    {
                        self.render_config.set_theme(false, ui);
                    }
                } else {
                    if ui
                        .add(Button::new("🌙").frame(false))
                        .on_hover_text("Switch to dark mode")
                        .clicked()
                    {
                        self.render_config.set_theme(true, ui);
                    }
                }

                ui.separator();

                // 选项卡
                if ui
                    .selectable_label(self.active_tab == ActiveTab::General, "General")
                    .clicked()
                {
                    self.active_tab = ActiveTab::General;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Render, "Render")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Render;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Interface, "Interface")
                    .clicked()
                {
                    self.active_tab = ActiveTab::Interface;
                }
            });

            ui.separator();

            match self.active_tab {
                ActiveTab::General => {
                    ui.label("General Settings");
                }
                ActiveTab::Render => {
                    self.render_config.render(ui);
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

        let font_config = RenderConfig::new();

        info!("IngameImeMenu Created");

        Self {
            active_tab: ActiveTab::General,
            render_config: font_config,
        }
    }
}
