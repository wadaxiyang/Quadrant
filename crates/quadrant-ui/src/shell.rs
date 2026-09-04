//! Main/Quick Add window construction and typed callback binding.

use std::{
    rc::Rc,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use jiff::{Timestamp, civil::DateTime, tz::TimeZone};
use quadrant_application::{
    ApplicationEvent, CompletedViewState, DesktopEvent, DesktopSettings, FocusMode, FocusSession,
    FocusStartRequest, FocusStatus, FocusViewState, MaintenanceState, NavigationRoute,
    PomodoroKind, PomodoroSettings, Quadrant, QuadrantsViewState, QuickAddSubmission,
    RecurrenceChoice, ReorderDirection, ReviewRange, ReviewViewState, SystemTheme, TaskEditorField,
    TaskEditorState, TaskEditorSubmission, TaskId, TaskPlacement,
    ThemeMode as ApplicationThemeMode, TodayViewState, UiIntent, UpdateViewState, UtcTimestamp,
    WindowCloseBehavior, WindowMinimizeBehavior,
};
use slint::{ComponentHandle, ModelRc, PhysicalPosition, SharedString, TimerMode, VecModel};

use crate::{
    CompletedTaskRow, Date as SlintDate, FocusTaskRow, InboxItem, MainWindow, QuickAddWindow,
    ReviewActivityRow, ReviewQuadrantRow, ReviewRecentRow, TaskEditorWindow, TaskRow,
    ThemeMode as SlintThemeMode, Time as SlintTime, ToastKind, TodayTaskRow,
};

/// Initial state supplied by the composition root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiShellConfig {
    /// Canonical Cargo package version shown by every UI surface.
    pub application_version: String,
    /// Static distribution/update ownership state.
    pub updates: UpdateViewState,
    /// User-selected theme behavior.
    pub theme_mode: ApplicationThemeMode,
    /// Current normalized platform appearance.
    pub system_theme: SystemTheme,
    /// Initial repository-backed active task projection.
    pub quadrants: QuadrantsViewState,
    /// Initial repository-backed Today projection.
    pub today: TodayViewState,
    /// Initial repository-backed Focus projection.
    pub focus: FocusViewState,
    /// Initial repository-backed Review projection.
    pub review: ReviewViewState,
    /// Initial bounded Completed projection.
    pub completed: CompletedViewState,
    /// Initial backup/restore projection.
    pub maintenance: MaintenanceState,
    /// Persisted desktop lifecycle policy.
    pub desktop_settings: DesktopSettings,
}

/// Platform capability snapshot normalized by the composition root.
#[allow(clippy::struct_excessive_bools)] // Independent capabilities, not one lifecycle state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UiPlatformCapabilities {
    /// Whether login startup can be configured.
    pub autostart: bool,
    /// Whether hiding windows remains recoverable through a tray/status item.
    pub tray: bool,
    /// Whether the global Quick Add shortcut registered successfully.
    pub global_hotkey: bool,
    /// Whether native notifications are supported.
    pub native_notifications: bool,
    /// Whether activation forwarding is available.
    pub single_instance: bool,
}

/// Thread-safe sink used by application-runtime work to enqueue typed UI events.
pub type ApplicationEventSink = Arc<dyn Fn(ApplicationEvent) + Send + Sync>;

/// Thread-safe sink used by hotkeys, tray, and redirected launches.
pub type DesktopEventSink = Arc<dyn Fn(DesktopEvent) + Send + Sync>;

/// Constructed Slint shell kept on the UI thread.
pub struct UiShell {
    main_window: MainWindow,
    quick_add: QuickAddWindow,
    task_editor: TaskEditorWindow,
    focus_session: Arc<Mutex<Option<FocusSession>>>,
    _focus_timer: slint::Timer,
    initial_desktop_settings: DesktopSettings,
}

impl UiShell {
    /// Constructs both windows, installs initial state, and binds typed intents.
    ///
    /// # Errors
    ///
    /// Returns a platform error when a window cannot be created.
    pub fn new(
        config: &UiShellConfig,
        on_intent: impl Fn(UiIntent) + 'static,
    ) -> Result<Self, slint::PlatformError> {
        let main_window = MainWindow::new()?;
        let quick_add = QuickAddWindow::new()?;
        let task_editor = TaskEditorWindow::new()?;
        let focus_session = Arc::new(Mutex::new(config.focus.session.clone()));
        let intent_handler: Rc<dyn Fn(UiIntent)> = Rc::new(on_intent);

        initialize_theme(&main_window, &quick_add, &task_editor, config);
        apply_desktop_settings(&main_window, config.desktop_settings);
        apply_quadrants_state(&main_window, &config.quadrants);
        apply_today_state(&main_window, &config.today);
        apply_focus_state(&main_window, &config.focus, &focus_session);
        apply_review_state(&main_window, &config.review);
        apply_completed_state(&main_window, &config.completed);
        apply_maintenance_state(&main_window, &config.maintenance);
        main_window
            .set_application_version(SharedString::from(config.application_version.as_str()));
        main_window.set_update_description(SharedString::from(config.updates.description.as_str()));
        main_window.set_can_open_releases(config.updates.can_open_releases);
        bind_main_window(&main_window, &quick_add, &task_editor, &intent_handler);
        bind_quick_add(&quick_add, Rc::clone(&intent_handler));
        bind_task_editor(&task_editor, intent_handler);

        let focus_timer = start_focus_projection_timer(&main_window, Arc::clone(&focus_session));
        Ok(Self {
            main_window,
            quick_add,
            task_editor,
            focus_session,
            _focus_timer: focus_timer,
            initial_desktop_settings: config.desktop_settings,
        })
    }

    /// Creates a cross-thread event sink that wakes the Slint event loop without polling.
    #[must_use]
    pub fn event_sink(&self) -> ApplicationEventSink {
        let main_weak = self.main_window.as_weak();
        let editor_weak = self.task_editor.as_weak();
        let focus_session = Arc::clone(&self.focus_session);
        Arc::new(move |event| {
            let editor_weak = editor_weak.clone();
            let focus_session = Arc::clone(&focus_session);
            drop(main_weak.upgrade_in_event_loop(move |main| {
                apply_application_event(&main, &editor_weak, &focus_session, event);
            }));
        })
    }

