// SPDX-License-Identifier: GPL-3.0-only
//! Native renderer diagnostic with synthetic UI; never connects to Agent/storage.

use quadrant_ui::{MainWindow, ThemeMode};
use slint::{ComponentHandle, LogicalSize, Timer, TimerMode};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

struct Sample {
    milliseconds: u128,
    width: u32,
    height: u32,
    scale: f32,
    collapsed: bool,
    strip: Vec<u8>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("output directory required")?,
    );
    let main = MainWindow::new()?;
    main.set_agent_connected(true);
    main.set_application_version("Restore diagnostic".into());
    main.invoke_apply_theme(ThemeMode::Dark, true);
    main.window().set_size(LogicalSize::new(1100.0, 720.0));
    main.show()?;
    let samples = Rc::new(RefCell::new(Vec::new()));
    let captured = samples.clone();
    let weak = main.as_weak();
    let start = Instant::now();
    let mut minimized = false;
    let mut restored = false;
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
        let Some(main) = weak.upgrade() else {
            return;
        };
        let elapsed = start.elapsed().as_millis();
        if elapsed >= 4000 && !minimized {
            main.window().set_minimized(true);
            minimized = true;
        }
        if elapsed >= 5000 && !restored {
            main.window().set_minimized(false);
            restored = true;
        }
        if elapsed >= 5800 {
            drop(slint::quit_event_loop());
            return;
        }
        if (3200..4000).contains(&elapsed) || restored {
            match main.window().take_snapshot() {
                Ok(pixels) => {
                    let scale = main.window().scale_factor();
                    let origin = LogicalSize::new(10.0, 40.0).to_physical(scale);
                    let left = origin.width as usize;
                    let width = LogicalSize::new(36.0, 0.0).to_physical(scale).width;
                    let top = origin.height;
                    let mut strip = Vec::new();
                    for row in pixels
                        .as_bytes()
                        .chunks_exact(pixels.width() as usize * 4)
                        .skip(top as usize)
                    {
                        if let Some(part) = row.get(left * 4..(left + width as usize) * 4) {
                            strip.extend_from_slice(part);
                        }
                    }
                    captured.borrow_mut().push(Sample {
                        milliseconds: elapsed,
                        width,
                        height: pixels.height().saturating_sub(top),
                        scale,
                        collapsed: main.get_sidebar_auto_collapsed(),
                        strip,
                    });
                }
                Err(error) => eprintln!("snapshot at {elapsed}: {error}"),
            }
        }
        main.window().request_redraw();
    });
    slint::run_event_loop_until_quit()?;
    main.hide()?;
    std::fs::create_dir_all(&output)?;
    for sample in samples.borrow().iter() {
        println!(
            "{}ms strip={}x{} scale={} collapsed={} valid={}",
            sample.milliseconds,
            sample.width,
            sample.height,
            sample.scale,
            sample.collapsed,
            sample.strip.chunks_exact(4).any(|pixel| pixel[3] != 0)
        );
        if sample.strip.is_empty() {
            continue;
        }
        let file = std::fs::File::create(output.join(format!("{}.png", sample.milliseconds)))?;
        let mut encoder = png::Encoder::new(file, sample.width, sample.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&sample.strip)?;
    }
    Ok(())
}
