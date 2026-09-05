// SPDX-License-Identifier: GPL-3.0-only
//! Real Slint components, queued UI dispatch and weak adapter lifetime assertions.

use super::*;
use quadrant_protocol::CommandOutcome;
use slint::platform::{
    Platform, WindowAdapter,
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
};
use std::{
    cell::Cell,
    collections::VecDeque,
    sync::atomic::{AtomicUsize, Ordering},
};

type UiCalls = Arc<Mutex<VecDeque<Box<dyn FnOnce() + Send>>>>;
type WindowHistory = Rc<RefCell<Vec<std::rc::Weak<MinimalSoftwareWindow>>>>;

struct Headless {
    windows: WindowHistory,
    quits: Arc<AtomicUsize>,
    calls: UiCalls,
    fail_next: Rc<Cell<bool>>,
}
struct EventProxy {
    quits: Arc<AtomicUsize>,
    calls: UiCalls,
}
impl slint::platform::EventLoopProxy for EventProxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        self.quits.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        self.calls.lock().unwrap().push_back(event);
        Ok(())
    }
}
impl Platform for Headless {
    fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
        Some(Box::new(EventProxy {
            quits: self.quits.clone(),
            calls: self.calls.clone(),
        }))
    }
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        if self.fail_next.replace(false) {
            return Err(slint::PlatformError::Other(
                "Injected window creation failure".into(),
            ));
        }
        let window = MinimalSoftwareWindow::new(RepaintBufferType::default());
        self.windows.borrow_mut().push(Rc::downgrade(&window));
        Ok(window)
    }
    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        assert!(
            self.windows
                .borrow()
                .iter()
                .filter_map(std::rc::Weak::upgrade)
                .any(|window| window.is_visible())
        );
        Ok(())
    }
}

struct Fixture {
    windows: WindowHistory,
    quits: Arc<AtomicUsize>,
    calls: UiCalls,
    fail_next: Rc<Cell<bool>>,
    commands: Rc<RefCell<Vec<GuiCommand>>>,
    snapshot: AppSnapshot,
}
impl Fixture {
    fn new() -> Self {
        let fixture = Self {
            windows: Rc::default(),
            quits: Arc::default(),
            calls: Arc::default(),
            fail_next: Rc::default(),
            commands: Rc::default(),
            snapshot: serde_json::from_str(include_str!(
                "../../quadrant-protocol/tests/fixtures/snapshot_v1.json"
            ))
            .unwrap(),
        };
        slint::platform::set_platform(Box::new(Headless {
            windows: fixture.windows.clone(),
            quits: fixture.quits.clone(),
            calls: fixture.calls.clone(),
            fail_next: fixture.fail_next.clone(),
        }))
        .unwrap();
        fixture
    }
    fn shell(&self) -> UiShell {
        let commands = self.commands.clone();
        UiShell::new(&self.snapshot, "test", move |command| {
            commands.borrow_mut().push(command);
            true
        })
        .unwrap()
    }
    fn pump(&self) {
        loop {
            let callback = self.calls.lock().unwrap().pop_front();
            let Some(callback) = callback else {
                break;
            };
            callback();
        }
    }
    fn apply(&self, shell: &UiShell, update: ClientUpdate) {
        shell.update_sink()(update);
        self.pump();
    }
    fn event(&self, shell: &UiShell, event: ServerEvent) {
        self.apply(shell, ClientUpdate::Event(event));
    }
    fn ready(&self, shell: &UiShell) {
        self.apply(
            shell,
            ClientUpdate::Connection {
                state: ConnectionState::Ready,
                message: String::new(),
            },
        );
    }
    fn finish(&self, shell: &UiShell, outcome: CommandOutcome) {
        self.apply(
            shell,
            ClientUpdate::CommandFinished {
                command: self.commands.borrow().last().unwrap().clone(),
                outcome,
            },
        );
    }
    fn live(&self) -> usize {
        self.windows
            .borrow()
            .iter()
            .filter(|window| window.strong_count() > 0)
            .count()
    }
}
fn quick(shell: &UiShell) -> QuickAddWindow {
    shell
        .transients
        .borrow()
        .quick_add
        .as_ref()
        .unwrap()
        .component
        .clone_strong()
}
fn editor(shell: &UiShell) -> TaskEditorWindow {
    shell
        .transients
        .borrow()
        .task_editor
        .as_ref()
        .unwrap()
        .component
        .clone_strong()
}
fn task() -> TaskEditorState {
    TaskEditorState {
        task_id: TaskId::generate(),
        title: "Edit me".into(),
        notes: String::new(),
        placement: TaskPlacement::Inbox,
        planned_on: String::new(),
        due_at: String::new(),
        due_time_zone: String::new(),
        reminder_at: String::new(),
        reminder_time_zone: String::new(),
        recurrence: RecurrenceChoice::None,
        custom_interval_days: String::new(),
    }
}
fn submit_editor(window: &TaskEditorWindow) {
    window.invoke_submitted(
        window.get_task_id(),
        window.get_title_text(),
        window.get_notes_text(),
        window.get_destination(),
        "".into(),
        "".into(),
        "".into(),
        "".into(),
        "".into(),
        0,
        "".into(),
    );
}
fn failed() -> CommandOutcome {
    CommandOutcome::Failed(quadrant_application::UserFacingError {
        message: "Save failed".into(),
    })
}

