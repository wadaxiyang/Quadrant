//! Main/Quick Add window construction and typed callback binding.

use std::{rc::Rc, str::FromStr, sync::Arc};

use quadrant_application::{
    ApplicationEvent, NavigationRoute, Quadrant, QuadrantsViewState, QuickAddSubmission,
    RecurrenceChoice, ReorderDirection, SystemTheme, TaskEditorState, TaskEditorSubmission, TaskId,
    TaskPlacement, ThemeMode as ApplicationThemeMode, UiIntent,
};
use slint::{ComponentHandle, ModelRc, PhysicalPosition, SharedString, VecModel};

use crate::{
    MainWindow, QuickAddWindow, TaskEditorWindow, TaskRow, ThemeMode as SlintThemeMode, ToastKind,
};

/// Initial state supplied by the composition root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiShellConfig {
    /// User-selected theme behavior.
    pub theme_mode: ApplicationThemeMode,
    /// Current normalized platform appearance.
    pub system_theme: SystemTheme,
    /// Initial repository-backed active task projection.
    pub quadrants: QuadrantsViewState,
}

/// Thread-safe sink used by application-runtime work to enqueue typed UI events.
pub type ApplicationEventSink = Arc<dyn Fn(ApplicationEvent) + Send + Sync>;

/// Constructed Slint shell kept on the UI thread.
pub struct UiShell {
    main_window: MainWindow,
    quick_add: QuickAddWindow,
    task_editor: TaskEditorWindow,
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
        let intent_handler: Rc<dyn Fn(UiIntent)> = Rc::new(on_intent);

        initialize_theme(&main_window, &quick_add, &task_editor, config);
        apply_quadrants_state(&main_window, &config.quadrants);
        bind_main_window(&main_window, &quick_add, &task_editor, &intent_handler);
        bind_quick_add(&quick_add, Rc::clone(&intent_handler));
        bind_task_editor(&task_editor, intent_handler);

        Ok(Self {
            main_window,
            quick_add,
            task_editor,
        })
    }

    /// Creates a cross-thread event sink that wakes the Slint event loop without polling.
    #[must_use]
    pub fn event_sink(&self) -> ApplicationEventSink {
        let main_weak = self.main_window.as_weak();
        let editor_weak = self.task_editor.as_weak();
        Arc::new(move |event| {
            let editor_weak = editor_weak.clone();
            drop(main_weak.upgrade_in_event_loop(move |main| {
                apply_application_event(&main, &editor_weak, event);
            }));
        })
    }

    /// Runs the Slint event loop until normal application shutdown.
    ///
    /// # Errors
    ///
    /// Returns an event-loop platform error.
    pub fn run(self) -> Result<(), slint::PlatformError> {
        let result = self.main_window.run();
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

    bind_task_actions(main_window, intent_handler);
    bind_main_window_controls(main_window);
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
              due_at,
              due_time_zone,
              reminder_at,
              reminder_time_zone,
              recurrence,
              custom_interval_days| {
            let Some(window) = submit_weak.upgrade() else {
                return;
            };
            let task_id = TaskId::from_str(id.as_str());
            let placement = placement_from_destination(destination);
            let recurrence = recurrence_from_index(recurrence);
            let (Ok(task_id), Some(placement), Some(recurrence)) = (task_id, placement, recurrence)
            else {
                window
                    .set_error_message(SharedString::from("The editor state is no longer valid."));
                return;
            };
            window.set_error_message(SharedString::default());
            intent_handler(UiIntent::SubmitTaskEditor(TaskEditorSubmission {
                task_id,
                title: title.to_string(),
                notes: notes.to_string(),
                placement,
                planned_on: planned_on.to_string(),
                due_at: due_at.to_string(),
                due_time_zone: due_time_zone.to_string(),
                reminder_at: reminder_at.to_string(),
                reminder_time_zone: reminder_time_zone.to_string(),
                recurrence,
                custom_interval_days: custom_interval_days.to_string(),
            }));
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
    event: ApplicationEvent,
) {
    match event {
        ApplicationEvent::QuadrantsChanged(state) => apply_quadrants_state(main, &state),
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
        ApplicationEvent::TaskEditorValidationFailed(message) => {
            if let Some(editor) = task_editor.upgrade() {
                editor.set_error_message(SharedString::from(message));
            }
        }
        ApplicationEvent::OperationSucceeded(message) => {
            main.invoke_show_toast(SharedString::from(message), ToastKind::Success);
        }
        ApplicationEvent::OperationFailed(error) => {
            main.invoke_show_toast(SharedString::from(error.message), ToastKind::Error);
        }
    }
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
    editor.set_error_message(SharedString::default());
}

fn apply_quadrants_state(main: &MainWindow, state: &QuadrantsViewState) {
    main.set_inbox_tasks(task_model(&state.inbox));
    main.set_q1_tasks(task_model(&state.q1));
    main.set_q2_tasks(task_model(&state.q2));
    main.set_q3_tasks(task_model(&state.q3));
    main.set_q4_tasks(task_model(&state.q4));
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