    /// Creates a desktop-shell sink that marshals platform events onto the Slint event loop.
    #[must_use]
    pub fn desktop_event_sink(&self) -> DesktopEventSink {
        let main_weak = self.main_window.as_weak();
        let quick_add_weak = self.quick_add.as_weak();
        Arc::new(move |event| {
            let quick_add_weak = quick_add_weak.clone();
            drop(main_weak.upgrade_in_event_loop(move |main| match event {
                DesktopEvent::ShowMainWindow => {
                    main.window().set_minimized(false);
                    drop(main.show());
                }
                DesktopEvent::OpenQuickAdd => {
                    if let Some(quick_add) = quick_add_weak.upgrade() {
                        show_quick_add(&quick_add, &main);
                    }
                }
                DesktopEvent::ExitRequested => drop(slint::quit_event_loop()),
            }));
        })
    }

    /// Applies the capabilities that initialized successfully before the event loop starts.
    pub fn set_platform_capabilities(&self, capabilities: UiPlatformCapabilities) {
        self.main_window
            .set_autostart_supported(capabilities.autostart);
        self.main_window.set_tray_supported(capabilities.tray);
        self.main_window
            .set_global_hotkey_available(capabilities.global_hotkey);
        self.main_window
            .set_native_notifications_available(capabilities.native_notifications);
        self.main_window
            .set_single_instance_available(capabilities.single_instance);
    }

    /// Runs the Slint event loop until normal application shutdown.
    ///
    /// # Errors
    ///
    /// Returns an event-loop platform error.
    pub fn run(self, background_requested: bool) -> Result<(), slint::PlatformError> {
        let start_hidden = should_hide_at_startup(
            self.main_window.get_tray_supported(),
            self.initial_desktop_settings.start_hidden,
            background_requested,
        );
        if !start_hidden {
            self.main_window.show()?;
        }
        // Tray/background operation must not use Slint's default
        // QuitOnLastWindowClosed behavior: hiding the final visible window is
        // an expected steady state, not application shutdown.
        let result = slint::run_event_loop_until_quit();
        drop(self.quick_add.hide());
        drop(self.task_editor.hide());
        result
    }
}

fn initialize_theme(
    main_window: &MainWindow,
    quick_add: &QuickAddWindow,
    task_editor: &TaskEditorWindow,
    config: &UiShellConfig,
) {
    #[cfg(target_os = "windows")]
    let ui_font_family = SharedString::from("Segoe UI Variable Text");
    #[cfg(not(target_os = "windows"))]
    let ui_font_family = SharedString::default();

    main_window.set_ui_font_family(ui_font_family.clone());
    quick_add.set_ui_font_family(ui_font_family.clone());
    task_editor.set_ui_font_family(ui_font_family);

    let mode = to_slint_theme_mode(config.theme_mode);
    let system_dark = config.system_theme == SystemTheme::Dark;
    main_window.invoke_apply_theme(mode, system_dark);
    quick_add.invoke_apply_theme(mode, system_dark);
    task_editor.invoke_apply_theme(mode, system_dark);
}

fn bind_main_window(
    main_window: &MainWindow,
    quick_add: &QuickAddWindow,
    task_editor: &TaskEditorWindow,
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
        if let Some(window) = quick_add_weak.upgrade()
            && let Some(main) = main_weak.upgrade()
        {
            show_quick_add(&window, &main);
        }
    });

    let theme_quick_add = quick_add.as_weak();
    let theme_editor = task_editor.as_weak();
    let theme_handler = Rc::clone(intent_handler);
    main_window.on_theme_selected(move |mode| {
        let application_mode = to_application_theme_mode(mode);
        theme_handler(UiIntent::SetTheme(application_mode));
        if let Some(window) = theme_quick_add.upgrade() {
            window.invoke_set_theme_mode(mode);
        }
        if let Some(window) = theme_editor.upgrade() {
            window.invoke_set_theme_mode(mode);
        }
    });

    let desktop_settings_handler = Rc::clone(intent_handler);
    main_window.on_desktop_settings_changed(
        move |launch_at_startup, start_hidden, close_to_tray, minimize_to_tray| {
            desktop_settings_handler(UiIntent::SetDesktopSettings(DesktopSettings {
                launch_at_startup,
                start_hidden,
                close_behavior: if close_to_tray {
                    WindowCloseBehavior::HideToTray
                } else {
                    WindowCloseBehavior::Quit
                },
                minimize_behavior: if minimize_to_tray {
                    WindowMinimizeBehavior::HideToTray
                } else {
                    WindowMinimizeBehavior::Taskbar
                },
            }));
        },
    );

    bind_task_actions(main_window, intent_handler);
    bind_focus_actions(main_window, intent_handler);
    bind_history_actions(main_window, intent_handler);
    bind_maintenance_actions(main_window, intent_handler);
    bind_main_window_controls(main_window);
}

fn bind_maintenance_actions(main_window: &MainWindow, intent_handler: &Rc<dyn Fn(UiIntent)>) {
    let create_handler = Rc::clone(intent_handler);
    main_window.on_create_backup_requested(move || create_handler(UiIntent::CreateBackup));

    let open_handler = Rc::clone(intent_handler);
    main_window.on_open_backup_directory_requested(move || {
        open_handler(UiIntent::OpenBackupDirectory);
    });

    let restore_handler = Rc::clone(intent_handler);
    main_window.on_backup_restore_confirmed(move || {
        restore_handler(UiIntent::StageLatestRestore);
    });

    let release_handler = Rc::clone(intent_handler);
    main_window.on_open_release_page_requested(move || {
        release_handler(UiIntent::OpenReleasePage);
    });
}

fn bind_history_actions(main_window: &MainWindow, intent_handler: &Rc<dyn Fn(UiIntent)>) {
    let range_handler = Rc::clone(intent_handler);
    let range_main = main_window.as_weak();
    main_window.on_review_range_selected(move |index| {
        if let Some(range) = ReviewRange::from_index(index) {
            range_handler(UiIntent::SetReviewRange(range));
        } else {
            show_invalid_history_action(&range_main);
        }
    });

    let reopen_handler = Rc::clone(intent_handler);
    let reopen_main = main_window.as_weak();
    main_window.on_completed_reopen_requested(move |id| match TaskId::from_str(id.as_str()) {
        Ok(task_id) => reopen_handler(UiIntent::ReopenTask(task_id)),
        Err(_) => show_invalid_history_action(&reopen_main),
    });

    let load_handler = Rc::clone(intent_handler);
    main_window.on_completed_load_more_requested(move || {
        load_handler(UiIntent::LoadMoreCompleted);
    });
}

fn show_invalid_history_action(main: &slint::Weak<MainWindow>) {
    if let Some(main) = main.upgrade() {
        main.invoke_show_toast(
            SharedString::from("That history action is no longer valid."),
            ToastKind::Error,
        );
    }
}

