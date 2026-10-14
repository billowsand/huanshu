//! Embedded icon collections - baked into the binary at compile time.
//! This eliminates runtime dependency on node_modules.

/// Carbon icons JSON - copied into OUT_DIR by build.rs at compile time.
/// Size: ~1.1MB containing 1000+ icons when frontend deps are installed.
pub static CARBON_ICONS_JSON: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/carbon-icons.json"));

/// Carbon icons package version, injected from build.rs via environment variable.
/// Falls back to "0.0.0" if not set (e.g., in some build configurations).
pub const CARBON_VERSION: &str = match option_env!("EMBEDDED_ICONS_CARBON_VERSION") {
    Some(v) => v,
    None => "0.0.0",
};
