//! Windows-only desktop integration.

mod autostart;
mod theme;

pub(crate) fn set_autostart(
    enabled: bool,
    start_hidden: bool,
) -> Result<(), quadrant_application::AutostartError> {
    autostart::set_enabled(enabled, start_hidden)
}

pub(crate) fn current_system_theme() -> quadrant_application::SystemTheme {
    theme::current_system_theme()
}

pub(crate) fn show_startup_error(detail: &str) {
    use windows::{
        Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
        core::HSTRING,
    };

    let message = HSTRING::from(format!(
        "Quadrant could not start. Your existing data was not intentionally deleted.\n\n{detail}\n\nCheck the data directory and restore/recovery files before trying again."
    ));
    let title = HSTRING::from("Quadrant startup error");
    // SAFETY: both HSTRING values own valid, NUL-terminated UTF-16 buffers for
    // the duration of this modal call; no window owner is required at startup.
    unsafe {
        let _ = MessageBoxW(None, &message, &title, MB_OK | MB_ICONERROR);
    }
}

use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use quadrant_application::DesktopEvent;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
        TranslateMessage, WM_QUIT,
    },
};

use crate::{DesktopEventSink, PlatformCapabilities, PlatformIntegrationError};

#[derive(Debug)]
pub(super) struct WindowsDesktopIntegration {
    thread_id: u32,
    worker: Option<thread::JoinHandle<()>>,
}

impl WindowsDesktopIntegration {
    pub(super) fn start(
        sink: DesktopEventSink,
    ) -> Result<(Self, PlatformCapabilities), PlatformIntegrationError> {
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("quadrant-platform".to_owned())
            .spawn(move || run_platform_thread(&sink, &ready_sender))
            .map_err(PlatformIntegrationError::new)?;
        let ready = ready_receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(PlatformIntegrationError::new)?;
        Ok((
            Self {
                thread_id: ready.thread_id,
                worker: Some(worker),
            },
            ready.capabilities,
        ))
    }

    pub(super) fn shutdown(mut self) {
        // SAFETY: `thread_id` comes from the live platform thread. `PeekMessageW`
        // creates its queue before readiness is reported, and WM_QUIT is the
        // documented way to end this GetMessageW loop.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ThreadReady {
    thread_id: u32,
    capabilities: PlatformCapabilities,
}

fn run_platform_thread(sink: &DesktopEventSink, ready_sender: &mpsc::SyncSender<ThreadReady>) {
    // SAFETY: called on the platform thread to record its OS identity.
    let thread_id = unsafe { GetCurrentThreadId() };
    // SAFETY: a no-remove peek is the documented way to ensure this thread has
    // a message queue before another thread can post WM_QUIT.
    unsafe {
        let mut message = MSG::default();
        let _ = PeekMessageW(&raw mut message, None, 0, 0, PM_NOREMOVE);
    }

    let quick_add_hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyQ);
    let hotkey_manager = GlobalHotKeyManager::new().ok();
    let hotkey_registered = hotkey_manager
        .as_ref()
        .is_some_and(|manager| manager.register(quick_add_hotkey).is_ok());
    if hotkey_registered {
        let hotkey_sink = Arc::clone(sink);
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == quick_add_hotkey.id() && event.state == HotKeyState::Pressed {
                hotkey_sink(DesktopEvent::OpenQuickAdd);
            }
        }));
    }

    let tray_icon = create_tray_icon(Arc::clone(sink)).ok();
    let capabilities = PlatformCapabilities {
        global_hotkey: hotkey_registered,
        tray: tray_icon.is_some(),
        autostart: true,
        native_notifications: true,
        native_backdrop: false,
        single_instance: true,
    };
    let _ = ready_sender.send(ThreadReady {
        thread_id,
        capabilities,
    });

    run_message_loop();

    if hotkey_registered && let Some(manager) = hotkey_manager.as_ref() {
        let _ = manager.unregister(quick_add_hotkey);
    }
    drop(tray_icon);
}

fn create_tray_icon(sink: DesktopEventSink) -> Result<TrayIcon, PlatformIntegrationError> {
    let menu = Menu::new();
    let quick_add = MenuItem::with_id("quick-add", "Quick Add", true, None);
    let show = MenuItem::with_id("show-main", "Show Quadrant", true, None);
    let separator = PredefinedMenuItem::separator();
    let exit = MenuItem::with_id("exit", "Exit", true, None);
    menu.append_items(&[&quick_add, &show, &separator, &exit])
        .map_err(PlatformIntegrationError::new)?;

    install_menu_handler(
        sink.clone(),
        quick_add.id().clone(),
        show.id().clone(),
        exit.id().clone(),
    );
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            sink(DesktopEvent::ShowMainWindow);
        }
    }));

    TrayIconBuilder::new()
        .with_tooltip("Quadrant")
        .with_menu(Box::new(menu))
        // Left release activates the GUI above; reserve the menu for right click.
        .with_menu_on_left_click(false)
        .with_icon(quadrant_icon()?)
        .build()
        .map_err(PlatformIntegrationError::new)
}

fn install_menu_handler(sink: DesktopEventSink, quick_add: MenuId, show: MenuId, exit: MenuId) {
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == quick_add {
            sink(DesktopEvent::OpenQuickAdd);
        } else if event.id == show {
            sink(DesktopEvent::ShowMainWindow);
        } else if event.id == exit {
            sink(DesktopEvent::ExitRequested);
        }
    }));
}

fn quadrant_icon() -> Result<Icon, PlatformIntegrationError> {
    const SIZE: u32 = 32;
    let rgba = include_bytes!("../../../../assets/branding/quadrant-32.rgba").to_vec();
    Icon::from_rgba(rgba, SIZE, SIZE).map_err(PlatformIntegrationError::new)
}

fn run_message_loop() {
    // SAFETY: this is the dedicated Windows platform thread. The MSG value is
    // initialized, and the loop follows the standard GetMessage/Translate/Dispatch pattern.
    unsafe {
        let mut message = MSG::default();
        loop {
            let status = GetMessageW(&raw mut message, None, 0, 0).0;
            if status <= 0 {
                break;
            }
            let _ = TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::quadrant_icon;

    #[test]
    fn generated_tray_icon_is_valid() {
        quadrant_icon().expect("valid RGBA tray icon");
    }
}
