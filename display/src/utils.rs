use std::collections::BTreeMap;

use font_kit::{font::Font, source::SystemSource};
use log::error;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_system_fonts() {
        for (name, font) in get_system_fonts() {
            let props = font.properties();
            let family = font.family_name();
            println!("{name}: Family: {family}, {props:?}");
        }
    }
}
