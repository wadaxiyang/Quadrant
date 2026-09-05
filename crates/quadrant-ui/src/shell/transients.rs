// SPDX-License-Identifier: GPL-3.0-only
//! UI-thread owners for disposable auxiliary windows and their originating saves.

use super::{
    apply_task_editor_state, bind_quick_add, bind_task_editor, placement_from_destination,
    set_task_editor_field_error, to_slint_theme_mode,
};
use crate::{
    Date as SlintDate, MainWindow, QuickAddWindow, TaskEditorWindow, ThemeMode as SlintThemeMode,
    Time as SlintTime, ToastKind,
};
use quadrant_application::{
    QuickAddSubmission, SystemTheme, TaskEditorField, TaskEditorState, UiIntent,
};
use quadrant_protocol::{AppSnapshot, CommandOutcome, GuiCommand};
use slint::{ComponentHandle, SharedString};
use std::{cell::RefCell, rc::Rc};

type Windows = Rc<RefCell<TransientWindows>>;

pub(super) struct Window<T> {
    pub component: T,
    token: Rc<()>,
}

pub(super) struct TransientWindows {
    pub quick_add: Option<Window<QuickAddWindow>>,
    pub task_editor: Option<Window<TaskEditorWindow>>,
    pub pending: Option<PendingSubmission>,
    mode: SlintThemeMode,
    system_dark: bool,
}

pub(super) enum PendingSubmission {
    Quick {
        token: Rc<()>,
        submission: QuickAddSubmission,
    },
    Editor {
        token: Rc<()>,
        draft: Box<EditorDraft>,
    },
}

/// Presentation draft, including date-picker values not yet converted to UTC.
#[derive(PartialEq)]
pub(super) struct EditorDraft {
    text: [SharedString; 9],
    destination: i32,
    recurrence: i32,
    planned: (bool, SlintDate),
    due: (bool, SlintDate, SlintTime),
    reminder: (bool, SlintDate, SlintTime),
}

impl EditorDraft {
    fn read(editor: &TaskEditorWindow) -> Self {
        Self {
            text: [
                editor.get_task_id(),
                editor.get_title_text(),
                editor.get_notes_text(),
                editor.get_planned_on(),
                editor.get_due_at(),
                editor.get_due_time_zone(),
                editor.get_reminder_at(),
                editor.get_reminder_time_zone(),
                editor.get_custom_interval_days(),
            ],
            destination: editor.get_destination(),
            recurrence: editor.get_recurrence(),
            planned: (editor.get_planned_selected(), editor.get_planned_date()),
            due: (
                editor.get_due_selected(),
                editor.get_due_date(),
                editor.get_due_time(),
            ),
            reminder: (
                editor.get_reminder_selected(),
                editor.get_reminder_date(),
                editor.get_reminder_time(),
            ),
        }
    }
}

impl TransientWindows {
    pub fn new(snapshot: &AppSnapshot) -> Self {
        Self {
            quick_add: None,
            task_editor: None,
            pending: None,
            mode: to_slint_theme_mode(snapshot.theme_mode),
            system_dark: snapshot.system_theme == SystemTheme::Dark,
        }
    }

    pub fn apply_theme(&mut self, mode: SlintThemeMode, system_dark: bool) {
        self.mode = mode;
        self.system_dark = system_dark;
        if let Some(window) = &self.quick_add {
            window.component.invoke_apply_theme(mode, system_dark);
        }
        if let Some(window) = &self.task_editor {
            window.component.invoke_apply_theme(mode, system_dark);
        }
    }

    pub fn set_ready(&self, ready: bool) {
        if let Some(window) = &self.quick_add {
            window.component.set_can_submit(ready);
        }
        if let Some(window) = &self.task_editor {
            window.component.set_can_submit(ready);
        }
    }

    pub fn disconnected(&mut self, message: SharedString) {
        self.pending = None; // Outcome is unknown; never bind it to a future window.
        if let Some(window) = &self.quick_add {
            window.component.set_error_message(message.clone());
        }
        if let Some(window) = &self.task_editor {
            window.component.set_error_message(message);
        }
    }