#[test]
fn lazy_windows_preserve_ipc_state_and_release_every_component() {
    let fixture = Fixture::new();
    let shell = fixture.shell();
    assert_eq!(fixture.live(), 1);
    // Production run() shows Main before queued IPC callbacks are processed.
    shell.main_window.show().unwrap();
    assert!(shell.transients.borrow().quick_add.is_none());
    assert!(shell.transients.borrow().task_editor.is_none());
    projections_without_auxiliary_windows(&fixture, &shell);
    quick_add_lifecycle(&fixture, &shell);
    editor_lifecycle(&fixture, &shell);
    creation_failures_and_teardown(&fixture, shell);
    assert_eq!(fixture.live(), 0);
    dedicated_capture(&fixture);
}

fn projections_without_auxiliary_windows(f: &Fixture, shell: &UiShell) {
    shell.main_window.set_current_route(5);
    // Queue multiple transport updates before a UI turn; each must apply in order.
    let sink = shell.update_sink();
    let mut snapshot = f.snapshot.clone();
    snapshot.focus.session = None;
    sink(ClientUpdate::Snapshot(Box::new(snapshot)));
    sink(ClientUpdate::Event(ServerEvent::ThemeChanged {
        theme_mode: ApplicationThemeMode::Dark,
        system_theme: SystemTheme::Dark,
    }));
    sink(ClientUpdate::Connection {
        state: ConnectionState::Ready,
        message: String::new(),
    });
    f.pump();
    assert_eq!(shell.main_window.get_current_route(), 5);
    assert_eq!(shell.main_window.get_focus_session_status(), -1);
    assert!(shell.main_window.get_agent_connected());
    assert_eq!(fixture_window_count(f), 1);
    shell
        .main_window
        .invoke_theme_selected(SlintThemeMode::Light);
    assert!(
        matches!(f.commands.borrow().last(), Some(GuiCommand::Application(intent)) if matches!(intent.as_ref(), UiIntent::SetTheme(ApplicationThemeMode::Light)))
    );
    f.finish(shell, CommandOutcome::Succeeded);
    f.ready(shell);
    let before = shell.main_window.get_launch_at_startup();
    shell
        .main_window
        .invoke_desktop_settings_changed(!before, true, true, false);
    assert_eq!(shell.main_window.get_launch_at_startup(), before);
    f.event(
        shell,
        ApplicationEvent::DesktopSettingsChanged(DesktopSettings {
            launch_at_startup: !before,
            ..f.snapshot.desktop_settings
        })
        .into(),
    );
    assert_eq!(shell.main_window.get_launch_at_startup(), !before);
    f.finish(shell, CommandOutcome::Succeeded);
    f.ready(shell);
}
fn fixture_window_count(f: &Fixture) -> usize {
    f.windows.borrow().len()
}

