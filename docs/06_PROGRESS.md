# 06 — Progress

Legend: `[ ]` not started, `[-]` in progress, `[x]` complete, `[!]` blocked.

## M0 — Rewrite foundation

- [x] Define rewrite direction: Rust + Slint
- [x] Define GPL/wsl-dashboard UI derivation policy
- [x] Define new `rusqlite` persistence policy
- [x] Define Codex memory documentation system
- [x] Install these bootstrap docs at repository root
- [x] Change repository license from MIT to GPL-3.0-only
- [x] Create Rust 2024 workspace
- [x] Create crate skeletons: domain/application/storage/platform/ui/app
- [x] Establish Slint build pipeline
- [x] Isolate legacy .NET source at `legacy/dotnet-reference/` as read-only reference
- [x] Record exact legacy feature inventory for the first task workflow in source map
- [x] Vendor/adapt first wsl-dashboard UI primitives with provenance
- [x] Add baseline CI for fmt/clippy/test and target builds

## M1 — UI shell

- [x] Main Slint window shell
- [x] wsl-dashboard-derived Sidebar
- [x] Quadrants/Today/Focus/Review route switching
- [x] Completed/Settings/About footer route switching
- [ ] title bar
- [x] theme globals
- [ ] System/Light/Dark
- [x] cross-platform icon abstraction
- [ ] shared dialogs/modals/toast pattern
- [ ] Quick Add shell

## M2 — Domain + storage

- [ ] Task/Quadrant/Inbox domain model
- [ ] recurrence value model
- [ ] SQLite migrations
- [ ] `rusqlite` connection configuration
- [ ] task repositories
- [ ] settings repository
- [ ] Quadrants CRUD
- [ ] task editor persistence
- [ ] Quick Add persistence
- [ ] task ordering

## M3 — Today + reminders + platform

- [ ] Today derivation rules
- [ ] reminder scheduler
- [ ] native notifications
- [ ] global Quick Add hotkey
- [ ] tray
- [ ] single-instance activation
- [ ] startup/autostart
- [ ] close/minimize-to-tray settings

## M4 — Focus

- [ ] focus state machine
- [ ] stopwatch
- [ ] Pomodoro
- [ ] task association
- [ ] focus session persistence
- [ ] focus settings

## M5 — Review + Completed

- [ ] completion events
- [ ] Review queries/aggregations
- [ ] Review UI
- [ ] Completed history
- [ ] reopen policy/implementation

## M6 — Hardening/release

- [ ] backup/restore
- [ ] updater architecture
- [ ] packaging Windows
- [ ] packaging Linux
- [ ] packaging macOS
- [ ] third-party notices finalization
- [ ] remove active legacy .NET/C#/XAML/project files
- [ ] final documentation refresh