    pub fn pending_for(&self, command: &GuiCommand) -> Option<PendingSubmission> {
        let GuiCommand::Application(intent) = command else {
            return None;
        };
        match intent.as_ref() {
            UiIntent::SubmitQuickAdd(submission) => {
                self.quick_add
                    .as_ref()
                    .map(|window| PendingSubmission::Quick {
                        token: window.token.clone(),
                        submission: submission.clone(),
                    })
            }
            UiIntent::SubmitTaskEditor(_) => {
                self.task_editor
                    .as_ref()
                    .map(|window| PendingSubmission::Editor {
                        token: window.token.clone(),
                        draft: Box::new(EditorDraft::read(&window.component)),
                    })
            }
            _ => None,
        }
    }

    pub fn editor_validation(&self, field: TaskEditorField, message: String) {
        if let Some(PendingSubmission::Editor { token, .. }) = &self.pending
            && let Some(window) = &self.task_editor
            && Rc::ptr_eq(token, &window.token)
        {
            set_task_editor_field_error(&window.component, field, message);
        }
    }
}

fn show_error(main: &MainWindow, message: &str) {
    main.invoke_show_toast(message.into(), ToastKind::Error);
}

fn show_window_error(main: &MainWindow, message: &str) {
    main.window().set_minimized(false);
    drop(main.show());
    show_error(main, message);
}

pub(super) fn open_quick(windows: &Windows, main: &MainWindow, intents: &Rc<dyn Fn(UiIntent)>) {
    let existing = windows
        .borrow()
        .quick_add
        .as_ref()
        .map(|window| window.component.clone_strong());
    if let Some(window) = existing {
        window.window().set_minimized(false);
        if window.show().is_err() {
            show_window_error(main, "Quick Add could not be opened.");
        }
        return; // A repeated hotkey never resets the current draft.
    }
    let Ok(component) = QuickAddWindow::new() else {
        show_window_error(main, "Quick Add could not be created.");
        return;
    };
    let token = Rc::new(());
    bind_quick_add(&component, intents.clone());
    let owner = Rc::downgrade(windows);
    let identity = token.clone();
    let close: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(owner) = owner.upgrade() {
            close_quick(&owner, &identity);
        }
    });
    let cancel = close.clone();
    component.on_cancelled(move || cancel());
    component.window().on_close_requested(move || {
        close();
        slint::CloseRequestResponse::KeepWindowShown
    });
    component.set_ui_font_family(main.get_ui_font_family());
    component.invoke_apply_theme(windows.borrow().mode, windows.borrow().system_dark);
    component.set_can_submit(main.get_agent_connected() && !main.get_command_pending());
    if !main.get_agent_connected() {
        component.set_error_message("Not connected to the background service.".into());
    }
    windows.borrow_mut().quick_add = Some(Window {
        component: component.clone_strong(),
        token: token.clone(),
    });
    if component.show().is_err() {
        close_quick(windows, &token);
        show_window_error(main, "Quick Add could not be opened.");
    }
}

pub(super) fn open_editor(
    windows: &Windows,
    main: &MainWindow,
    intents: &Rc<dyn Fn(UiIntent)>,
    state: &TaskEditorState,
) {
    let existing = windows
        .borrow()
        .task_editor
        .as_ref()
        .filter(|window| window.component.get_task_id() == state.task_id.to_string())
        .map(|window| window.component.clone_strong());
    if let Some(window) = existing {
        window.window().set_minimized(false);
        if window.show().is_err() {
            show_window_error(main, "The task editor could not be opened.");
        }
        return; // Refresh/activation of the same task preserves unsaved edits.
    }
    let Ok(component) = TaskEditorWindow::new() else {
        show_window_error(main, "The task editor could not be created.");
        return;
    };
    let token = Rc::new(());
    bind_task_editor(&component, intents.clone());
    let owner = Rc::downgrade(windows);
    let identity = token.clone();
    let close: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(owner) = owner.upgrade() {
            close_editor(&owner, &identity);
        }
    });
    let cancel = close.clone();
    component.on_cancelled(move || cancel());
    component.window().on_close_requested(move || {
        close();
        slint::CloseRequestResponse::KeepWindowShown
    });
    component.set_ui_font_family(main.get_ui_font_family());
    component.invoke_apply_theme(windows.borrow().mode, windows.borrow().system_dark);
    component.set_can_submit(main.get_agent_connected() && !main.get_command_pending());
    apply_task_editor_state(&component, state);
    if component.show().is_err() {
        drop(component.hide());
        show_window_error(main, "The task editor could not be opened.");
        return;
    }
    let previous = windows
        .borrow_mut()
        .task_editor
        .replace(Window { component, token });
    if let Some(previous) = previous {
        drop(previous.component.hide());
    }
}