fn bind_focus_actions(main_window: &MainWindow, intent_handler: &Rc<dyn Fn(UiIntent)>) {
    let start_handler = Rc::clone(intent_handler);
    let start_main = main_window.as_weak();
    main_window.on_focus_start_requested(move |mode, kind, task_id| {
        let mode = match mode {
            0 => Some(FocusMode::Stopwatch),
            1 => Some(FocusMode::Pomodoro),
            _ => None,
        };
        let pomodoro_kind = match (mode, kind) {
            (Some(FocusMode::Stopwatch), _) => Some(None),
            (Some(FocusMode::Pomodoro), 0) => Some(Some(PomodoroKind::Focus)),
            (Some(FocusMode::Pomodoro), 1) => Some(Some(PomodoroKind::ShortBreak)),
            (Some(FocusMode::Pomodoro), 2) => Some(Some(PomodoroKind::LongBreak)),
            _ => None,
        };
        let task_id = if task_id.is_empty() {
            Ok(None)
        } else {
            TaskId::from_str(task_id.as_str()).map(Some)
        };
        match (mode, pomodoro_kind, task_id) {
            (Some(mode), Some(pomodoro_kind), Ok(task_id)) => {
                start_handler(UiIntent::StartFocus(FocusStartRequest {
                    mode,
                    pomodoro_kind,
                    task_id,
                }));
            }
            _ => show_invalid_focus_action(&start_main),
        }
    });

    let pause_handler = Rc::clone(intent_handler);
    main_window.on_focus_pause_requested(move || pause_handler(UiIntent::PauseFocus));
    let resume_handler = Rc::clone(intent_handler);
    main_window.on_focus_resume_requested(move || resume_handler(UiIntent::ResumeFocus));
    let finish_handler = Rc::clone(intent_handler);
    main_window.on_focus_finish_requested(move || finish_handler(UiIntent::FinishFocus));
    let cancel_handler = Rc::clone(intent_handler);
    main_window.on_focus_cancel_requested(move || cancel_handler(UiIntent::CancelFocus));

    let settings_handler = Rc::clone(intent_handler);
    let settings_main = main_window.as_weak();
    main_window.on_focus_settings_changed(
        move |focus, short_break, long_break, interval, auto_break, auto_focus| {
            let parsed = (
                focus.trim().parse::<u16>(),
                short_break.trim().parse::<u16>(),
                long_break.trim().parse::<u16>(),
                interval.trim().parse::<u8>(),
            );
            match parsed {
                (
                    Ok(focus_minutes),
                    Ok(short_break_minutes),
                    Ok(long_break_minutes),
                    Ok(long_break_interval),
                ) => {
                    settings_handler(UiIntent::SetPomodoroSettings(PomodoroSettings {
                        focus_minutes,
                        short_break_minutes,
                        long_break_minutes,
                        long_break_interval,
                        auto_start_break: auto_break,
                        auto_start_focus: auto_focus,
                    }));
                }
                _ => show_invalid_focus_action(&settings_main),
            }
        },
    );
}

fn show_invalid_focus_action(main: &slint::Weak<MainWindow>) {
    if let Some(main) = main.upgrade() {
        main.invoke_show_toast(
            SharedString::from("Focus settings or selection are invalid."),
            ToastKind::Error,
        );
    }
}

fn show_quick_add(quick_add: &QuickAddWindow, main: &MainWindow) {
    quick_add.set_title_text(SharedString::default());
    quick_add.set_destination(0);
    quick_add.set_error_message(SharedString::default());
    if quick_add.show().is_err() {
        main.invoke_show_toast(
            SharedString::from("Quick Add could not be opened."),
            ToastKind::Error,
        );
    }
}

fn bind_task_actions(main_window: &MainWindow, intent_handler: &Rc<dyn Fn(UiIntent)>) {
    let move_handler = Rc::clone(intent_handler);
    let move_main = main_window.as_weak();
    main_window.on_task_move_requested(move |id, destination| {
        let parsed = TaskId::from_str(id.as_str());
        let placement = placement_from_destination(destination);
        match (parsed, placement) {
            (Ok(task_id), Some(placement)) => {
                move_handler(UiIntent::MoveTask { task_id, placement });
            }
            _ => show_invalid_task_action(&move_main),
        }
    });

    let reorder_handler = Rc::clone(intent_handler);
    let reorder_main = main_window.as_weak();
    main_window.on_task_reorder_requested(move |id, direction| {
        let direction = match direction {
            -1 => Some(ReorderDirection::Up),
            1 => Some(ReorderDirection::Down),
            _ => None,
        };
        match (TaskId::from_str(id.as_str()), direction) {
            (Ok(task_id), Some(direction)) => {
                reorder_handler(UiIntent::ReorderTask { task_id, direction });
            }
            _ => show_invalid_task_action(&reorder_main),
        }
    });

    let edit_handler = Rc::clone(intent_handler);
    let edit_main = main_window.as_weak();
    main_window.on_task_edit_requested(move |id| match TaskId::from_str(id.as_str()) {
        Ok(task_id) => edit_handler(UiIntent::OpenTaskEditor(task_id)),
        Err(_) => show_invalid_task_action(&edit_main),
    });

    let complete_handler = Rc::clone(intent_handler);
    let complete_main = main_window.as_weak();
    main_window.on_task_complete_requested(move |id| match TaskId::from_str(id.as_str()) {
        Ok(task_id) => complete_handler(UiIntent::CompleteTask(task_id)),
        Err(_) => show_invalid_task_action(&complete_main),
    });

    let delete_handler = Rc::clone(intent_handler);
    let delete_main = main_window.as_weak();
    main_window.on_task_delete_confirmed(move |id| match TaskId::from_str(id.as_str()) {
        Ok(task_id) => delete_handler(UiIntent::DeleteTask(task_id)),
        Err(_) => show_invalid_task_action(&delete_main),
    });
}

