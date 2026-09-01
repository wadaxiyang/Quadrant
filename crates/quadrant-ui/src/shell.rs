//! Main/Quick Add window construction and typed callback binding.

use std::rc::Rc;

use quadrant_application::{
    NavigationRoute, Quadrant, QuickAddSubmission, SystemTheme, TaskPlacement,
    ThemeMode as ApplicationThemeMode, UiIntent,
};
use slint::{ComponentHandle, PhysicalPosition, SharedString};

use crate::{MainWindow, QuickAddWindow, ThemeMode as SlintThemeMode, ToastKind};

/// Initial state supplied by the composition root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiShellConfig {
    /// User-selected theme behavior.
    pub theme_mode: ApplicationThemeMode,
    /// Current normalized platform appearance.
    pub system_theme: SystemTheme,
}

/// Constructs both M1 windows, binds typed UI intents, and runs the Slint event loop.
///
/// # Errors
///
/// Returns a platform error when a window or the event loop cannot be created.
pub fn run(
    config: UiShellConfig,
    on_intent: impl Fn(UiIntent) + 'static,
) -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;
    let quick_add = QuickAddWindow::new()?;
    let intent_handler: Rc<dyn Fn(UiIntent)> = Rc::new(on_intent);

    initialize_theme(&main_window, &quick_add, config);
    bind_main_window(&main_window, &quick_add, &intent_handler);
    bind_quick_add(&main_window, &quick_add, intent_handler);

    main_window.run()
}

fn initialize_theme(main_window: &MainWindow, quick_add: &QuickAddWindow, config: UiShellConfig) {
    let mode = to_slint_theme_mode(config.theme_mode);
    let system_dark = config.system_theme == SystemTheme::Dark;
    main_window.invoke_apply_theme(mode, system_dark);
    quick_add.invoke_apply_theme(mode, system_dark);
}

fn bind_main_window(
    main_window: &MainWindow,
    quick_add: &QuickAddWindow,
    intent_handler: &Rc<dyn Fn(UiIntent)>,
) {
    let navigation_handler = Rc::clone(intent_handler);
    main_window.on_navigation_requested(move |index| {
        if let Some(route) = NavigationRoute::from_index(index) {
            navigation_handler(UiIntent::Navigate(route));
        }
    });

    let quick_add_weak = quick_add.as_weak();
    let main_weak = main_window.as_weak();
    let open_handler = Rc::clone(intent_handler);
    main_window.on_quick_add_requested(move || {
        open_handler(UiIntent::OpenQuickAdd);
        if let Some(window) = quick_add_weak.upgrade() {
            window.set_title_text(SharedString::default());
            window.set_destination(0);
            window.set_error_message(SharedString::default());
            if window.show().is_err()
                && let Some(main) = main_weak.upgrade()
            {
                main.invoke_show_toast(
                    SharedString::from("Quick Add could not be opened."),
                    ToastKind::Error,
                );
            }
        }
    });

    let theme_quick_add = quick_add.as_weak();
    let theme_handler = Rc::clone(intent_handler);
    main_window.on_theme_selected(move |mode| {
        let application_mode = to_application_theme_mode(mode);
        theme_handler(UiIntent::SetTheme(application_mode));
        if let Some(window) = theme_quick_add.upgrade() {
            window.invoke_set_theme_mode(mode);
        }
    });

    let minimize_weak = main_window.as_weak();
    main_window.on_window_minimize(move || {
        if let Some(window) = minimize_weak.upgrade() {
            window.window().set_minimized(true);
        }
    });

    let maximize_weak = main_window.as_weak();
    main_window.on_window_maximize(move || {
        if let Some(window) = maximize_weak.upgrade() {
            let maximized = window.window().is_maximized();
            window.window().set_maximized(!maximized);
        }
    });

    let drag_weak = main_window.as_weak();
    main_window.on_window_drag_delta(move |dx, dy| {
        if let Some(window) = drag_weak.upgrade() {
            move_window_by(window.window(), dx, dy);
        }
    });

    main_window.on_window_close(|| {
        drop(slint::quit_event_loop());
    });

    main_window.window().on_close_requested(|| {
        drop(slint::quit_event_loop());
        slint::CloseRequestResponse::HideWindow
    });
}

fn bind_quick_add(
    main_window: &MainWindow,
    quick_add: &QuickAddWindow,
    intent_handler: Rc<dyn Fn(UiIntent)>,
) {
    let cancel_weak = quick_add.as_weak();
    quick_add.on_cancelled(move || {
        if let Some(window) = cancel_weak.upgrade() {
            drop(window.hide());
        }
    });

    let submit_weak = quick_add.as_weak();
    let main_weak = main_window.as_weak();
    quick_add.on_submitted(move |title, destination| {
        let Some(window) = submit_weak.upgrade() else {
            return;
        };
        let trimmed = title.trim();
        if trimmed.is_empty() {
            window.set_error_message(SharedString::from("Enter a task title."));
            return;
        }
        let Some(placement) = placement_from_destination(destination) else {
            window.set_error_message(SharedString::from("Choose Inbox or Q1–Q4."));
            return;
        };

        intent_handler(UiIntent::SubmitQuickAdd(QuickAddSubmission {
            title: trimmed.to_owned(),
            placement,
        }));
        drop(window.hide());
        if let Some(main) = main_weak.upgrade() {
            main.invoke_show_toast(
                SharedString::from("Capture intent emitted. Persistence arrives in M2."),
                ToastKind::Info,
            );
        }
    });

    let close_weak = quick_add.as_weak();
    quick_add.window().on_close_requested(move || {
        if let Some(window) = close_weak.upgrade() {
            drop(window.hide());
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
}

fn to_slint_theme_mode(mode: ApplicationThemeMode) -> SlintThemeMode {
    match mode {
        ApplicationThemeMode::System => SlintThemeMode::System,
        ApplicationThemeMode::Light => SlintThemeMode::Light,
        ApplicationThemeMode::Dark => SlintThemeMode::Dark,
    }
}

fn to_application_theme_mode(mode: SlintThemeMode) -> ApplicationThemeMode {
    match mode {
        SlintThemeMode::System => ApplicationThemeMode::System,
        SlintThemeMode::Light => ApplicationThemeMode::Light,
        SlintThemeMode::Dark => ApplicationThemeMode::Dark,
    }
}

fn placement_from_destination(destination: i32) -> Option<TaskPlacement> {
    match destination {
        0 => Some(TaskPlacement::Inbox),
        1 => Some(TaskPlacement::Quadrant(Quadrant::Q1)),
        2 => Some(TaskPlacement::Quadrant(Quadrant::Q2)),
        3 => Some(TaskPlacement::Quadrant(Quadrant::Q3)),
        4 => Some(TaskPlacement::Quadrant(Quadrant::Q4)),
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation)] // Window movement is constrained to physical i32 coordinates.
fn move_window_by(window: &slint::Window, dx: f32, dy: f32) {
    let position = window.position();
    window.set_position(PhysicalPosition::new(
        position.x.saturating_add(dx.round() as i32),
        position.y.saturating_add(dy.round() as i32),
    ));
}

#[cfg(test)]
mod tests {
    use super::placement_from_destination;
    use quadrant_application::{Quadrant, TaskPlacement};

    #[test]
    fn quick_add_destinations_map_to_typed_placement() {
        assert_eq!(placement_from_destination(0), Some(TaskPlacement::Inbox));
        assert_eq!(
            placement_from_destination(4),
            Some(TaskPlacement::Quadrant(Quadrant::Q4))
        );
        assert_eq!(placement_from_destination(5), None);
    }
}
