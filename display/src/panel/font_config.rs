use std::collections::BTreeMap;

use font_kit::{font::Font, source::SystemSource};
use log::error;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FontCategory {
    English,
    Chinese,
    Japanese,
    Korean,
}

pub struct FontConfigPanel {
    // 字体列表
    categories: BTreeMap<FontCategory, BTreeMap<String, Font>>,
    // 选择的字体
    fonts: BTreeMap<FontCategory, Option<String>>,
}

impl FontConfigPanel {
    pub fn new() -> Self {
        let system_fonts = Self::get_system_fonts();

        let mut categories = BTreeMap::new();

        // 初始化分类
        categories.insert(FontCategory::English, BTreeMap::new());
        categories.insert(FontCategory::Chinese, BTreeMap::new());
        categories.insert(FontCategory::Japanese, BTreeMap::new());
        categories.insert(FontCategory::Korean, BTreeMap::new());

        for (name, font) in system_fonts {
            categories
                .get_mut(&FontCategory::English)
                .unwrap()
                .insert(name.clone(), font.clone());

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
            fonts: BTreeMap::new(),
        }
    }

    pub fn get_system_fonts() -> BTreeMap<String, Font> {
        SystemSource::new()
            .all_fonts()
            .inspect_err(|e| error!("Unable to load System fonts: {e}"))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|it| it.load().ok())
            .filter_map(|font| font.postscript_name().map(|name| (name, font)))
            .fold(BTreeMap::new(), |mut acc, (name, handle)| {
                acc.entry(name).or_insert(handle);
                acc
            })
    }

    pub fn font_supports(font: &Font, ranges: &[(u32, u32, usize)]) -> bool {
        // 遍历每个区块，采样检测
        for &(start, end, step) in ranges {
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
                        return true; // 找到任意一个字形，立即返回
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
        for (name, font) in FontConfigPanel::get_system_fonts() {
            let full_name = font.full_name();
            println!("{full_name}: {name}");
        }
    }

    #[test]
    fn test_font_supports_chinese() {
        let fonts = FontConfigPanel::get_system_fonts();
        for (name, font) in fonts {
            if FontConfigPanel::font_supports_chinese(&font) {
                let full_name = font.full_name();
                println!("{full_name}: {name}");
            }
        }
    }

    #[test]
    fn test_font_supports_japanese() {
        let fonts = FontConfigPanel::get_system_fonts();
        for (name, font) in fonts {
            if FontConfigPanel::font_supports_japanese(&font) {
                let full_name = font.full_name();
                println!("{full_name}: {name}");
            }
        }
    }

    #[test]
    fn test_font_supports_korean() {
        let fonts = FontConfigPanel::get_system_fonts();
        for (name, font) in fonts {
            if FontConfigPanel::font_supports_korean(&font) {
                let full_name = font.full_name();
                println!("{full_name}: {name}");
            }
        }
    }
}
