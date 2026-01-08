use std::collections::BTreeMap;

use egui::{Context, FontData, FontDefinitions, Visuals};
use font_kit::{
    font::{self, Font},
    properties::Weight,
};
use log::{debug, info};

use crate::{utils::get_system_fonts, window::EguiMenu};

pub struct IngameImeMenu {
    // 文本框中的文本
    text: String,
    // 当前使用的字体
    font: String,
    // 可用字体列表
    fonts: BTreeMap<String, Font>,
}

impl EguiMenu for IngameImeMenu {
    fn render(&mut self, context: &Context) {
        context.set_visuals(Visuals::light());

        egui::Window::new("IngameIME").show(&context, |ui| {
            // 字体选择下拉菜单
            ui.label("Select Font:");
            egui::ComboBox::from_id_salt("font_select")
                .selected_text(self.fonts[&self.font].full_name())
                .show_ui(ui, |ui| {
                    for (name, font) in self.fonts.iter().rev() {
                        ui.selectable_value(&mut self.font, name.clone(), font.full_name())
                            .changed()
                            .then(|| {
                                info!("Font changed to {}", name);
                                Self::set_font(&self.fonts[name], context);
                            });
                    }
                });

            // 文本框
            ui.label("Text input with IME support");
            ui.text_edit_multiline(&mut self.text);
        });
    }
}

impl IngameImeMenu {
    pub fn new(context: &Context) -> Self {
        info!("Creating IngameImeMenu");

        debug!("Load System Fonts");
        let fonts = get_system_fonts()
            .into_iter()
            .filter(|(_, font)| font.properties().weight >= Weight::NORMAL)
            .filter(|(_, font)| Self::font_supports_cjk(font))
            .collect::<BTreeMap<_, _>>();

        let (name, font) = fonts.iter().rev().next().unwrap();
        Self::set_font(font, context);

        info!("IngameImeMenu Created");

        Self {
            text: String::new(),
            font: name.clone(),
            fonts,
        }
    }

    fn set_font(font: &Font, context: &Context) {
        let name = font.postscript_name().unwrap();
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            name.clone(),
            FontData::from_owned(font.copy_font_data().unwrap().to_vec()).into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, name.clone());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, name.clone());
        context.set_fonts(fonts);
    }

    /// 检测是否支持CJK字符
    pub fn font_supports_cjk(font: &Font) -> bool {
        // 定义核心CJK Unicode区块（起始码点, 结束码点, 采样步长）
        // 步长越大越快，步长越小越准确，100是平衡值
        let cjk_ranges = [
            (0x4E00, 0x9FFF, 100), // CJK统一汉字（基本区）- 核心中文
            (0x3040, 0x309F, 10),  // 平假名（小区块，步长小）
            (0x30A0, 0x30FF, 10),  // 片假名（小区块，步长小）
            (0xAC00, 0xD7AF, 100), // 韩文谚文音节
            (0x3000, 0x303F, 5),   // CJK标点（极小区块，全遍历）
            (0xFF00, 0xFFEF, 20),  // 全角字符
        ];

        // 遍历每个CJK区块，采样检测
        for &(start, end, step) in &cjk_ranges {
            // 1. 先检测区块起始、中间、结束三个关键码点（快速命中）
            let key_points = [
                start,                     // 起始点
                start + (end - start) / 2, // 中间点
                end.min(start + 1000),     // 结束点（最多检测前1000个，避免大区块）
            ];

            for &code_point in &key_points {
                if code_point > end {
                    continue;
                }
                // 检测关键码点是否有字形
                if let Some(ch) = std::char::from_u32(code_point) {
                    if font.glyph_for_char(ch).is_some() {
                        return true; // 找到任意一个CJK字形，立即返回
                    }
                }
            }

            // 2. 关键码点未命中时，按步长采样检测（进一步验证）
            for code_point in (start..=end).step_by(step) {
                if let Some(ch) = std::char::from_u32(code_point) {
                    if font.glyph_for_char(ch).is_some() {
                        return true;
                    }
                }
            }
        }

        false
    }
}