fn bind_main_window_controls(main_window: &MainWindow) {
    let minimize_weak = main_window.as_weak();
    main_window.on_window_minimize(move || {
        if let Some(window) = minimize_weak.upgrade() {
            if should_hide_to_tray(window.get_tray_supported(), window.get_minimize_to_tray()) {
                drop(window.hide());
            } else {
                window.window().set_minimized(true);
            }
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

    let close_weak = main_window.as_weak();
    main_window.on_window_close(move || {
        if let Some(window) = close_weak.upgrade() {
            if should_hide_to_tray(window.get_tray_supported(), window.get_close_to_tray()) {
                drop(window.hide());
            } else {
                drop(slint::quit_event_loop());
            }
        }
    });

    let native_close_weak = main_window.as_weak();
    main_window.window().on_close_requested(move || {
        if let Some(window) = native_close_weak.upgrade()
            && !should_hide_to_tray(window.get_tray_supported(), window.get_close_to_tray())
        {
            drop(slint::quit_event_loop());
        }
        slint::CloseRequestResponse::HideWindow
    });
}

fn bind_task_editor(task_editor: &TaskEditorWindow, intent_handler: Rc<dyn Fn(UiIntent)>) {
    let cancel_weak = task_editor.as_weak();
    task_editor.on_cancelled(move || {
        if let Some(window) = cancel_weak.upgrade() {
            drop(window.hide());
        }
    });

    let submit_weak = task_editor.as_weak();
    task_editor.on_submitted(
        move |id,
              title,
              notes,
              destination,
              planned_on,
              due_local,
              due_time_zone,
              reminder_local,
              reminder_time_zone,
              recurrence,
              custom_interval_days| {
            let Some(window) = submit_weak.upgrade() else {
                return;
            };
            clear_task_editor_errors(&window);
            let fields = TaskEditorUiSubmission {
                id,
                title,
                notes,
                destination,
                planned_on,
                due_local,
                due_time_zone,
                reminder_local,
                reminder_time_zone,
                recurrence,
                custom_interval_days,
            };
            match validate_task_editor_submission(&fields) {
                Ok(submission) => intent_handler(UiIntent::SubmitTaskEditor(submission)),
                Err((field, message)) => set_task_editor_field_error(&window, field, message),
            }
        },
    );

    let close_weak = task_editor.as_weak();
    task_editor.window().on_close_requested(move || {
        if let Some(window) = close_weak.upgrade() {
            drop(window.hide());
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
}

struct TaskEditorUiSubmission {
    id: SharedString,
    title: SharedString,
    notes: SharedString,
    destination: i32,
    planned_on: SharedString,
    due_local: SharedString,
    due_time_zone: SharedString,
    reminder_local: SharedString,
    reminder_time_zone: SharedString,
    recurrence: i32,
    custom_interval_days: SharedString,
}

type TaskEditorInputError = (TaskEditorField, &'static str);

fn validate_task_editor_submission(
    fields: &TaskEditorUiSubmission,
) -> Result<TaskEditorSubmission, TaskEditorInputError> {
    let task_id = TaskId::from_str(fields.id.as_str()).map_err(|_| {
        (
            TaskEditorField::General,
            "The editor state is no longer valid.",
        )
    })?;
    let placement = placement_from_destination(fields.destination).ok_or((
        TaskEditorField::General,
        "The editor state is no longer valid.",
    ))?;
    let recurrence = recurrence_from_index(fields.recurrence).ok_or((
        TaskEditorField::General,
        "The editor state is no longer valid.",
    ))?;
    let title = fields.title.to_string();
    if title.trim().is_empty() {
        return Err((TaskEditorField::Title, "Task title is required."));
    }
    if title.trim().chars().count() > 500 {
        return Err((
            TaskEditorField::Title,
            "Task title cannot exceed 500 characters.",
        ));
    }
    if recurrence == RecurrenceChoice::CustomDays
        && fields
            .custom_interval_days
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|days| (1..=365).contains(days))
            .is_none()
    {
        return Err((
            TaskEditorField::Recurrence,
            "Custom recurrence must be between 1 and 365 days.",
        ));
    }

    let planned_on = planned_input(fields.planned_on.as_str())
        .map_err(|message| (TaskEditorField::PlannedDate, message))?;
    let due_at = scheduled_input(fields.due_local.as_str(), fields.due_time_zone.as_str())
        .map_err(|(field, message)| (field.due_field(), message))?;
    let reminder_at = scheduled_input(
        fields.reminder_local.as_str(),
        fields.reminder_time_zone.as_str(),
    )
    .map_err(|(field, message)| (field.reminder_field(), message))?;
    if let (Ok(due), Ok(reminder)) = (
        due_at.parse::<Timestamp>(),
        reminder_at.parse::<Timestamp>(),
    ) && reminder > due
    {
        return Err((
            TaskEditorField::ReminderDateTime,
            "Reminder cannot be after due time.",
        ));
    }

    Ok(TaskEditorSubmission {
        task_id,
        title,
        notes: fields.notes.to_string(),
        placement,
        planned_on,
        due_at,
        due_time_zone: fields.due_time_zone.trim().to_owned(),
        reminder_at,
        reminder_time_zone: fields.reminder_time_zone.trim().to_owned(),
        recurrence,
        custom_interval_days: fields.custom_interval_days.to_string(),
    })
}

#[derive(Clone, Copy, Debug)]
enum ScheduleInputField {
    DateTime,
    TimeZone,
}

impl ScheduleInputField {
    const fn due_field(self) -> TaskEditorField {
        match self {
            Self::DateTime => TaskEditorField::DueDateTime,
            Self::TimeZone => TaskEditorField::DueTimeZone,
        }
    }

    const fn reminder_field(self) -> TaskEditorField {
        match self {
            Self::DateTime => TaskEditorField::ReminderDateTime,
            Self::TimeZone => TaskEditorField::ReminderTimeZone,
        }
    }
}

fn planned_input(value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    value
        .parse::<jiff::civil::Date>()
        .map(|value| value.to_string())
        .map_err(|_| "Choose a valid planned date.")
}

fn scheduled_input(
    local_date_time: &str,
    time_zone: &str,
) -> Result<String, (ScheduleInputField, &'static str)> {
    let local_date_time = local_date_time.trim();
    let time_zone = time_zone.trim();
    if local_date_time.is_empty() && time_zone.is_empty() {
        return Ok(String::new());
    }
    if local_date_time.is_empty() {
        return Err((
            ScheduleInputField::DateTime,
            "Choose a valid local date and time.",
        ));
    }
    if time_zone.is_empty() {
        return Err((
            ScheduleInputField::TimeZone,
            "Enter an IANA timezone such as Asia/Shanghai.",
        ));
    }
    let time_zone = TimeZone::get(time_zone).map_err(|_| {
        (
            ScheduleInputField::TimeZone,
            "Enter a valid IANA timezone such as Asia/Shanghai.",
        )
    })?;
    let date_time = local_date_time.parse::<DateTime>().map_err(|_| {
        (
            ScheduleInputField::DateTime,
            "Choose a valid local date and time.",
        )
    })?;
    time_zone
        .to_ambiguous_timestamp(date_time)
        .unambiguous()
        .map(|timestamp| timestamp.to_string())
        .map_err(|_| {
            (
                ScheduleInputField::DateTime,
                "This local time is skipped or repeated by daylight saving time. Choose another time.",
            )
        })
}

fn clear_task_editor_errors(editor: &TaskEditorWindow) {
    editor.set_error_message(SharedString::default());
    editor.set_title_error(SharedString::default());
    editor.set_planned_error(SharedString::default());
    editor.set_due_error(SharedString::default());
    editor.set_due_time_zone_error(SharedString::default());
    editor.set_reminder_error(SharedString::default());
    editor.set_reminder_time_zone_error(SharedString::default());
    editor.set_recurrence_error(SharedString::default());
}

fn set_task_editor_field_error(
    editor: &TaskEditorWindow,
    field: TaskEditorField,
    message: impl Into<SharedString>,
) {
    let message = message.into();
    match field {
        TaskEditorField::General => editor.set_error_message(message),
        TaskEditorField::Title => editor.set_title_error(message),
        TaskEditorField::PlannedDate => editor.set_planned_error(message),
        TaskEditorField::DueDateTime => editor.set_due_error(message),
        TaskEditorField::DueTimeZone => editor.set_due_time_zone_error(message),
        TaskEditorField::ReminderDateTime => editor.set_reminder_error(message),
        TaskEditorField::ReminderTimeZone => editor.set_reminder_time_zone_error(message),
        TaskEditorField::Recurrence => editor.set_recurrence_error(message),
    }
}

fn bind_quick_add(quick_add: &QuickAddWindow, intent_handler: Rc<dyn Fn(UiIntent)>) {
    let cancel_weak = quick_add.as_weak();
    quick_add.on_cancelled(move || {
        if let Some(window) = cancel_weak.upgrade() {
            drop(window.hide());
        }
    });

    let submit_weak = quick_add.as_weak();
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
    });

    let close_weak = quick_add.as_weak();
    quick_add.window().on_close_requested(move || {
        if let Some(window) = close_weak.upgrade() {
            drop(window.hide());
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
}

fn show_invalid_task_action(main: &slint::Weak<MainWindow>) {
    if let Some(main) = main.upgrade() {
        main.invoke_show_toast(
            SharedString::from("That task action is no longer valid."),
            ToastKind::Error,
        );
    }
}

fn apply_application_event(
    main: &MainWindow,
    task_editor: &slint::Weak<TaskEditorWindow>,
    focus_session: &Arc<Mutex<Option<FocusSession>>>,
    event: ApplicationEvent,
) {
    match event {
        ApplicationEvent::QuadrantsChanged(state) => apply_quadrants_state(main, &state),
        ApplicationEvent::TodayChanged(state) => apply_today_state(main, &state),
        ApplicationEvent::FocusChanged(state) => apply_focus_state(main, &state, focus_session),
        ApplicationEvent::ReviewChanged(state) => apply_review_state(main, &state),
        ApplicationEvent::CompletedChanged(state) => apply_completed_state(main, &state),
        ApplicationEvent::MaintenanceChanged(state) => apply_maintenance_state(main, &state),
        ApplicationEvent::ReminderDue(alert) => {
            main.invoke_show_toast(
                SharedString::from(format!("Reminder: {}", alert.title)),
                ToastKind::Info,
            );
        }
        ApplicationEvent::TaskEditorLoaded(state) => {
            if let Some(editor) = task_editor.upgrade() {
                apply_task_editor_state(&editor, &state);
                if editor.show().is_err() {
                    main.invoke_show_toast(
                        SharedString::from("The task editor could not be opened."),
                        ToastKind::Error,
                    );
                }
            }
        }
        ApplicationEvent::TaskEditorSaved => {
            if let Some(editor) = task_editor.upgrade() {
                drop(editor.hide());
            }
        }
        ApplicationEvent::TaskEditorValidationFailed { field, message } => {
            if let Some(editor) = task_editor.upgrade() {
                set_task_editor_field_error(&editor, field, message);
            }
        }
        ApplicationEvent::DesktopSettingsChanged(settings) => {
            apply_desktop_settings(main, settings);
        }
        ApplicationEvent::OperationSucceeded(message) => {
            main.invoke_show_toast(SharedString::from(message), ToastKind::Success);
        }
        ApplicationEvent::OperationFailed(error) => {
            main.invoke_show_toast(SharedString::from(error.message), ToastKind::Error);
        }
    }
}

fn apply_focus_state(
    main: &MainWindow,
    state: &FocusViewState,
    focus_session: &Arc<Mutex<Option<FocusSession>>>,
) {
    main.set_focus_tasks(focus_task_model(&state.tasks));
    main.set_focus_minutes(SharedString::from(state.settings.focus_minutes.to_string()));
    main.set_short_break_minutes(SharedString::from(
        state.settings.short_break_minutes.to_string(),
    ));
    main.set_long_break_minutes(SharedString::from(
        state.settings.long_break_minutes.to_string(),
    ));
    main.set_long_break_interval(SharedString::from(
        state.settings.long_break_interval.to_string(),
    ));
    main.set_auto_start_break(state.settings.auto_start_break);
    main.set_auto_start_focus(state.settings.auto_start_focus);
    main.set_focus_today_summary(SharedString::from(if state.today.session_count == 0 {
        "No productive focus completed today".to_owned()
    } else {
        format!(
            "Today: {} across {} session{}",
            format_duration(state.today.total_seconds),
            state.today.session_count,
            if state.today.session_count == 1 {
                ""
            } else {
                "s"
            }
        )
    }));
    if let Ok(mut session) = focus_session.lock() {
        session.clone_from(&state.session);
    }
    if let Some(session) = state.session.as_ref() {
        let record = session.record();
        main.set_focus_selected_mode(match record.mode {
            FocusMode::Stopwatch => 0,
            FocusMode::Pomodoro => 1,
        });
        main.set_focus_selected_kind(match record.pomodoro_kind {
            None | Some(PomodoroKind::Focus) => 0,
            Some(PomodoroKind::ShortBreak) => 1,
            Some(PomodoroKind::LongBreak) => 2,
        });
        main.set_focus_selected_task_id(SharedString::from(
            record
                .task
                .as_ref()
                .and_then(|task| task.id)
                .map(|id| id.to_string())
                .unwrap_or_default(),
        ));
        update_focus_projection(main, session, current_utc());
    } else {
        main.set_focus_session_status(-1);
        main.set_focus_timer_progress(0.0);
        main.set_focus_session_label(SharedString::from("Ready to focus"));
        main.set_focus_timer_text(SharedString::from(format_clock(
            u64::from(state.settings.focus_minutes) * 60,
        )));
    }
}

fn start_focus_projection_timer(
    main: &MainWindow,
    focus_session: Arc<Mutex<Option<FocusSession>>>,
) -> slint::Timer {
    let timer = slint::Timer::default();
    let main = main.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
        let Some(main) = main.upgrade() else {
            return;
        };
        let Ok(session) = focus_session.lock() else {
            return;
        };
        if let Some(session) = session.as_ref() {
            update_focus_projection(&main, session, current_utc());
        }
    });
    timer
}

fn update_focus_projection(main: &MainWindow, session: &FocusSession, now: UtcTimestamp) {
    let record = session.record();
    main.set_focus_session_status(match record.status {
        FocusStatus::Running => 0,
        FocusStatus::Paused => 1,
        FocusStatus::Completed | FocusStatus::Cancelled => -1,
    });
    let elapsed = session.elapsed_seconds_at(now);
    let display = session.remaining_seconds_at(now).unwrap_or(elapsed);
    main.set_focus_timer_text(SharedString::from(format_clock(u64::from(display))));
    let progress = record.target_duration_seconds.map_or(0.0, |target| {
        let elapsed = u16::try_from(elapsed).unwrap_or(u16::MAX);
        let target = u16::try_from(target).unwrap_or(u16::MAX);
        (f32::from(elapsed) / f32::from(target)).clamp(0.0, 1.0)
    });
    main.set_focus_timer_progress(progress);
    let phase = match (record.mode, record.pomodoro_kind) {
        (FocusMode::Stopwatch, _) => "Stopwatch",
        (_, Some(PomodoroKind::Focus)) => "Pomodoro focus",
        (_, Some(PomodoroKind::ShortBreak)) => "Short break",
        (_, Some(PomodoroKind::LongBreak)) => "Long break",
        _ => "Focus",
    };
    let status = if record.status == FocusStatus::Paused {
        "Paused"
    } else {
        "Running"
    };
    let task = record
        .task
        .as_ref()
        .map_or(String::new(), |task| format!(" · {}", task.title));
    main.set_focus_session_label(SharedString::from(format!("{phase} · {status}{task}")));
}

fn current_utc() -> UtcTimestamp {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    UtcTimestamp::from_unix_seconds(i64::try_from(seconds).unwrap_or(i64::MAX))
}

fn format_clock(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn apply_review_state(main: &MainWindow, state: &ReviewViewState) {
    main.set_review_range(state.range.index());
    main.set_review_completed_value(SharedString::from(
        state.current.completed_tasks.to_string(),
    ));
    main.set_review_completed_hint(SharedString::from(comparison_hint(
        state.current.completed_tasks,
        state.previous.map(|value| value.completed_tasks),
    )));
    main.set_review_focus_value(SharedString::from(format_duration(
        state.current.focus_seconds,
    )));
    main.set_review_focus_hint(SharedString::from(comparison_hint(
        state.current.focus_seconds,
        state.previous.map(|value| value.focus_seconds),
    )));
    main.set_review_sessions_value(SharedString::from(state.current.focus_sessions.to_string()));
    main.set_review_sessions_hint(SharedString::from(comparison_hint(
        state.current.focus_sessions,
        state.previous.map(|value| value.focus_sessions),
    )));
    main.set_review_average_value(SharedString::from(format_duration(
        state.current.average_focus_seconds(),
    )));
    main.set_review_state_summary(SharedString::from(format!(
        "Inbox {} · Overdue {}",
        state.current_inbox_count, state.current_overdue_count
    )));
    main.set_review_activity(review_activity_model(state));
    main.set_review_activity_completed_max(saturating_i32(state.completed_activity_max));
    main.set_review_activity_focus_max(saturating_i32(state.focus_activity_max));
    main.set_review_quadrants(review_quadrant_model(state));
    main.set_review_quadrant_completed_max(saturating_i32(
        state
            .quadrants
            .iter()
            .map(|value| value.completed)
            .max()
            .unwrap_or(0)
            .max(1),
    ));
    main.set_review_quadrant_focus_max(saturating_i32(
        state
            .quadrants
            .iter()
            .map(|value| value.focus_seconds)
            .max()
            .unwrap_or(0)
            .max(1),
    ));
    main.set_review_longest_focus(SharedString::from(format_duration(
        state.focus.longest_session_seconds,
    )));
    main.set_review_top_task(SharedString::from(
        state
            .focus
            .most_focused_task_title
            .as_deref()
            .unwrap_or("No linked task"),
    ));
    main.set_review_top_task_detail(SharedString::from(
        if state.focus.most_focused_task_title.is_some() {
            format!(
                "{} across {} session{}",
                format_duration(state.focus.most_focused_task_seconds),
                state.focus.most_focused_task_sessions,
                if state.focus.most_focused_task_sessions == 1 {
                    ""
                } else {
                    "s"
                }
            )
        } else {
            "Complete linked Focus sessions to see a leader.".to_owned()
        },
    ));
    main.set_review_top_quadrant(SharedString::from(
        state.focus.most_focused_quadrant.map_or_else(
            || "No quadrant Focus yet".to_owned(),
            |quadrant| {
                format!(
                    "Top quadrant: {} · {}",
                    quadrant_label(quadrant),
                    format_duration(state.focus.most_focused_quadrant_seconds)
                )
            },
        ),
    ));
    main.set_review_recent(review_recent_model(state));
}

fn comparison_hint(current: u64, previous: Option<u64>) -> String {
    previous.map_or_else(
        || "All retained history".to_owned(),
        |previous| {
            let delta = i128::from(current) - i128::from(previous);
            format!("{delta:+} vs previous period")
        },
    )
}

fn review_activity_model(state: &ReviewViewState) -> ModelRc<ReviewActivityRow> {
    let rows = state
        .activity
        .iter()
        .map(|point| ReviewActivityRow {
            label: SharedString::from(activity_label(state.range, point.date)),
            completed: saturating_i32(point.completed),
            focus_seconds: saturating_i32(point.focus_seconds),
        })
        .collect::<Vec<_>>();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

fn activity_label(range: ReviewRange, date: quadrant_application::LocalDate) -> String {
    if range == ReviewRange::AllTime {
        format!("{:04}-{:02}", date.year(), date.month())
    } else {
        format!("{:02}-{:02}", date.month(), date.day())
    }
}

fn review_quadrant_model(state: &ReviewViewState) -> ModelRc<ReviewQuadrantRow> {
    let rows = state
        .quadrants
        .iter()
        .map(|value| ReviewQuadrantRow {
            label: SharedString::from(value.quadrant.map_or("Inbox / Unlinked", quadrant_label)),
            completed: saturating_i32(value.completed),
            focus_seconds: saturating_i32(value.focus_seconds),
            focus_text: SharedString::from(format_duration(value.focus_seconds)),
        })
        .collect::<Vec<_>>();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

fn review_recent_model(state: &ReviewViewState) -> ModelRc<ReviewRecentRow> {
    let rows = state
        .recent_completed
        .iter()
        .map(|item| {
            let placement = item.quadrant.map_or("Inbox", quadrant_label);
            let overdue = if item.was_overdue {
                " · was overdue"
            } else {
                ""
            };
            ReviewRecentRow {
                title: SharedString::from(item.title.as_str()),
                metadata: SharedString::from(format!(
                    "{} · {placement}{overdue}",
                    item.completed_local_date
                )),
            }
        })
        .collect::<Vec<_>>();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

fn apply_completed_state(main: &MainWindow, state: &CompletedViewState) {
    let rows = state
        .tasks
        .iter()
        .map(|task| CompletedTaskRow {
            id: SharedString::from(task.id.to_string()),
            title: SharedString::from(task.title.as_str()),
            metadata: SharedString::from(task.metadata.as_str()),
        })
        .collect::<Vec<_>>();
    main.set_completed_tasks(ModelRc::from(Rc::new(VecModel::from(rows))));
    main.set_completed_has_more(state.has_more);
}

fn apply_maintenance_state(main: &MainWindow, state: &MaintenanceState) {
    main.set_backup_directory(SharedString::from(
        state.backup_directory.to_string_lossy().as_ref(),
    ));
    main.set_latest_backup(SharedString::from(
        state.latest_backup.as_ref().map_or_else(
            || "No backup created yet".to_owned(),
            |backup| {
                let filename = backup.path.file_name().map_or_else(
                    || backup.path.display().to_string(),
                    |value| value.to_string_lossy().into_owned(),
                );
                format!(
                    "Latest: {filename} · {}",
                    format_file_size(backup.size_bytes)
                )
            },
        ),
    ));
    main.set_restore_pending(state.restore_pending);
}

fn format_file_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes >= MIB {
        format!("{} MiB", bytes.div_ceil(MIB))
    } else if bytes >= KIB {
        format!("{} KiB", bytes.div_ceil(KIB))
    } else {
        format!("{bytes} B")
    }
}

const fn quadrant_label(quadrant: Quadrant) -> &'static str {
    match quadrant {
        Quadrant::Q1 => "Q1",
        Quadrant::Q2 => "Q2",
        Quadrant::Q3 => "Q3",
        Quadrant::Q4 => "Q4",
    }
}

fn saturating_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn focus_task_model(tasks: &[quadrant_application::FocusTaskSummary]) -> ModelRc<FocusTaskRow> {
    let rows = tasks
        .iter()
        .map(|task| FocusTaskRow {
            id: SharedString::from(task.id.to_string()),
            title: SharedString::from(task.title.as_str()),
            metadata: SharedString::from(placement_label(task.placement)),
        })
        .collect::<Vec<_>>();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

const fn placement_label(placement: TaskPlacement) -> &'static str {
    match placement {
        TaskPlacement::Inbox => "Inbox",
        TaskPlacement::Quadrant(Quadrant::Q1) => "Q1",
        TaskPlacement::Quadrant(Quadrant::Q2) => "Q2",
        TaskPlacement::Quadrant(Quadrant::Q3) => "Q3",
        TaskPlacement::Quadrant(Quadrant::Q4) => "Q4",
    }
}

fn apply_desktop_settings(main: &MainWindow, settings: DesktopSettings) {
    main.set_launch_at_startup(settings.launch_at_startup);
    main.set_start_hidden(settings.start_hidden);
    main.set_close_to_tray(settings.close_behavior == WindowCloseBehavior::HideToTray);
    main.set_minimize_to_tray(settings.minimize_behavior == WindowMinimizeBehavior::HideToTray);
}

const fn should_hide_to_tray(tray_available: bool, setting_enabled: bool) -> bool {
    tray_available && setting_enabled
}

const fn should_hide_at_startup(
    tray_available: bool,
    setting_enabled: bool,
    background_requested: bool,
) -> bool {
    tray_available && (setting_enabled || background_requested)
}

fn apply_task_editor_state(editor: &TaskEditorWindow, state: &TaskEditorState) {
    editor.set_task_id(SharedString::from(state.task_id.to_string()));
    editor.set_title_text(SharedString::from(state.title.as_str()));
    editor.set_notes_text(SharedString::from(state.notes.as_str()));
    editor.set_destination(destination_from_placement(state.placement));
    editor.set_planned_on(SharedString::from(state.planned_on.as_str()));
    editor.set_due_at(SharedString::from(state.due_at.as_str()));
    editor.set_due_time_zone(SharedString::from(state.due_time_zone.as_str()));
    editor.set_reminder_at(SharedString::from(state.reminder_at.as_str()));
    editor.set_reminder_time_zone(SharedString::from(state.reminder_time_zone.as_str()));
    editor.set_recurrence(recurrence_index(state.recurrence));
    editor.set_custom_interval_days(SharedString::from(state.custom_interval_days.as_str()));
    let (default_date, default_time) = default_editor_date_time();
    let planned_date = parse_editor_date(&state.planned_on).unwrap_or_else(|| default_date.clone());
    editor.set_planned_selected(!state.planned_on.is_empty());
    editor.set_planned_date(planned_date);

    let (due_selected, due_date, due_time) =
        parse_editor_schedule(&state.due_at, &state.due_time_zone).unwrap_or_else(|| {
            (
                !state.due_at.is_empty(),
                default_date.clone(),
                default_time.clone(),
            )
        });
    editor.set_due_selected(due_selected);
    editor.set_due_date(due_date);
    editor.set_due_time(due_time);

    let (reminder_selected, reminder_date, reminder_time) = parse_editor_schedule(
        &state.reminder_at,
        &state.reminder_time_zone,
    )
    .unwrap_or((!state.reminder_at.is_empty(), default_date, default_time));
    editor.set_reminder_selected(reminder_selected);
    editor.set_reminder_date(reminder_date);
    editor.set_reminder_time(reminder_time);

    if state.due_time_zone.is_empty() {
        editor.set_due_time_zone(SharedString::from("UTC"));
    }
    if state.reminder_time_zone.is_empty() {
        editor.set_reminder_time_zone(SharedString::from("UTC"));
    }
    clear_task_editor_errors(editor);
}

fn default_editor_date_time() -> (SlintDate, SlintTime) {
    let now = Timestamp::now().to_zoned(TimeZone::UTC);
    (
        SlintDate {
            year: i32::from(now.year()),
            month: i32::from(now.month()),
            day: i32::from(now.day()),
        },
        SlintTime {
            hour: i32::from(now.hour()),
            minute: i32::from(now.minute()),
            second: 0,
        },
    )
}

fn parse_editor_date(value: &str) -> Option<SlintDate> {
    let date = value.parse::<jiff::civil::Date>().ok()?;
    Some(SlintDate {
        year: i32::from(date.year()),
        month: i32::from(date.month()),
        day: i32::from(date.day()),
    })
}

fn parse_editor_schedule(value: &str, time_zone: &str) -> Option<(bool, SlintDate, SlintTime)> {
    if value.is_empty() {
        return None;
    }
    let timestamp = value.parse::<Timestamp>().ok()?;
    let time_zone = TimeZone::get(time_zone).ok()?;
    let local = timestamp.to_zoned(time_zone);
    Some((
        true,
        SlintDate {
            year: i32::from(local.year()),
            month: i32::from(local.month()),
            day: i32::from(local.day()),
        },
        SlintTime {
            hour: i32::from(local.hour()),
            minute: i32::from(local.minute()),
            second: i32::from(local.second()),
        },
    ))
}

fn apply_quadrants_state(main: &MainWindow, state: &QuadrantsViewState) {
    main.set_inbox_tasks(inbox_model(&state.inbox));
    main.set_q1_tasks(task_model(&state.q1));
    main.set_q2_tasks(task_model(&state.q2));
    main.set_q3_tasks(task_model(&state.q3));
    main.set_q4_tasks(task_model(&state.q4));
}

fn inbox_model(tasks: &[quadrant_application::TaskSummary]) -> ModelRc<InboxItem> {
    let rows = tasks
        .iter()
        .map(|task| InboxItem {
            id: SharedString::from(task.id.to_string()),
            title: SharedString::from(task.title.as_str()),
            supporting_text: SharedString::default(),
            completed: false,
            selected: false,
            disabled: false,
        })
        .collect::<Vec<_>>();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

fn apply_today_state(main: &MainWindow, state: &TodayViewState) {
    main.set_overdue_tasks(today_model(&state.overdue));
    main.set_planned_today_tasks(today_model(&state.planned_today));
    main.set_due_today_tasks(today_model(&state.due_today));
    main.set_needs_reschedule_tasks(today_model(&state.needs_reschedule));
    main.set_today_task_count(i32::try_from(state.unique_task_count).unwrap_or(i32::MAX));
}

fn today_model(tasks: &[quadrant_application::TodayTaskSummary]) -> ModelRc<TodayTaskRow> {
    let rows = tasks
        .iter()
        .map(|task| TodayTaskRow {
            id: SharedString::from(task.id.to_string()),
            title: SharedString::from(task.title.as_str()),
            metadata: SharedString::from(task.metadata.as_str()),
        })
        .collect::<Vec<_>>();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

fn task_model(tasks: &[quadrant_application::TaskSummary]) -> ModelRc<TaskRow> {
    let rows = tasks
        .iter()
        .map(|task| TaskRow {
            id: SharedString::from(task.id.to_string()),
            title: SharedString::from(task.title.as_str()),
        })
        .collect::<Vec<_>>();
    ModelRc::from(Rc::new(VecModel::from(rows)))
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

const fn destination_from_placement(placement: TaskPlacement) -> i32 {
    match placement {
        TaskPlacement::Inbox => 0,
        TaskPlacement::Quadrant(Quadrant::Q1) => 1,
        TaskPlacement::Quadrant(Quadrant::Q2) => 2,
        TaskPlacement::Quadrant(Quadrant::Q3) => 3,
        TaskPlacement::Quadrant(Quadrant::Q4) => 4,
    }
}

const fn recurrence_from_index(index: i32) -> Option<RecurrenceChoice> {
    match index {
        0 => Some(RecurrenceChoice::None),
        1 => Some(RecurrenceChoice::Daily),
        2 => Some(RecurrenceChoice::Weekly),
        3 => Some(RecurrenceChoice::Monthly),
        4 => Some(RecurrenceChoice::CustomDays),
        _ => None,
    }
}

const fn recurrence_index(recurrence: RecurrenceChoice) -> i32 {
    match recurrence {
        RecurrenceChoice::None => 0,
        RecurrenceChoice::Daily => 1,
        RecurrenceChoice::Weekly => 2,
        RecurrenceChoice::Monthly => 3,
        RecurrenceChoice::CustomDays => 4,
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
    use super::{
        inbox_model, parse_editor_schedule, placement_from_destination, scheduled_input,
        should_hide_at_startup, should_hide_to_tray,
    };
    use quadrant_application::{Quadrant, TaskId, TaskPlacement, TaskSummary};
    use slint::Model;
    use std::str::FromStr;

    #[test]
    fn inbox_adapter_produces_presentation_only_defaults() {
        let task_id = TaskId::from_str("018f3f76-3773-7a35-b310-48f25ed6bc93")
            .expect("fixture id should be valid");
        let model = inbox_model(&[TaskSummary {
            id: task_id,
            title: "Capture portable Inbox contract".to_owned(),
            placement: TaskPlacement::Inbox,
        }]);

        assert_eq!(model.row_count(), 1);
        let row = model.row_data(0).expect("adapter should retain the row");
        assert_eq!(row.id.as_str(), task_id.to_string());
        assert_eq!(row.title.as_str(), "Capture portable Inbox contract");
        assert!(row.supporting_text.is_empty());
        assert!(!row.completed);
        assert!(!row.selected);
        assert!(!row.disabled);
    }

    #[test]
    fn quick_add_destinations_map_to_typed_placement() {
        assert_eq!(placement_from_destination(0), Some(TaskPlacement::Inbox));
        assert_eq!(
            placement_from_destination(4),
            Some(TaskPlacement::Quadrant(Quadrant::Q4))
        );
        assert_eq!(placement_from_destination(5), None);
    }

    #[test]
    fn tray_window_policies_never_hide_without_a_recovery_surface() {
        assert!(should_hide_to_tray(true, true));
        assert!(!should_hide_to_tray(false, true));
        assert!(should_hide_at_startup(true, false, true));
        assert!(!should_hide_at_startup(false, true, true));
    }

    #[test]
    fn editor_local_schedule_round_trips_through_utc() {
        let timestamp =
            scheduled_input("2026-09-03T09:15:00", "Asia/Shanghai").expect("valid local schedule");
        assert_eq!(timestamp, "2026-09-03T01:15:00Z");

        let (_, date, time) =
            parse_editor_schedule(&timestamp, "Asia/Shanghai").expect("valid persisted schedule");
        assert_eq!((date.year, date.month, date.day), (2026, 9, 3));
        assert_eq!((time.hour, time.minute), (9, 15));
    }

    #[test]
    fn editor_rejects_skipped_and_repeated_dst_times() {
        let spring_gap = scheduled_input("2026-03-08T02:30:00", "America/New_York");
        assert!(spring_gap.is_err());

        let autumn_fold = scheduled_input("2026-11-01T01:30:00", "America/New_York");
        assert!(autumn_fold.is_err());
    }
}
