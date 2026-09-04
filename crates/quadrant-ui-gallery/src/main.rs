//! Development-only Fluent component gallery host.

#![deny(unsafe_code)]
#![allow(missing_docs)]

use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
    time::Duration,
};

use slint::{ComponentHandle, LogicalSize, Rgba8Pixel, SharedPixelBuffer, SharedString, Weak};

slint::include_modules!();

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

    if let Some(page) = env_i32("QUADRANT_GALLERY_PAGE") {
        gallery.set_gallery_page(page.clamp(0, 7));
    }

    if let Some(preview) = env_i32("QUADRANT_GALLERY_PREVIEW") {
        gallery.set_preview_mode(preview.clamp(0, 2));
    }

    if let Ok(snapshot_path) = std::env::var("QUADRANT_GALLERY_SNAPSHOT") {
        gallery.show()?;
        gallery.window().request_redraw();
        schedule_snapshot(
            gallery.as_weak(),
            PathBuf::from(snapshot_path),
            3,
            Duration::from_millis(1200),
        );
        return slint::run_event_loop();
    }

    gallery.run()
}

fn env_f32(name: &str) -> Option<f32> {
    std::env::var(name).ok()?.parse().ok()
}

fn env_i32(name: &str) -> Option<i32> {
    std::env::var(name).ok()?.parse().ok()
}

fn schedule_snapshot(
    gallery_weak: Weak<DesignGalleryWindow>,
    snapshot_path: PathBuf,
    attempts_remaining: u8,
    delay: Duration,
) {
    slint::Timer::single_shot(delay, move || {
        let Some(gallery) = gallery_weak.upgrade() else {
            eprintln!("failed to save Design Gallery snapshot: gallery closed before snapshot");
            std::process::exit(2);
        };

        match gallery.window().take_snapshot() {
            Ok(pixels) if snapshot_has_visible_pixels(&pixels) => {
                if let Err(error) = write_snapshot(&snapshot_path, &pixels) {
                    eprintln!("failed to save Design Gallery snapshot: {error}");
                    std::process::exit(2);
                }
                drop(slint::quit_event_loop());
            }
            Ok(_) if attempts_remaining > 1 => {
                gallery.window().request_redraw();
                drop(gallery);
                schedule_snapshot(
                    gallery_weak,
                    snapshot_path,
                    attempts_remaining - 1,
                    Duration::from_millis(800),
                );
            }
            Ok(_) => {
                eprintln!(
                    "failed to save Design Gallery snapshot: renderer returned an all-transparent frame"
                );
                std::process::exit(2);
            }
            Err(error) => {
                eprintln!("failed to save Design Gallery snapshot: {error}");
                std::process::exit(2);
            }
        }
    });
}

fn snapshot_has_visible_pixels(pixels: &SharedPixelBuffer<Rgba8Pixel>) -> bool {
    pixels.as_bytes().chunks_exact(4).any(|rgba| rgba[3] != 0)
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
