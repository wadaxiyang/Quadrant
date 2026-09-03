//! Opens the development-only Task Editor visual-check surface.

use std::{fs::File, io::BufWriter, path::Path, time::Duration};

use quadrant_ui::{Date, TaskEditorWindow, ThemeMode, Time};
use slint::{ComponentHandle, LogicalSize, Rgba8Pixel, SharedPixelBuffer, SharedString};

fn main() -> Result<(), slint::PlatformError> {
    let editor = TaskEditorWindow::new()?;
    #[cfg(target_os = "windows")]
    editor.set_ui_font_family(SharedString::from("Segoe UI Variable Text"));

    if let (Some(width), Some(height)) = (
        env_f32("QUADRANT_EDITOR_WIDTH"),
        env_f32("QUADRANT_EDITOR_HEIGHT"),
    ) {
        editor.window().set_size(LogicalSize::new(width, height));
    }
    let theme = match std::env::var("QUADRANT_EDITOR_THEME")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "dark" => ThemeMode::Dark,
        "system" => ThemeMode::System,
        _ => ThemeMode::Light,
    };
    editor.invoke_set_theme_mode(theme);

    editor.set_task_id(SharedString::from("visual-check"));
    editor.set_title_text(SharedString::from("Prepare the release checklist"));
    editor.set_notes_text(SharedString::from(
        "Confirm keyboard states, schedule controls, and field-level validation.",
    ));
    editor.set_destination(1);
    editor.set_planned_selected(true);
    editor.set_planned_date(Date {
        year: 2026,
        month: 9,
        day: 4,
    });
    editor.set_due_selected(true);
    editor.set_due_date(Date {
        year: 2026,
        month: 9,
        day: 4,
    });
    editor.set_due_time(Time {
        hour: 17,
        minute: 30,
        second: 0,
    });
    editor.set_due_time_zone(SharedString::from("Asia/Shanghai"));
    editor.set_reminder_selected(true);
    editor.set_reminder_date(Date {
        year: 2026,
        month: 9,
        day: 4,
    });
    editor.set_reminder_time(Time {
        hour: 16,
        minute: 30,
        second: 0,
    });
    editor.set_reminder_time_zone(SharedString::from("Asia/Shanghai"));

    if let Ok(snapshot_path) = std::env::var("QUADRANT_EDITOR_SNAPSHOT") {
        editor.show()?;
        editor.window().request_redraw();
        let editor_weak = editor.as_weak();
        slint::Timer::single_shot(Duration::from_millis(1200), move || {
            let result = editor_weak
                .upgrade()
                .ok_or_else(|| "Task Editor closed before snapshot".into())
                .and_then(|editor| editor.window().take_snapshot())
                .and_then(|pixels| {
                    write_snapshot(Path::new(&snapshot_path), &pixels)
                        .map_err(|error| error.to_string().into())
                });
            if let Err(error) = result {
                eprintln!("failed to save Task Editor snapshot: {error}");
                std::process::exit(2);
            }
            drop(slint::quit_event_loop());
        });
        return slint::run_event_loop();
    }

    editor.run()
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
