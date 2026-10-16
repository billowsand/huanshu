//! 枚举本机安装的字体家族。设置页与 PDF 导出的字体角色都基于它。
//!
//! Windows 走 DirectWrite；其余平台（macOS、Linux）走 fontdb——Typst 扫系统字体用的也是它
//! （`typst-kit`），所以设置页里能选到的字体与幻灯片里能渲染出来的字体是同一批。

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SystemFont {
    pub name: String,
    pub label: String,
    pub aliases: Vec<String>,
}

#[cfg(windows)]
pub fn enumerate_system_fonts() -> Result<Vec<SystemFont>, String> {
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, DWRITE_FACTORY_TYPE_ISOLATED,
    };

    // Each request checks for installations/removals. COM objects remain on this worker.
    let enumerate = || -> windows::core::Result<Vec<SystemFont>> {
        // SAFETY: DirectWrite owns the interfaces; all string buffers include the NUL
        // terminator required by GetString/GetLocaleName and use lengths from the API.
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_ISOLATED)?;
            let mut collection = None;
            factory.GetSystemFontCollection(&mut collection, true)?;
            let collection = collection.ok_or_else(windows::core::Error::from_win32)?;
            let mut fonts = Vec::new();
            for index in 0..collection.GetFontFamilyCount() {
                let names = collection.GetFontFamily(index)?.GetFamilyNames()?;
                let mut localized = Vec::new();
                for name_index in 0..names.GetCount() {
                    let length = names.GetStringLength(name_index)? as usize;
                    let mut buffer = vec![0u16; length + 1];
                    names.GetString(name_index, &mut buffer)?;
                    let name = String::from_utf16_lossy(&buffer[..length]);
                    let locale_length = names.GetLocaleNameLength(name_index)? as usize;
                    let mut locale_buffer = vec![0u16; locale_length + 1];
                    names.GetLocaleName(name_index, &mut locale_buffer)?;
                    let locale = String::from_utf16_lossy(&locale_buffer[..locale_length]);
                    if !name.trim().is_empty() && !name.starts_with('@') {
                        localized.push((locale.to_lowercase(), name));
                    }
                }
                if let Some(font) = family_from_names(localized) {
                    fonts.push(font);
                }
            }
            fonts.sort_by_key(|font| font.name.to_lowercase());
            fonts.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
            Ok(fonts)
        }
    };
    enumerate().map_err(|error| format!("读取系统字体失败：{error}"))
}

#[cfg(not(windows))]
pub fn enumerate_system_fonts() -> Result<Vec<SystemFont>, String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    // 一个家族有多个字面（常规、粗体……），按英文名归并，各字面的本地化名合在一起。
    let mut families: std::collections::BTreeMap<String, Vec<(String, String)>> =
        std::collections::BTreeMap::new();
    for face in db.faces() {
        let Some((primary, _)) = face.families.first() else {
            continue;
        };
        if primary.trim().is_empty() || primary.starts_with('.') {
            // `.SF NS` 之类以点开头的是 macOS 的私有系统字体，不给用户选。
            continue;
        }
        let names = families.entry(primary.to_lowercase()).or_default();
        for (name, language) in &face.families {
            let entry = (locale_tag(*language), name.clone());
            if !names.contains(&entry) {
                names.push(entry);
            }
        }
    }
    let mut fonts: Vec<SystemFont> = families
        .into_values()
        .filter_map(family_from_names)
        .collect();
    fonts.sort_by_key(|font| font.name.to_lowercase());
    fonts.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    if fonts.is_empty() {
        return Err("读取系统字体失败：没有找到任何字体".to_string());
    }
    Ok(fonts)
}

/// fontdb 的语言 → DirectWrite 风格的小写区域名（`family_from_names` 认 `en-us` / `zh-*`）。
#[cfg(not(windows))]
fn locale_tag(language: fontdb::Language) -> String {
    use fontdb::Language as L;
    match language {
        L::English_UnitedStates => "en-us",
        L::Chinese_PeoplesRepublicOfChina => "zh-cn",
        L::Chinese_Singapore => "zh-sg",
        L::Chinese_Taiwan => "zh-tw",
        L::Chinese_HongKongSAR => "zh-hk",
        L::Chinese_MacaoSAR => "zh-mo",
        _ => "other",
    }
    .to_string()
}

fn family_from_names(names: Vec<(String, String)>) -> Option<SystemFont> {
    let name = names
        .iter()
        .find(|(locale, _)| locale == "en-us")
        .or_else(|| names.first())?
        .1
        .clone();
    let label = names
        .iter()
        .find(|(locale, _)| locale == "zh-cn")
        .or_else(|| names.iter().find(|(locale, _)| locale.starts_with("zh")))
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| name.clone());
    let mut aliases: Vec<_> = names.into_iter().map(|(_, name)| name).collect();
    aliases.sort();
    aliases.dedup();
    Some(SystemFont {
        name,
        label,
        aliases,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_names_share_one_family() {
        let font = family_from_names(vec![
            ("zh-cn".into(), "微软雅黑".into()),
            ("en-us".into(), "Microsoft YaHei".into()),
            ("zh-tw".into(), "微軟雅黑".into()),
        ])
        .unwrap();
        assert_eq!(font.name, "Microsoft YaHei");
        assert_eq!(font.label, "微软雅黑");
        assert_eq!(font.aliases.len(), 3);
        assert!(family_from_names(Vec::new()).is_none());
    }

    /// 只在一定装有字体的桌面系统上跑（精简的 Linux 容器可能一个字体都没有）。
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn enumerates_installed_families() {
        let fonts = enumerate_system_fonts().unwrap();
        assert!(!fonts.is_empty());
        assert!(fonts
            .iter()
            .all(|font| !font.name.is_empty() && font.aliases.contains(&font.name)));
        assert!(fonts
            .windows(2)
            .all(|pair| !pair[0].name.eq_ignore_ascii_case(&pair[1].name)));
        println!("enumerated {} installed font families", fonts.len());
    }
}
