//! Opens the development-only Fluent component gallery.

use std::{fs::File, io::BufWriter, path::Path, time::Duration};

use quadrant_ui::{DesignGalleryWindow, ThemeMode};
use slint::{ComponentHandle, LogicalSize, Rgba8Pixel, SharedPixelBuffer, SharedString};

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

    if let Ok(snapshot_path) = std::env::var("QUADRANT_GALLERY_SNAPSHOT") {
        gallery.show()?;
        gallery.window().request_redraw();
        let gallery_weak = gallery.as_weak();
        slint::Timer::single_shot(Duration::from_millis(1200), move || {
            let result = gallery_weak
                .upgrade()
                .ok_or_else(|| "gallery closed before snapshot".into())
                .and_then(|gallery| gallery.window().take_snapshot())
                .and_then(|pixels| {
                    write_snapshot(Path::new(&snapshot_path), &pixels)
                        .map_err(|error| error.to_string().into())
                });
            if let Err(error) = result {
                eprintln!("failed to save Design Gallery snapshot: {error}");
                std::process::exit(2);
            }
            drop(slint::quit_event_loop());
        });
        return slint::run_event_loop();
    }

    gallery.run()
}

fn env_f32(name: &str) -> Option<f32> {
    std::env::var(name).ok()?.parse().ok()
}

fn write_snapshot(
    path: &Path,
    pixels: &SharedPixelBuffer<Rgba8Pixel>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = png::Encoder::new(file, pixels.width(), pixels.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()?
        .write_image_data(pixels.as_bytes())?;
    Ok(())
}