fn close_quick(windows: &Windows, token: &Rc<()>) {
    let removed = {
        let mut owner = windows.borrow_mut();
        if owner
            .quick_add
            .as_ref()
            .is_some_and(|window| Rc::ptr_eq(&window.token, token))
        {
            owner.quick_add.take()
        } else {
            None
        }
    };
    if let Some(window) = removed {
        drop(window.component.hide());
    }
}

fn close_editor(windows: &Windows, token: &Rc<()>) {
    let removed = {
        let mut owner = windows.borrow_mut();
        if owner
            .task_editor
            .as_ref()
            .is_some_and(|window| Rc::ptr_eq(&window.token, token))
        {
            owner.task_editor.take()
        } else {
            None
        }
    };
    if let Some(window) = removed {
        drop(window.component.hide());
    }
}

pub(super) fn close_all(windows: &Windows) {
    let (quick, editor) = {
        let mut owner = windows.borrow_mut();
        owner.pending = None;
        (owner.quick_add.take(), owner.task_editor.take())
    };
    if let Some(window) = quick {
        drop(window.component.hide());
    }
    if let Some(window) = editor {
        drop(window.component.hide());
    }
}

pub(super) fn finish_command(
    windows: &Windows,
    main: &MainWindow,
    command: &GuiCommand,
    outcome: CommandOutcome,
) {
    let pending = windows.borrow_mut().pending.take();
    match (pending, command) {
        (Some(PendingSubmission::Quick { token, submission }), GuiCommand::Application(intent))
            if matches!(intent.as_ref(), UiIntent::SubmitQuickAdd(_)) =>
        {
            let current = windows
                .borrow()
                .quick_add
                .as_ref()
                .filter(|window| Rc::ptr_eq(&window.token, &token))
                .map(|window| window.component.clone_strong());
            if let Some(window) = current {
                match &outcome {
                    CommandOutcome::Succeeded
                        if window.get_title_text().trim() == submission.title
                            && placement_from_destination(window.get_destination())
                                == Some(submission.placement) =>
                    {
                        close_quick(windows, &token);
                    }
                    CommandOutcome::Failed(error) => {
                        window.set_error_message(error.message.clone().into());
                    }
                    CommandOutcome::Succeeded => {}
                }
            }
        }
        (Some(PendingSubmission::Editor { token, draft }), GuiCommand::Application(intent))
            if matches!(intent.as_ref(), UiIntent::SubmitTaskEditor(_)) =>
        {
            let current = windows
                .borrow()
                .task_editor
                .as_ref()
                .filter(|window| Rc::ptr_eq(&window.token, &token))
                .map(|window| window.component.clone_strong());
            if let Some(window) = current {
                match &outcome {
                    CommandOutcome::Succeeded if EditorDraft::read(&window) == *draft => {
                        close_editor(windows, &token);
                    }
                    CommandOutcome::Failed(error) => {
                        window.set_error_message(error.message.clone().into());
                    }
                    CommandOutcome::Succeeded => {}
                }
            }
        }
        _ => {}
    }
    if let CommandOutcome::Failed(error) = outcome {
        show_error(main, &error.message);
    }
}
