use std::path::Path;

fn main() {
    // Read version from package.json for @iconify-json/carbon
    let package_json_path = Path::new("../../package.json");
    if let Ok(raw) = std::fs::read_to_string(package_json_path) {
        // Simple regex-free extraction: find '"@iconify-json/carbon": "<version>"'
        if let Some(start) = raw.find("\"@iconify-json/carbon\"") {
            let after_name = &raw[start..];
            if let Some(colon) = after_name.find(':') {
                let after_colon = &after_name[colon + 1..];
                // Skip whitespace and find opening quote
                let trimmed = after_colon.trim_start();
                if let Some(stripped) = trimmed.strip_prefix('"') {
                    if let Some(end_quote) = stripped.find('"') {
                        let version = &stripped[..end_quote];
                        println!("cargo:rustc-env=EMBEDDED_ICONS_CARBON_VERSION={}", version);
                    }
                }
            }
        }
    }

    tauri_build::build()
}
