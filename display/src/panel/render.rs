use std::{collections::BTreeMap, iter, ops::RangeInclusive};

use ab_glyph::FontVec;
use egui::{ComboBox, DragValue, FontFamily, FontId, RichText, Theme};
use font_kit::{font::Font, source::SystemSource};
use log::error;

use crate::panel::FontCache;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FontCategory {
    English,
    Chinese,
    Japanese,
    Korean,
}

pub struct RenderConfig {
    // 亮暗模式
    pub dark_mode: bool,
    // 字体缩放
    pub font_scale: u32,
    // 选择的字体
    pub fonts: BTreeMap<FontCategory, Option<String>>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        let mut fonts = BTreeMap::new();
        fonts.insert(FontCategory::English, None);
        fonts.insert(FontCategory::Chinese, None);
        fonts.insert(FontCategory::Japanese, None);
        fonts.insert(FontCategory::Korean, None);

        Self {
            dark_mode: true,
            font_scale: 100,
            fonts,
        }
    }
}

struct FontComboBox;

impl FontComboBox {
    pub fn render(
        ui: &mut egui::Ui,
        cache: &mut FontCache,
        selected: &mut String,
        fonts: &BTreeMap<String, Font>,
    ) {
        let size = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Body)
            .unwrap()
            .size;

        ComboBox::from_label("Select Font")
            .width(200.0)
            .selected_text(selected.clone())
            .show_ui(ui, |ui| {
                for (name, font) in fonts {
                    if cache.is_loaded(name) {
                        if ui
                            .selectable_label(
                                *selected == *name,
                                RichText::new(font.full_name()).font(FontId {
                                    size,
                                    family: FontFamily::Name(name.clone().into()),
                                }),
                            )
                            .clicked()
                        {
                            *selected = name.clone();
                        }
                    }
                }
            });
    }
}

#[derive(Default)]
pub struct RenderConfigPanel {
    // 字体列表
    pub categories: BTreeMap<FontCategory, BTreeMap<String, Font>>,
    // 渲染配置
    pub config: RenderConfig,
}

impl RenderConfigPanel {
    pub fn new() -> Self {
        let system_fonts = Self::get_system_fonts();

        let mut categories = BTreeMap::new();

        // 初始化分类
        categories.insert(FontCategory::English, BTreeMap::new());
        categories.insert(FontCategory::Chinese, BTreeMap::new());
        categories.insert(FontCategory::Japanese, BTreeMap::new());
        categories.insert(FontCategory::Korean, BTreeMap::new());

        for (name, font) in &system_fonts {
            if Self::font_supports_english(&font) {
                categories
                    .get_mut(&FontCategory::English)
                    .unwrap()
                    .insert(name.clone(), font.clone());
            }

            if Self::font_supports_chinese(&font) {
                categories
                    .get_mut(&FontCategory::Chinese)
                    .unwrap()
                    .insert(name.clone(), font.clone());
            }

            if Self::font_supports_japanese(&font) {
                categories
                    .get_mut(&FontCategory::Japanese)
                    .unwrap()
                    .insert(name.clone(), font.clone());
            }

            if Self::font_supports_korean(&font) {
                categories
                    .get_mut(&FontCategory::Korean)
                    .unwrap()
                    .insert(name.clone(), font.clone());
            }
        }

        Self {
            categories,
            ..Default::default()
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, cache: &mut FontCache) {
        // 字体大小
        ui.horizontal(|ui| {
            ui.label("Font Size:");
            if ui
                .add(
                    DragValue::new(&mut self.config.font_scale)
                        .speed(1)
                        .range(RangeInclusive::new(50, 300)),
                )
                .changed()
            {
                // 更新字体大小
                self.set_font_scale(self.config.font_scale, ui);
            }
        });

        // 更新字体缓存
        for fonts in self.categories.values() {
            for (name, font) in fonts {
                cache.add_font(name, font);
            }
        }

        // 渲染字体列表
        let mut selected = String::new();
        ui.push_id(1, |ui| {
            FontComboBox::render(
                ui,
                cache,
                &mut selected,
                &self.categories[&FontCategory::English],
            );
        });
        ui.push_id(2, |ui| {
            FontComboBox::render(
                ui,
                cache,
                &mut selected,
                &self.categories[&FontCategory::Chinese],
            );
        });
        ui.push_id(3, |ui| {
            FontComboBox::render(
                ui,
                cache,
                &mut selected,
                &self.categories[&FontCategory::Japanese],
            );
        });
        ui.push_id(4, |ui| {
            FontComboBox::render(
                ui,
                cache,
                &mut selected,
                &self.categories[&FontCategory::Korean],
            );
        });
    }

    fn update_style(&mut self, ui: &mut egui::Ui) {
        let mut style = if self.config.dark_mode {
            Theme::Dark.default_style()
        } else {
            Theme::Light.default_style()
        };
        style.text_styles.iter_mut().for_each(|it| {
            it.1.size *= self.config.font_scale as f32 / 100.0;
        });
        ui.ctx().set_style(style);
    }

    pub fn set_theme(&mut self, dark_mode: bool, ui: &mut egui::Ui) {
        self.config.dark_mode = dark_mode;
        self.update_style(ui);
    }

    pub fn set_font_scale(&mut self, size: u32, ui: &mut egui::Ui) {
        self.config.font_scale = size;
        self.update_style(ui);
    }

    pub fn get_system_fonts() -> BTreeMap<String, Font> {
        SystemSource::new()
            .all_fonts()
            .inspect_err(|e| error!("Unable to load System fonts: {e}"))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|it| it.load().ok())
            .filter(|font| {
                font.copy_font_data()
                    .map(|it| it.to_vec())
                    .and_then(|it| FontVec::try_from_vec(it).ok())
                    .is_some()
            })
            .filter_map(|font| font.postscript_name().map(|name| (name, font)))
            .fold(BTreeMap::new(), |mut acc, (name, handle)| {
                acc.entry(name).or_insert(handle);
                acc
            })
    }

