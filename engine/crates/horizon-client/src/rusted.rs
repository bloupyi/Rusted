//! Rusted's native integration surface. The upstream simulation stays independent.
pub use rusted_platform::{CREDITS, NAME};
pub const VERSION: &str = "0.3.5";
pub const TITLE_LOGO: &str = "rusted/title-logo";
/// No Horizon telemetry, registry migrations or Discord application identity in Rusted.
pub const UPSTREAM_SERVICES: bool = false;
#[path = "../rusted_icon.rs"]
mod icon_data;
pub fn icon() -> Option<winit::window::Icon> {
    winit::window::Icon::from_rgba(icon_data::rgba(), 64, 64).ok()
}
use std::sync::OnceLock;

pub fn mods_directory() -> std::path::PathBuf {
    std::env::var_os("RUSTED_INSTANCE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(crate::logging::game_home)
        .join("mods")
}

/// The bridge sets the original launcher instance; saves still use the isolated native home.
/// Cache the inventory so drawing menus never rereads every mod archive each frame.
pub fn java_mods() -> &'static rusted_platform::Report {
    static REPORT: OnceLock<rusted_platform::Report> = OnceLock::new();
    REPORT.get_or_init(|| rusted_platform::scan(&mods_directory()))
}

pub fn title() -> String {
    format!(
        "{NAME} {VERSION} - Minecraft {}",
        horizon_core::VANILLA_VERSION
    )
}

/// The project's title artwork is a separate PNG beside the executable.
/// Discard transparent padding when placing it, while keeping its aspect ratio.
pub fn title_logo() -> Option<(u32, u32, Vec<u8>)> {
    let path = std::env::var_os("RUSTED_TITLE_LOGO")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .map(|p| p.with_file_name("rusted-title.png"))
        })?;
    if std::fs::metadata(&path).ok()?.len() > 8 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    // Validate dimensions before asking the PNG decoder to allocate its buffer.
    if bytes.get(..8)? != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let width = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
    if width == 0 || height == 0 || width > 4096 || height > 2048 {
        return None;
    }
    let (width, height, pixels) = crate::hud::decode_bytes(&bytes).ok()?;
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (i, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if pixel[3] >= 8 {
            let (x, y) = (i as u32 % width, i as u32 / width);
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if left >= right || top >= bottom {
        return None;
    }
    let mut cropped = Vec::with_capacity(((right - left) * (bottom - top) * 4) as usize);
    for y in top..bottom {
        cropped.extend_from_slice(
            &pixels[((y * width + left) * 4) as usize..((y * width + right) * 4) as usize],
        );
    }
    Some((right - left, bottom - top, cropped))
}
