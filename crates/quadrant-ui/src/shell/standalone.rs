// SPDX-License-Identifier: GPL-3.0-only
//! A disposable capture host: no `MainWindow`, editor, projections or Focus timer.

use super::{ClientUpdateSink, bind_quick_add, placement_from_destination, to_slint_theme_mode};
use crate::QuickAddWindow;
use quadrant_application::{QuickAddSubmission, SystemTheme, UiIntent};
use quadrant_protocol::{
    AppSnapshot, ClientUpdate, CommandOutcome, ConnectionState, GuiCommand, ServerEvent,
};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct State {
    ready: bool,
    received_status: bool,
    closed: bool,
    pending: Option<QuickAddSubmission>,
}

pub struct QuickAddShell {
    pub(super) window: QuickAddWindow,
    mailbox: Arc<Mutex<Option<ClientUpdate>>>,
}

impl QuickAddShell {
    pub fn new(
        snapshot: &AppSnapshot,
        on_command: impl Fn(GuiCommand) -> bool + 'static,
    ) -> Result<Self, slint::PlatformError> {
        let window = QuickAddWindow::new()?;
        #[cfg(target_os = "windows")]
        window.set_ui_font_family("Segoe UI Variable Text".into());
        apply_snapshot(&window, snapshot);
        window.set_can_submit(false);
        window.set_error_message("Connecting to background service…".into());
        let state = Rc::new(RefCell::new(State::default()));
        let mailbox = Arc::new(Mutex::new(None));

        let weak = window.as_weak();
        let submission_state = state.clone();
        bind_quick_add(
            &window,
            Rc::new(move |intent| {
                let Some(window) = weak.upgrade() else {
                    return;
                };
                let UiIntent::SubmitQuickAdd(submission) = &intent else {
                    return;
                };
                if !submission_state.borrow().ready || submission_state.borrow().closed {
                    window.set_error_message(
                        "Wait for the current operation or reconnect before trying again.".into(),
                    );
                    return;
                }
                let submission = submission.clone();
                if !on_command(intent.into()) {
                    window.set_error_message(
                        "Wait for the current operation or reconnect before trying again.".into(),
                    );
                    return;
                }
                let mut state = submission_state.borrow_mut();
                state.ready = false;
                state.pending = Some(submission);
                window.set_can_submit(false);
            }),
        );

        let cancel = state.clone();
        window.on_cancelled(move || close(&mut cancel.borrow_mut()));
        let native_close = state.clone();
        window.window().on_close_requested(move || {
            close(&mut native_close.borrow_mut());
            slint::CloseRequestResponse::KeepWindowShown
        });

        let dispatch_mailbox = mailbox.clone();
        let weak = window.as_weak();
        window.on_dispatch_client_update(move || {
            let update = dispatch_mailbox
                .lock()
                .ok()
                .and_then(|mut slot| slot.take());
            if let Some(window) = weak.upgrade()
                && let Some(update) = update
            {
                apply(&window, &mut state.borrow_mut(), update);
            }
        });
        Ok(Self { window, mailbox })
    }

    pub fn update_sink(&self) -> ClientUpdateSink {
        let weak = self.window.as_weak();
        let mailbox = self.mailbox.clone();
        Arc::new(move |update| {
            let mailbox = mailbox.clone();
            drop(weak.upgrade_in_event_loop(move |window| {
                if let Ok(mut slot) = mailbox.lock() {
                    *slot = Some(update);
                }
                window.invoke_dispatch_client_update();
            }));
        })
    }

    pub fn run(self) -> Result<(), slint::PlatformError> {
        self.window.show()?;
        slint::run_event_loop_until_quit()
    }
}

impl Drop for QuickAddShell {
    fn drop(&mut self) {
        drop(self.window.hide());
    }
}

fn close(state: &mut State) {
    if !state.closed {
        state.closed = true;
        state.ready = false;
        state.pending = None;
        drop(slint::quit_event_loop());
    }
}

fn apply_snapshot(window: &QuickAddWindow, snapshot: &AppSnapshot) {
    window.invoke_apply_theme(
        to_slint_theme_mode(snapshot.theme_mode),
        snapshot.system_theme == SystemTheme::Dark,
    );
}

fn apply(window: &QuickAddWindow, state: &mut State, update: ClientUpdate) {
    if state.closed {
        return;
    }
    match update {
        ClientUpdate::Snapshot(snapshot) => apply_snapshot(window, &snapshot),
        ClientUpdate::Connection {
            state: connection,
            message,
        } => {
            state.ready = connection == ConnectionState::Ready;
            if !state.received_status && state.ready {
                window.set_error_message(slint::SharedString::default());
            }
            state.received_status = true;
            window.set_can_submit(state.ready);
            if !matches!(connection, ConnectionState::Ready | ConnectionState::Busy) {
                state.pending = None;
                window.set_error_message(if message.is_empty() {
                    "Not connected. Check the task list before retrying unconfirmed changes.".into()
                } else {
                    message.into()
                });
            }
        }
        ClientUpdate::CommandFinished { command, outcome } => {
            if let Some(submission) = state.pending.take()
                && matches!(command, GuiCommand::Application(ref intent) if matches!(intent.as_ref(), UiIntent::SubmitQuickAdd(_)))
            {
                match outcome {
                    CommandOutcome::Succeeded
                        if window.get_title_text().trim() == submission.title
                            && placement_from_destination(window.get_destination())
                                == Some(submission.placement) =>
                    {
                        close(state);
                    }
                    CommandOutcome::Failed(error) => window.set_error_message(error.message.into()),
                    CommandOutcome::Succeeded => {}
                }
            }
        }
        ClientUpdate::Event(event) => match event {
            ServerEvent::OpenQuickAdd => {
                window.window().set_minimized(false);
                if window.show().is_err() {
                    window.set_error_message("Quick Add could not be opened.".into());
                }
            }
            ServerEvent::ThemeChanged {
                theme_mode,
                system_theme,
            } => window.invoke_apply_theme(
                to_slint_theme_mode(theme_mode),
                system_theme == SystemTheme::Dark,
            ),
            ServerEvent::ExitGui | ServerEvent::AgentShuttingDown => close(state),
            // Other projections belong to Main; never construct it for a push.
            _ => {}
        },
    }
}
