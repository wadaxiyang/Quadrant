# 04 — Platform & Runtime Memory

## Core rule

All OS-specific behavior belongs in **`quadrant-platform`**. Other crates depend on ports/capabilities, not Win32/macOS/Linux APIs.

## Runtime ownership

`quadrant-app` creates and owns the application's asynchronous runtime once.

Do not create separate Tokio runtimes per subsystem.

The runtime is used for:

- reminder scheduling
- update/network work
- platform event streams when asynchronous
- application orchestration that must not block Slint

Blocking database work is explicitly isolated away from the UI event loop.

## Slint boundary

Slint event loop/thread owns Slint component state.

Background code must schedule UI updates through Slint's event-loop-safe mechanism. Never mutate Slint UI objects from arbitrary runtime threads.

## Reminder scheduler contract

The scheduler is a dedicated application service, not a timer loop in a page.

Inputs/signals:

- application startup / initial schedule load
- task created
- task edited
- task completed/reopened
- task deleted
- reminder settings changed
- system resume/time-change/timezone-change when relevant

Behavior:

1. determine next scheduled reminder
2. wait until deadline or schedule-change signal
3. fire through notification platform port
4. update fired/next recurrence state where applicable
5. recompute next deadline

No periodic SQL polling.

## Single-instance contract

Second launch should not create a second independent app instance when single-instance mode is supported/desired.

Activation payload may include:

- show main window
- open Quick Add
- open/edit a specific task in future extensibility

Platform implementation forwards activation to the primary instance through an appropriate local IPC primitive.

UI/application handles the activation intent; platform layer handles OS-specific transport.

## Global hotkey contract

Application defines semantic action `QuickAdd` and a validated hotkey setting.

Platform layer:

- registers/unregisters
- reports conflict/failure
- emits semantic activation event

UI does not call Win32 `RegisterHotKey` directly.

## Tray contract

Tray supports at least:

- show/open Quadrant
- Quick Add
- quit

Potential Focus controls may be added only if product design requires them.

Close/minimize-to-tray behavior is a setting/application policy. The platform crate implements the OS mechanics.

## Notifications

Application layer decides **what** reminder should be shown.

Platform layer decides **how** to present a native notification.

Notification callbacks/actions, if supported, should map back to typed activation intents.

## Startup/autostart

Expose a capability-oriented API:

```text
is_supported
get_status
enable
disable
```

Windows implementation may use the best packaging-aware method; Linux/macOS use their native conventions.

Do not leak registry/package APIs into settings UI.

## Theme/accent

Platform can report:

- system dark/light preference
- accent color if reliable/supported

Slint theme consumes normalized values.

If unsupported, use Quadrant defaults. Do not make theme startup fail because accent detection failed.

## Native window/backdrop

Windows-specific Mica/backdrop/title-bar enhancements are optional platform capabilities layered over a complete Slint UI.

The UI must remain visually correct without native backdrop support.

## Cross-platform capability model

Avoid `if windows { ... }` throughout the UI.

Expose capability state to application/UI such as:

```text
PlatformCapabilities {
  global_hotkey: bool,
  tray: bool,
  autostart: bool,
  native_notifications: bool,
  native_backdrop: bool,
}
```

Settings can hide/disable unsupported controls cleanly.

## Windows implementation boundary

Windows-specific source can live under:

```text
crates/quadrant-platform/src/windows/
```

This is the only place expected to use the `windows` crate/unsafe Win32 FFI.

## Linux/macOS

Implement the same semantic ports. If a feature is unavailable in an early platform milestone, report it through capabilities; do not compile broken stubs that panic when called.

## Shutdown

Shutdown sequence should deliberately:

1. stop accepting new UI intents
2. unregister hotkeys/tray/platform hooks as needed
3. signal long-lived application services
4. finish/flush required storage/log work
5. close UI/runtime cleanly

Do not rely on abrupt process termination for normal Quit.
