//! Opens the development-only Fluent component gallery.

use quadrant_ui::DesignGalleryWindow;
use slint::{ComponentHandle, SharedString};

fn main() -> Result<(), slint::PlatformError> {
    let gallery = DesignGalleryWindow::new()?;
    #[cfg(target_os = "windows")]
    gallery.set_ui_font_family(SharedString::from("Segoe UI Variable Text"));
    gallery.run()
}
