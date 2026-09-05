// SPDX-License-Identifier: GPL-3.0-only
//! Real Slint components on a headless software window adapter; no native effects.

use super::*;
use slint::platform::{
    Platform, WindowAdapter,
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
};
use std::cell::RefCell;

struct Headless(Rc<RefCell<Vec<Rc<MinimalSoftwareWindow>>>>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::default());
        self.0.borrow_mut().push(window.clone());
        Ok(window)
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        assert!(self.0.borrow()[0].is_visible());
        Ok(())
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One ordered lifecycle story on a single Slint context.
fn ipc_updates_preserve_drafts_until_confirmation_and_restore_authoritative_state() {
    let windows = Rc::new(RefCell::new(Vec::new()));
    slint::platform::set_platform(Box::new(Headless(windows.clone()))).unwrap();
    let snapshot: AppSnapshot = serde_json::from_str(include_str!(
        "../../quadrant-protocol/tests/fixtures/snapshot_v1.json"
    ))
    .unwrap();
    let submissions = Rc::new(RefCell::new(Vec::new()));
    let received = submissions.clone();
    let shell = UiShell::new(&snapshot, "test", move |intent| {
        received.borrow_mut().push(intent);
        true
    })
    .unwrap();
    let apply = |update| {
        apply_client_update(
            &shell.main_window,
            &shell.quick_add,
            &shell.task_editor,
            &shell.focus_session,
            update,
        );
    };
    apply(ClientUpdate::Connection {
        state: ConnectionState::Ready,
        message: String::new(),
    });
    shell.main_window.set_current_route(5);
    shell.quick_add.set_title_text("Keep this draft".into());
    shell.quick_add.show().unwrap();
    shell
        .quick_add
        .invoke_submitted("Keep this draft".into(), 0);
    assert!(shell.quick_add.window().is_visible());
    assert_eq!(shell.quick_add.get_title_text(), "Keep this draft");
    assert!(!shell.quick_add.get_can_submit());
    assert!(shell.main_window.get_command_pending());
    assert_eq!(submissions.borrow().len(), 1);
    shell.quick_add.invoke_submitted("duplicate".into(), 0);
    assert_eq!(submissions.borrow().len(), 1);

    apply(ClientUpdate::Connection {
        state: ConnectionState::Reconnecting,
        message: "Connection lost; operation outcome unknown.".into(),
    });
    assert!(!shell.main_window.get_agent_connected());
    assert!(!shell.task_editor.get_can_submit());
    if let Some(directory) = std::env::var_os("QUADRANT_IPC_RENDER_DIR") {
        shell.main_window.show().unwrap();
        render_review(
            &windows.borrow()[0],
            &std::path::PathBuf::from(&directory).join("disconnected.png"),
            900,
            640,
        );
        render_review(
            &windows.borrow()[1],
            &std::path::PathBuf::from(directory).join("quick-add-disconnected.png"),
            520,
            252,
        );
    }
    assert_eq!(shell.quick_add.get_title_text(), "Keep this draft");
    shell.main_window.hide().unwrap();
    apply(ClientUpdate::Connection {
        state: ConnectionState::Unavailable,
        message: "Reconnect exhausted.".into(),
    });
    assert!(shell.main_window.window().is_visible());
    apply(ClientUpdate::Snapshot(Box::new(snapshot.clone())));
    assert_eq!(shell.main_window.get_current_route(), 5);
    assert_eq!(shell.quick_add.get_title_text(), "Keep this draft");
    assert_eq!(*shell.focus_session.lock().unwrap(), snapshot.focus.session);
    apply(ClientUpdate::Connection {
        state: ConnectionState::Ready,
        message: String::new(),
    });

    // Selecting a persistent preference only submits intent. Agent push applies it.
    shell
        .main_window
        .invoke_theme_selected(SlintThemeMode::Dark);
    assert!(matches!(
        submissions.borrow().last(),
        Some(UiIntent::SetTheme(ApplicationThemeMode::Dark))
    ));
    apply(ClientUpdate::Event(ServerEvent::ThemeChanged {
        theme_mode: ApplicationThemeMode::Light,
        system_theme: SystemTheme::Dark,
    }));
    apply(ClientUpdate::Connection {
        state: ConnectionState::Ready,
        message: String::new(),
    });
    let before = shell.main_window.get_launch_at_startup();
    shell
        .main_window
        .invoke_desktop_settings_changed(!before, true, true, false);
    assert_eq!(shell.main_window.get_launch_at_startup(), before);
    apply(ClientUpdate::Event(
        ApplicationEvent::DesktopSettingsChanged(DesktopSettings {
            launch_at_startup: !before,
            ..snapshot.desktop_settings
        })
        .into(),
    ));
    assert_eq!(shell.main_window.get_launch_at_startup(), !before);

    let quick_command: GuiCommand = UiIntent::SubmitQuickAdd(QuickAddSubmission {
        title: "Keep this draft".into(),
        placement: TaskPlacement::Inbox,
    })
    .into();
    apply(ClientUpdate::CommandFinished {
        command: quick_command.clone(),
        outcome: CommandOutcome::Failed(quadrant_application::UserFacingError {
            message: "Save failed".into(),
        }),
    });
    assert_eq!(shell.quick_add.get_title_text(), "Keep this draft");
    assert!(shell.quick_add.window().is_visible());
    assert_eq!(shell.quick_add.get_error_message(), "Save failed");
    shell
        .quick_add
        .set_title_text("Next task drafted while saving".into());
    apply(ClientUpdate::CommandFinished {
        command: quick_command.clone(),
        outcome: CommandOutcome::Succeeded,
    });
    assert_eq!(
        shell.quick_add.get_title_text(),
        "Next task drafted while saving"
    );
    assert!(shell.quick_add.window().is_visible());
    shell.quick_add.set_title_text("Keep this draft".into());
    apply(ClientUpdate::CommandFinished {
        command: quick_command,
        outcome: CommandOutcome::Succeeded,
    });
    assert!(!shell.quick_add.window().is_visible());
    assert!(shell.quick_add.get_title_text().is_empty());
    // An explicitly launched GUI must show even when Agent startup is hidden.
    assert!(snapshot.desktop_settings.start_hidden);
    shell.main_window.hide().unwrap();
    assert!(!shell.main_window.window().is_visible());
    shell.run().unwrap();
}

fn render_review(window: &MinimalSoftwareWindow, path: &std::path::Path, width: u32, height: u32) {
    window.set_size(slint::PhysicalSize::new(width, height));
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    let mut pixels = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(width, height);
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(pixels.make_mut_slice(), width as usize);
    }));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels.as_bytes())
        .unwrap();
}