    pub fn font_supports(font: &Font, ranges: &[(u32, u32, usize)]) -> bool {
        for &(start, end, step) in ranges {
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

    /// 检测是否支持英文字符
    pub fn font_supports_english(font: &Font) -> bool {
        let ranges = [
            (0x0041, 0x005A, 1), // ASCII字符
            (0x0061, 0x007A, 1), // ASCII字符
        ];
        Self::font_supports(font, &ranges)
    }

    /// 检测是否支持中文字符
    pub fn font_supports_chinese(font: &Font) -> bool {
        let ranges = [
            (0x4E00, 0x9FFF, 100), // CJK统一汉字
        ];
        Self::font_supports(font, &ranges)
    }

    pub fn font_supports_japanese(font: &Font) -> bool {
        let ranges = [
            (0x3040, 0x309F, 10), // 平假名（小区块，步长小）
            (0x30A0, 0x30FF, 10), // 片假名（小区块，步长小）
        ];
        Self::font_supports(font, &ranges)
    }

    pub fn font_supports_korean(font: &Font) -> bool {
        let ranges = [
            (0xAC00, 0xD7AF, 100), // 韩文谚文音节
        ];
        Self::font_supports(font, &ranges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_system_fonts() {
        for (name, font) in RenderConfigPanel::get_system_fonts() {
            let full_name = font.full_name();
            println!("{full_name}: {name}");
        }
    }

    #[test]
    fn test_font_supports_english() {
        let fonts = RenderConfigPanel::get_system_fonts();
        for (name, font) in fonts {
            if RenderConfigPanel::font_supports_english(&font) {
                let full_name = font.full_name();
                println!("{full_name}: {name}");
            }
        }
    }

    #[test]
    fn test_font_supports_chinese() {
        let fonts = RenderConfigPanel::get_system_fonts();
        for (name, font) in fonts {
            if RenderConfigPanel::font_supports_chinese(&font) {
                let full_name = font.full_name();
                println!("{full_name}: {name}");
            }
        }
    }

    #[test]
    fn test_font_supports_japanese() {
        let fonts = RenderConfigPanel::get_system_fonts();
        for (name, font) in fonts {
            if RenderConfigPanel::font_supports_japanese(&font) {
                let full_name = font.full_name();
                println!("{full_name}: {name}");
            }
        }
    }

    #[test]
    fn test_font_supports_korean() {
        let fonts = RenderConfigPanel::get_system_fonts();
        for (name, font) in fonts {
            if RenderConfigPanel::font_supports_korean(&font) {
                let full_name = font.full_name();
                println!("{full_name}: {name}");
            }
        }
    }
}