fn quick_add_lifecycle(f: &Fixture, shell: &UiShell) {
    f.event(shell, ServerEvent::OpenQuickAdd);
    let window = quick(shell);
    let original = window.as_weak();
    assert_eq!(f.live(), 2);
    assert!(window.get_can_submit());
    window.set_title_text("Keep this draft".into());
    window.set_destination(2);
    let created = fixture_window_count(f);
    f.event(shell, ServerEvent::OpenQuickAdd);
    assert_eq!(fixture_window_count(f), created);
    assert_eq!(window.get_title_text(), "Keep this draft");
    window.invoke_submitted(window.get_title_text(), 2);
    let sent = f.commands.borrow().len();
    window.invoke_submitted("duplicate".into(), 0);
    assert_eq!(f.commands.borrow().len(), sent);
    f.finish(shell, failed());
    assert_eq!(window.get_error_message(), "Save failed");
    assert_eq!(window.get_title_text(), "Keep this draft");
    f.ready(shell);
    window.invoke_submitted(window.get_title_text(), 2);
    window.set_title_text("Newer draft".into());
    f.finish(shell, CommandOutcome::Succeeded);
    assert!(shell.transients.borrow().quick_add.is_some());
    assert_eq!(window.get_title_text(), "Newer draft");
    f.ready(shell);
    window.invoke_submitted(window.get_title_text(), 2);
    f.finish(shell, CommandOutcome::Succeeded);
    assert!(shell.transients.borrow().quick_add.is_none());
    drop(window);
    assert!(original.upgrade().is_none());
    assert_eq!(f.live(), 1);
    quick_reopen_ignores_old_result(f, shell);
}

fn quick_reopen_ignores_old_result(f: &Fixture, shell: &UiShell) {
    f.ready(shell);
    f.event(shell, ServerEvent::OpenQuickAdd);
    let old = quick(shell);
    let weak = old.as_weak();
    old.set_title_text("Same title".into());
    old.invoke_submitted(old.get_title_text(), 0);
    old.invoke_cancelled();
    drop(old);
    assert!(weak.upgrade().is_none());
    f.event(shell, ServerEvent::OpenQuickAdd);
    let current = quick(shell);
    current.set_title_text("Same title".into());
    assert!(!current.get_can_submit());
    f.finish(shell, CommandOutcome::Succeeded);
    assert_eq!(current.get_title_text(), "Same title");
    assert!(current.window().is_visible());
    f.apply(
        shell,
        ClientUpdate::Connection {
            state: ConnectionState::Reconnecting,
            message: "Connection lost; operation outcome unknown.".into(),
        },
    );
    assert!(!current.get_can_submit());
    if let Some(directory) = std::env::var_os("QUADRANT_IPC_RENDER_DIR") {
        shell.main_window.show().unwrap();
        render_review(
            &f.windows.borrow()[0].upgrade().unwrap(),
            &std::path::PathBuf::from(&directory).join("disconnected.png"),
            900,
            640,
        );
        let adapter = f.windows.borrow().last().unwrap().upgrade().unwrap();
        render_review(
            &adapter,
            &std::path::PathBuf::from(directory).join("quick-add-disconnected.png"),
            520,
            252,
        );
    }
    f.apply(shell, ClientUpdate::Snapshot(Box::new(f.snapshot.clone())));
    assert_eq!(current.get_title_text(), "Same title");
    assert_eq!(shell.main_window.get_current_route(), 5);
    shell.main_window.hide().unwrap();
    f.apply(
        shell,
        ClientUpdate::Connection {
            state: ConnectionState::Unavailable,
            message: "Reconnect exhausted".into(),
        },
    );
    assert!(shell.main_window.window().is_visible());
    let weak = current.as_weak();
    current
        .window()
        .dispatch_event(slint::platform::WindowEvent::CloseRequested);
    drop(current);
    assert!(weak.upgrade().is_none());
    assert_eq!(f.live(), 1);
    f.ready(shell);
}

