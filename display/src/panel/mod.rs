pub mod render;

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use egui::{Button, Context, FontData, FontDefinitions, FontFamily};
use font_kit::font::Font;
use log::{debug, info};

use crate::{panel::render::RenderConfigPanel, window::EguiMenu as EguiPanel};

pub struct FontCache {
    // 字体缓存<postscript_name, FontData>
    cache: BTreeMap<String, Arc<FontData>>,
    // 当前加载的字体<postscript_name>
    curr_fonts: HashSet<String>,
    // 之前加载的字体<postscript_name>
    pub prev_fonts: HashSet<String>,
    // 当前帧是否已经加载过新字体
    has_loaded: bool,
}

impl Default for FontCache {
    fn default() -> Self {
        Self {
            cache: BTreeMap::new(),
            has_loaded: false,
            curr_fonts: HashSet::new(),
            prev_fonts: HashSet::new(),
        }
    }
}

impl FontCache {
    pub fn is_loaded(&self, name: &String) -> bool {
        self.prev_fonts.contains(name)
    }

    pub fn add_font(&mut self, name: &String, font: &Font) {
        if !self.cache.contains_key(name) {
            let font_data = FontData::from_owned(font.copy_font_data().unwrap().to_vec());
            self.cache.insert(name.clone(), Arc::new(font_data));
            self.has_loaded = true;
        }
        self.curr_fonts.insert(name.clone());
    }

    /// 刷新字体缓存和字体定义
    pub fn next_frame(&mut self, ui: &mut egui::Ui) {
        self.cache.retain(|name, _| self.curr_fonts.contains(name));

        // 字体数量不变且无新字体加载，说明无需更新
        if self.curr_fonts.len() != self.prev_fonts.len() || self.has_loaded {
            let mut font_defs = FontDefinitions::default();
            // egui默认字体
            let defaults = font_defs
                .families
                .get(&FontFamily::Proportional)
                .cloned()
                .unwrap_or_default();
            for (name, font) in &self.cache {
                // 字体数据
                font_defs.font_data.insert(name.clone(), font.clone());
                // 字体链
                let chain = std::iter::once(name.clone())
                    .chain(defaults.clone())
                    .collect::<Vec<_>>();
                // 字体族
                font_defs
                    .families
                    .insert(FontFamily::Name(name.clone().into()), chain);
            }
            ui.ctx().set_fonts(font_defs);
            ui.ctx().request_repaint();
            debug!("Font cache updated, total fonts: {}", self.cache.len());
        }

        self.prev_fonts = self.curr_fonts.clone();
        self.curr_fonts.clear();
        self.has_loaded = false;
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum ActiveTab {
    General,
    Render,
    Interface,
}

pub struct IngameImePanel {
    cache: FontCache,
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
                    self.render.render(ui, &mut self.cache);
                }
                ActiveTab::Interface => {
                    ui.label("Interface Settings");
                }
            }
            self.cache.next_frame(ui);
        });
    }
}

impl IngameImePanel {
    pub fn new() -> Self {
        info!("Creating IngameImeMenu");

        let render = RenderConfigPanel::new();

        info!("IngameImeMenu Created");

        Self {
            active: ActiveTab::General,
            render,
            cache: FontCache::default(),
        }
    }
}
