//! Opens the development-only Fluent component gallery.

use quadrant_ui::{DesignGalleryWindow, ThemeMode};
use slint::{ComponentHandle, LogicalSize, SharedString};

fn main() -> Result<(), slint::PlatformError> {
    let gallery = DesignGalleryWindow::new()?;
    #[cfg(target_os = "windows")]
    gallery.set_ui_font_family(SharedString::from("Segoe UI Variable Text"));

    if let (Some(width), Some(height)) = (
        env_f32("QUADRANT_GALLERY_WIDTH"),
        env_f32("QUADRANT_GALLERY_HEIGHT"),
    ) {
        gallery.window().set_size(LogicalSize::new(width, height));
    }

    if let Ok(theme) = std::env::var("QUADRANT_GALLERY_THEME") {
        let theme = match theme.to_ascii_lowercase().as_str() {
            "dark" => ThemeMode::Dark,
            "system" => ThemeMode::System,
            _ => ThemeMode::Light,
        };
        gallery.set_gallery_theme(theme);
    }

    gallery.run()
}

fn env_f32(name: &str) -> Option<f32> {
    std::env::var(name).ok()?.parse().ok()
}