fn editor_lifecycle(f: &Fixture, shell: &UiShell) {
    let state = task();
    f.event(
        shell,
        ApplicationEvent::TaskEditorLoaded(state.clone()).into(),
    );
    let window = editor(shell);
    let weak = window.as_weak();
    assert_eq!(f.live(), 2);
    window.set_notes_text("Unsaved notes".into());
    let created = fixture_window_count(f);
    f.event(
        shell,
        ApplicationEvent::TaskEditorLoaded(state.clone()).into(),
    );
    assert_eq!(fixture_window_count(f), created);
    assert_eq!(window.get_notes_text(), "Unsaved notes");
    submit_editor(&window);
    f.event(
        shell,
        ApplicationEvent::TaskEditorValidationFailed {
            field: TaskEditorField::Title,
            message: "Invalid title".into(),
        }
        .into(),
    );
    assert_eq!(window.get_title_error(), "Invalid title");
    f.finish(shell, failed());
    assert_eq!(window.get_error_message(), "Save failed");
    f.ready(shell);
    submit_editor(&window);
    f.event(shell, ApplicationEvent::TaskEditorSaved.into());
    assert!(window.window().is_visible()); // Wait for the correlated result.
    window.set_notes_text("Edited while waiting".into());
    f.finish(shell, CommandOutcome::Succeeded);
    assert!(shell.transients.borrow().task_editor.is_some());
    f.ready(shell);
    submit_editor(&window);
    f.finish(shell, CommandOutcome::Succeeded);
    assert!(shell.transients.borrow().task_editor.is_none());
    drop(window);
    assert!(weak.upgrade().is_none());
    assert_eq!(f.live(), 1);
    editor_reopen_ignores_old_result(f, shell, &state);
}

fn editor_reopen_ignores_old_result(f: &Fixture, shell: &UiShell, state: &TaskEditorState) {
    f.ready(shell);
    f.event(
        shell,
        ApplicationEvent::TaskEditorLoaded(state.clone()).into(),
    );
    let old = editor(shell);
    let weak = old.as_weak();
    submit_editor(&old);
    old.window()
        .dispatch_event(slint::platform::WindowEvent::CloseRequested);
    drop(old);
    assert!(weak.upgrade().is_none());
    f.event(
        shell,
        ApplicationEvent::TaskEditorLoaded(state.clone()).into(),
    );
    let current = editor(shell);
    current.set_notes_text("New window".into());
    f.event(shell, ApplicationEvent::TaskEditorSaved.into());
    f.event(
        shell,
        ApplicationEvent::TaskEditorValidationFailed {
            field: TaskEditorField::Title,
            message: "Old validation".into(),
        }
        .into(),
    );
    f.finish(shell, failed());
    assert!(current.window().is_visible());
    assert!(current.get_title_error().is_empty());
    assert!(current.get_error_message().is_empty());
    current.invoke_cancelled();
    let weak = current.as_weak();
    drop(current);
    assert!(weak.upgrade().is_none());
    assert_eq!(f.live(), 1);
    // Late editor pushes never create a hidden replacement.
    f.event(shell, ApplicationEvent::TaskEditorSaved.into());
    assert_eq!(f.live(), 1);
    f.ready(shell);
}

