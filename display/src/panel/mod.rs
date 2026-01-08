pub mod render_config;

use egui::Context;
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
                if ui
                    .selectable_label(self.active_tab == ActiveTab::General, "General")
                    .clicked()
                {
                    self.active_tab = ActiveTab::General;
                }
                if ui
                    .selectable_label(self.active_tab == ActiveTab::Render, "Render Config")
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