fn creation_failures_and_teardown(f: &Fixture, shell: UiShell) {
    f.fail_next.set(true);
    f.event(&shell, ServerEvent::OpenQuickAdd);
    assert!(shell.transients.borrow().quick_add.is_none());
    assert!(
        shell
            .main_window
            .get_toast_message()
            .contains("could not be created")
    );
    f.event(&shell, ServerEvent::OpenQuickAdd);
    assert_eq!(f.live(), 2);
    f.fail_next.set(true);
    f.event(&shell, ApplicationEvent::TaskEditorLoaded(task()).into());
    assert!(shell.transients.borrow().task_editor.is_none());
    f.event(&shell, ApplicationEvent::TaskEditorLoaded(task()).into());
    assert_eq!(f.live(), 3);
    let created = fixture_window_count(f);
    f.event(
        &shell,
        ServerEvent::ThemeChanged {
            theme_mode: ApplicationThemeMode::Dark,
            system_theme: SystemTheme::Dark,
        },
    );
    assert_eq!(fixture_window_count(f), created);
    shell.main_window.show().unwrap();
    shell.main_window.set_tray_supported(true);
    shell.main_window.set_close_to_tray(true);
    shell.main_window.invoke_window_minimize();
    assert!(shell.main_window.window().is_visible());
    shell.main_window.invoke_window_close();
    assert_eq!(f.quits.load(Ordering::SeqCst), 1);
    shell
        .main_window
        .window()
        .dispatch_event(slint::platform::WindowEvent::CloseRequested);
    assert_eq!(f.quits.load(Ordering::SeqCst), 2);
    shell.main_window.set_close_to_tray(false);
    shell.main_window.invoke_window_close();
    assert!(matches!(
        f.commands.borrow().last(),
        Some(GuiCommand::ExitApplication)
    ));
    let sent = f.commands.borrow().len();
    shell.main_window.invoke_window_close();
    assert_eq!(f.commands.borrow().len(), sent);
    f.event(&shell, ServerEvent::ExitGui);
    assert_eq!(f.quits.load(Ordering::SeqCst), 3);
    let main_weak = shell.main_window.as_weak();
    let quick_weak = quick(&shell).as_weak();
    let editor_weak = editor(&shell).as_weak();
    let late_sink = shell.update_sink();
    assert!(f.snapshot.desktop_settings.start_hidden);
    shell.main_window.hide().unwrap();
    shell.run().unwrap();
    assert!(main_weak.upgrade().is_none());
    assert!(quick_weak.upgrade().is_none());
    assert!(editor_weak.upgrade().is_none());
    late_sink(ClientUpdate::Event(ServerEvent::OpenQuickAdd));
    f.pump();
    assert_eq!(f.live(), 0);
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

fn dedicated_capture(f: &Fixture) {
    use quadrant_protocol::GuiLaunchMode;
    f.fail_next.set(true);
    assert!(GuiShell::new(GuiLaunchMode::QuickAdd, &f.snapshot, "test", |_| true).is_err());
    assert_eq!(f.live(), 0);
    let created = fixture_window_count(f);
    let host = capture_host(f);
    assert_eq!(fixture_window_count(f), created + 1);
    assert_eq!(f.live(), 1); // No Main, editor or auxiliary component was constructed.
    let GuiShell::QuickAdd(shell) = &host else {
        panic!("capture host required");
    };
    let window = shell.window.clone_strong();
    window.show().unwrap();
    let weak = window.as_weak();
    let sink = host.update_sink();
    let apply = |update| {
        sink(update);
        f.pump();
    };
    let ready = || {
        apply(ClientUpdate::Connection {
            state: ConnectionState::Ready,
            message: String::new(),
        });
    };
    assert!(!window.get_can_submit());
    let sent = f.commands.borrow().len();
    window.invoke_submitted("Offline".into(), 0);
    assert_eq!(f.commands.borrow().len(), sent);
    ready();
    assert!(window.get_error_message().is_empty());
    window.invoke_submitted("  ".into(), 0);
    assert_eq!(f.commands.borrow().len(), sent);
    window.set_title_text("Capture draft".into());
    sink(ClientUpdate::Snapshot(Box::new(f.snapshot.clone())));
    sink(ClientUpdate::Event(ServerEvent::ThemeChanged {
        theme_mode: ApplicationThemeMode::Dark,
        system_theme: SystemTheme::Dark,
    }));
    sink(ClientUpdate::Event(ServerEvent::OpenQuickAdd));
    // Even irrelevant Main/editor pushes never construct another component.
    sink(ClientUpdate::Event(ServerEvent::ActivateMainWindow));
    sink(ClientUpdate::Event(
        ApplicationEvent::TaskEditorLoaded(task()).into(),
    ));
    f.pump();
    assert_eq!(window.get_title_text(), "Capture draft");
    assert_eq!(fixture_window_count(f), created + 1);
    capture_results(f, &window, &sink);
    drop(window);
    host.run().unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(f.live(), 0);
    sink(ClientUpdate::Event(ServerEvent::OpenQuickAdd));
    f.pump();
    assert_eq!(f.live(), 0);

    capture_closes(f);
}

fn capture_closes(f: &Fixture) {
    for action in 0..3 {
        let host = capture_host(f);
        let GuiShell::QuickAdd(shell) = &host else {
            unreachable!()
        };
        let window = shell.window.clone_strong();
        window.show().unwrap();
        let weak = window.as_weak();
        let sink = host.update_sink();
        sink(ClientUpdate::Connection {
            state: ConnectionState::Ready,
            message: String::new(),
        });
        f.pump();
        window.set_title_text("Pending close".into());
        window.invoke_submitted(window.get_title_text(), 0);
        let quits = f.quits.load(Ordering::SeqCst);
        match action {
            0 => window.invoke_cancelled(),
            1 => window
                .window()
                .dispatch_event(slint::platform::WindowEvent::CloseRequested),
            _ => {
                sink(ClientUpdate::Event(ServerEvent::AgentShuttingDown));
                f.pump();
            }
        }
        assert_eq!(f.quits.load(Ordering::SeqCst), quits + 1);
        sink(ClientUpdate::CommandFinished {
            command: f.commands.borrow().last().unwrap().clone(),
            outcome: failed(),
        });
        f.pump();
        drop(window);
        drop(host);
        assert!(weak.upgrade().is_none());
        assert_eq!(f.live(), 0);
    }
}

fn capture_results(f: &Fixture, window: &QuickAddWindow, sink: &ClientUpdateSink) {
    let sent = f.commands.borrow().len();
    let apply = |update| {
        sink(update);
        f.pump();
    };
    let ready = || {
        apply(ClientUpdate::Connection {
            state: ConnectionState::Ready,
            message: String::new(),
        });
    };
    window.invoke_submitted(window.get_title_text(), 0);
    let command = f.commands.borrow().last().unwrap().clone();
    assert!(!window.get_can_submit());
    window.invoke_submitted(window.get_title_text(), 0);
    assert_eq!(f.commands.borrow().len(), sent + 1);
    apply(ClientUpdate::CommandFinished {
        command: command.clone(),
        outcome: failed(),
    });
    ready();
    assert_eq!(window.get_error_message(), "Save failed");
    assert_eq!(window.get_title_text(), "Capture draft");
    let quits = f.quits.load(Ordering::SeqCst);
    window.invoke_submitted(window.get_title_text(), 0);
    window.set_title_text("Newer draft".into());
    apply(ClientUpdate::CommandFinished {
        command,
        outcome: CommandOutcome::Succeeded,
    });
    ready();
    assert_eq!(f.quits.load(Ordering::SeqCst), quits);
    assert_eq!(window.get_title_text(), "Newer draft");
    window.invoke_submitted(window.get_title_text(), 0);
    let uncertain = f.commands.borrow().last().unwrap().clone();
    apply(ClientUpdate::Connection {
        state: ConnectionState::Reconnecting,
        message: "Connection lost; outcome unknown.".into(),
    });
    assert!(!window.get_can_submit());
    let sent = f.commands.borrow().len();
    apply(ClientUpdate::Snapshot(Box::new(f.snapshot.clone())));
    ready();
    apply(ClientUpdate::CommandFinished {
        command: uncertain,
        outcome: CommandOutcome::Succeeded,
    });
    assert_eq!(f.commands.borrow().len(), sent); // No replay, no late uncertain close.
    assert_eq!(f.quits.load(Ordering::SeqCst), quits);
    assert_eq!(window.get_title_text(), "Newer draft");
    if let Some(directory) = std::env::var_os("QUADRANT_IPC_RENDER_DIR") {
        render_review(
            &f.windows.borrow().last().unwrap().upgrade().unwrap(),
            &std::path::PathBuf::from(directory).join("standalone-capture.png"),
            520,
            252,
        );
    }
    window.invoke_submitted(window.get_title_text(), 0);
    apply(ClientUpdate::CommandFinished {
        command: f.commands.borrow().last().unwrap().clone(),
        outcome: CommandOutcome::Succeeded,
    });
    assert_eq!(f.quits.load(Ordering::SeqCst), quits + 1);
    let sent = f.commands.borrow().len();
    ready(); // Queued Ready/activation after success cannot reopen the closing host.
    apply(ClientUpdate::Event(ServerEvent::OpenQuickAdd));
    window.invoke_submitted("Must not send".into(), 0);
    assert_eq!(f.commands.borrow().len(), sent);
}

fn capture_host(f: &Fixture) -> GuiShell {
    let commands = f.commands.clone();
    GuiShell::new(
        quadrant_protocol::GuiLaunchMode::QuickAdd,
        &f.snapshot,
        "test",
        move |command| {
            commands.borrow_mut().push(command);
            true
        },
    )
    .unwrap()
}
