# Quadrant Rewrite Specification

Status: **Authoritative baseline specification**  
Target implementation: **Rust + Slint**  
License: **GPL-3.0-only**

## 1. Product definition

Quadrant is a **local-first, cross-platform four-quadrant task management desktop application** designed around a short execution loop:

```text
Capture -> Classify -> Plan -> Focus -> Complete -> Review
```

The product should feel instantaneous and remain comfortable as an always-available desktop utility. It is not a web application packaged as a desktop executable.

The legacy .NET application is a reference implementation, not a compatibility target.

## 2. Rewrite goals

The rewrite SHALL:

- use Rust for all application logic
- use Slint for all application UI
- provide a handwritten Fluent visual language based primarily on `owu/wsl-dashboard`
- remain local-first
- use a clean new SQLite schema implemented through `rusqlite`
- preserve the useful product workflow of the existing Quadrant application
- centralize OS-specific behavior behind a platform boundary
- support Windows first while remaining structurally ready for Linux and macOS
- be event-driven at idle rather than continuously polling
- support native-feeling tray, shortcut, notification, startup, and single-instance behavior where each platform allows it

## 3. Explicit non-goals

The initial rewrite does NOT require:

- migration of the legacy .NET SQLite database
- a .NET compatibility layer
- cloud accounts or server sync
- collaborative/team tasks
- a web frontend
- mobile clients
- Electron/Tauri/webview shells
- formal performance-budget benchmark gates
- exact pixel replication of the old WPF UI

## 4. Source references

### 4.1 Legacy Quadrant

The current repository contains a .NET 10/WPF implementation. Its role during rewrite is:

- product behavior reference
- data/setting inventory reference
- old UI/interaction reference
- edge-case discovery reference

Its architecture and technologies are not binding on the rewrite.

### 4.2 wsl-dashboard

Primary UI upstream:

```text
Repository: https://github.com/owu/wsl-dashboard
Baseline commit: 948589a255a4bd8a3ff9c3de49e2e13109378fcd
Version at baseline: 0.11.0
License: GPL-3.0-only
```

Quadrant may directly copy and modify relevant Slint UI code because Quadrant will also be GPL-3.0-only. Derived files must preserve upstream attribution/SPDX notices.

The most important design reference is the handwritten sidebar/navigation system, theme globals, common controls, title bar, scrollbar, and modal patterns.

## 5. Main information architecture

### 5.1 Main window

Main sidebar navigation:

1. **Quadrants**
2. **Today**
3. **Focus**
4. **Review**

Footer navigation:

5. **Completed**
6. **Settings**
7. **About**

The Sidebar should inherit the interaction language of wsl-dashboard: compact/expanded modes, animated width transition, icon-first navigation, hover/selection states, top primary group, bottom utility group.

### 5.2 Quick Add

Quick Add is a dedicated low-friction capture surface.

Requirements:

- callable from the main app
- callable through a global shortcut where supported
- opens quickly and focuses the title field
- default destination is **Inbox** unless classification is explicitly supplied
- supports keyboard-first submission/cancel
- may expose a minimal subset of task properties without becoming the full editor

### 5.3 Task editor

The full task editor supports:

- title
- notes/description
- Inbox or quadrant assignment
- planned date
- due date/time
- reminder date/time
- recurrence
- completion state

Editing a task must be a domain/application operation, not a UI-bound mutable database record.

## 6. Core task model

### 6.1 Placement

A task is either:

- `Inbox`
- `Quadrant(Q1)` — important + urgent
- `Quadrant(Q2)` — important + not urgent
- `Quadrant(Q3)` — not important + urgent
- `Quadrant(Q4)` — not important + not urgent

The database may store a nullable quadrant code (`NULL = Inbox`) rather than duplicate boolean flags.

### 6.2 Task state

Recommended domain shape:

```text
Task
- id
- title
- notes
- quadrant: Option<Quadrant>
- status: Active | Completed
- planned_on: Option<LocalDate>
- due_at: Option<Zoned/UTC instant + local-zone semantics>
- reminder_at: Option<Zoned/UTC instant + local-zone semantics>
- recurrence: Option<RecurrenceRule>
- sort_key
- created_at
- updated_at
- completed_at
```

A concrete Rust representation may evolve, but domain invariants must remain explicit.

## 7. Task workflow

### 7.1 Capture

New tasks created by Quick Add should enter Inbox by default.

### 7.2 Classify

Inbox tasks can be moved into any quadrant. Tasks can be moved between quadrants at any time.

### 7.3 Plan

`planned_on` determines intentional scheduling for Today/future planning. `due_at` represents an external/internal deadline and must not be conflated with `planned_on`.

### 7.4 Complete

Completing a non-recurring task:

- updates task state atomically
- records completion time
- creates any completion/review event required for historical aggregation

Completing a recurring task:

- records the completion occurrence
- advances/generates the next occurrence according to the recurrence rule
- preserves review history
- reschedules reminders atomically with the task transition

### 7.5 Reopen

A completed task may be restored where product behavior permits. Reopening must maintain review/history correctness rather than simply toggling a boolean without event reconciliation.

## 8. Today

Today is an execution view, not a duplicate database.

It should derive tasks from rules such as:

- tasks explicitly planned for today
- overdue active tasks that require attention
- optionally tasks due today according to product settings

The exact inclusion rule must be centralized in application/domain logic and covered by deterministic tests.

## 9. Focus

Focus supports at least the existing product concepts:

- stopwatch mode
- Pomodoro mode
- optional association with a task
- start / pause / resume / finish / cancel transitions
- persistent completed focus sessions for Review

Timer truth must not depend on UI tick counts. Store time anchors and derive elapsed/remaining time from a clock so sleep, lag, or hidden windows do not accumulate drift.

The UI may repaint frequently while visible, but persistence/state correctness is based on timestamps and state transitions.

## 10. Review

Review is built from persistent task completion/focus history, not mutable UI counters.

It should support useful ranges such as recent day/week/month windows and report at least:

- completed tasks
- completion distribution by quadrant
- focused duration
- focus sessions
- trend/summary information carried over from the legacy product where valuable

Aggregation queries belong in the storage/application layer.

## 11. Completed

Completed provides historical task browsing and supported actions such as reopen/delete where product rules allow.

It must not load unbounded history into Slint models at once. Use pagination/windowing or bounded queries when the history becomes large.

## 12. Settings

Initial settings domains include:

- theme: System / Light / Dark
- launch at startup
- close/minimize to tray behavior
- global Quick Add hotkey
- Pomodoro durations/behavior
- notification/reminder preferences
- backup/import/export paths or behavior
- language/localization readiness if implemented
- update channel/checking behavior when updater work is enabled

Settings must be validated before persistence.

## 13. About

About should include:

- Quadrant name/version
- repository link
- GPL-3.0-only license
- third-party/upstream notices
- explicit acknowledgement of UI code/design derived from `owu/wsl-dashboard` where applicable

## 14. Architecture

Required workspace boundaries:

### `quadrant-domain`
Pure domain model and rules.

Owns:

- Task, Quadrant, recurrence value objects
- focus state machine/value objects
- domain validation/invariants
- domain events/value-level calculations

Must have no UI, database, Tokio, or OS dependencies.

### `quadrant-application`
Use cases and orchestration.

Owns:

- commands/queries/use cases
- application state projections
- reminder scheduling logic
- Today selection orchestration
- focus orchestration
- review orchestration
- repository/platform ports (or shared port module if preferred)
- typed application events

### `quadrant-storage`
Persistence adapter using `rusqlite`.

Owns:

- connection/configuration
- migrations
- repository implementations
- transactions
- review queries
- backup/restore mechanics

### `quadrant-platform`
All platform integration.

Owns:

- global shortcut
- tray
- notifications
- startup
- single instance/activation
- window/platform integration
- system theme/accent observation
- platform directories and shell-open behavior

OS-specific modules are selected with `cfg(...)`.

### `quadrant-ui`
Rust-to-Slint adapter.

Owns:

- Slint component generation/inclusion
- Slint DTO/model conversion
- UI callback binding
- UI state projection/update scheduling
- no SQL
- no direct native OS implementation

### `quadrant-app`
Binary/composition root.

Owns:

- runtime creation
- concrete dependency wiring
- startup sequence
- shutdown coordination
- top-level error reporting

It should remain small.

## 15. Concurrency and runtime model

Quadrant uses one application-owned asynchronous runtime for background orchestration.

Rules:

- Slint owns the UI event loop/thread.
- UI mutations happen on the Slint event loop.
- network/update/timer/platform asynchronous operations run through the application runtime.
- blocking database work never runs on the UI thread.
- no feature may create its own private runtime.
- no periodic "check everything" loop is allowed for reminders.

### 15.1 Reminder scheduler

The scheduler maintains the next relevant deadline.

Conceptual behavior:

```text
Load active reminder schedule
        |
        v
Determine nearest due reminder
        |
        v
Sleep until deadline OR receive schedule-change signal
        |
        +--> deadline: fire reminder and compute next schedule
        |
        +--> mutation: recompute nearest deadline
```

Task create/edit/complete/delete operations that change reminder state must notify the scheduler immediately.

## 16. Storage specification

### 16.1 Database

Use SQLite via **`rusqlite`**.

There is **no requirement to preserve or migrate the legacy schema/data**.

Initial database should use:

- foreign keys enabled
- schema migrations
- explicit transactions
- a sensible busy timeout
- WAL where appropriate for desktop access patterns

### 16.2 Initial tables

Recommended baseline:

```text
schema_migrations
settings
tasks
task_completion_events
focus_sessions
```

Optional normalized tables may be added when justified by real query/invariant needs.

#### `tasks`

Suggested columns:

```text
id TEXT PRIMARY KEY
title TEXT NOT NULL
notes TEXT NOT NULL DEFAULT ''
quadrant INTEGER NULL
status INTEGER NOT NULL
planned_on TEXT NULL
due_at_utc INTEGER NULL
due_tz TEXT NULL
reminder_at_utc INTEGER NULL
reminder_tz TEXT NULL
recurrence_json TEXT NULL
sort_key INTEGER NOT NULL
created_at_utc INTEGER NOT NULL
updated_at_utc INTEGER NOT NULL
completed_at_utc INTEGER NULL
```

The final exact schema should be encoded by migrations and documented in `docs/03_DOMAIN_DATA.md`.

#### `task_completion_events`

Keeps immutable occurrence/history information needed for review and recurrence correctness.

#### `focus_sessions`

Stores completed/record-worthy focus sessions independently from the live UI timer.

#### `settings`

A small key/value JSON table is acceptable for heterogeneous settings if every key is typed/validated at the Rust boundary.

## 17. Backups

Backup must create a consistent snapshot, not copy a potentially mid-write database file blindly.

The storage layer should use an SQLite-consistent backup mechanism and include enough metadata to detect application/schema version.

Because the legacy app has not shipped, legacy backup import is not required.

## 18. UI specification

### 18.1 Direct wsl-dashboard derivation

Quadrant should intentionally derive/adapt the upstream Slint Fluent implementation rather than re-create Fluent from scratch.

Priority reuse/adaptation targets:

1. Sidebar layout and interaction
2. Theme/color globals
3. Sidebar item/common button states
4. Title bar patterns
5. Scrollbar
6. Modal/dialog visual system
7. Form controls/cards where suitable

Remove WSL-specific product semantics and rename components cleanly for Quadrant.

### 18.2 Sidebar target

Use the wsl-dashboard pattern as the behavioral baseline:

- compact width around 54 px
- expanded width around 200 px
- animated width transition around 200 ms
- hamburger/toggle control
- primary navigation group at top
- spacer
- utility navigation group at bottom
- selected/hover/accent states controlled by theme

Quadrant may adjust dimensions only when needed for its content, but the upstream visual character should remain recognizable.

### 18.3 Theme

Maintain centralized Slint theme globals for:

- background/content/card surfaces
- text primary/secondary
- borders
- hover/selected backgrounds
- accent
- scrollbar
- inputs
- icon semantic colors
- typography/icon source

Support System/Light/Dark.

Windows may use native backdrop effects where practical. Linux/macOS must have graceful equivalents and must not depend on Win32 visual APIs.

### 18.4 Icons

The upstream currently uses `Segoe Fluent Icons` glyphs. Quadrant is cross-platform, so essential icons must be provided through a platform-neutral asset abstraction.

Preferred direction:

- use legally distributable Fluent-style SVG assets
- convert/embed as required by Slint build
- centralize icon identifiers so UI components do not hard-code OS font glyph assumptions
- record provenance/license for every vendored icon set

## 19. Platform behavior

### Windows parity target

Windows implementation should eventually cover the useful legacy capabilities:

- single instance
- global Quick Add hotkey
- system tray
- startup registration
- desktop notifications
- close/minimize-to-tray policy
- activation forwarding from second launch
- theme/backdrop integration where appropriate

### Linux/macOS

The same application ports should have platform implementations or explicit capability reporting. Unsupported capabilities must degrade cleanly rather than leaking conditional checks throughout domain/UI code.

## 20. Update system

The legacy project had a separate updater launcher. The Rust rewrite should keep update logic architecturally isolated.

Do not make self-update a prerequisite for early core milestones. When implemented, it should live as a dedicated application/platform service or small companion binary if the target packaging model requires replacement outside the running process.

## 21. Error handling

- Domain validation errors are typed and user-presentable.
- Storage errors are wrapped with operation context.
- Platform capability/permission failures are explicit.
- UI receives stable user-facing error states/messages, not raw Rust debug output.
- Unrecoverable startup errors should produce a useful diagnostic path/log.

## 22. Logging

Use structured Rust logging/tracing.

Do not log task content/notes by default when not necessary. Keep logs useful for startup, migration, platform registration, reminder scheduling, update flow, and errors.

## 23. Testing

Correctness tests are required even though a formal performance budget is not.

Minimum suite:

- domain task invariants
- quadrant/inbox moves
- Today rules
- recurrence transitions
- reminder schedule recomputation
- focus state machine and clock-based calculations
- review aggregation
- database migration from empty DB through latest schema
- transaction rollback on failed compound mutations
- settings validation

UI screenshot/golden testing may be added where useful but is not a blocking architectural requirement.

## 24. Milestone order

### M0 — Repository rewrite foundation

- install root `AGENTS.md`, `SPEC.md`, `docs/`
- switch license to GPL-3.0-only
- create Rust workspace
- establish Slint build
- preserve legacy reference in a temporary isolated location if still needed
- record upstream wsl-dashboard source snapshot and derived-file mapping

### M1 — UI shell

- main Slint window
- wsl-dashboard-derived Sidebar
- title bar/theme/common controls
- main navigation page switching
- System/Light/Dark foundation
- cross-platform icon abstraction

### M2 — Domain + new rusqlite storage

- Task/Quadrant model
- migrations
- repositories
- Inbox/Quadrants CRUD
- task editor and Quick Add persistence

### M3 — Today + reminders + platform shell

- Today rules
- reminder scheduler
- notifications
- global shortcut
- tray
- single instance
- startup setting

### M4 — Focus

- stopwatch
- Pomodoro
- task association
- persistent focus sessions

### M5 — Review + Completed

- completion event history
- review aggregation
- completed history/reopen

### M6 — Settings/backup/release hardening

- persistent settings
- consistent backups
- update architecture
- packaging/release work
- remove remaining legacy .NET/C# source from final tree once reference value is exhausted

## 25. Final rewrite completion criteria

The rewrite is complete when:

- all supported user-facing legacy workflows have a Rust/Slint replacement or an explicitly accepted redesign
- the running app has no .NET dependency
- the repository's active application code contains no C#/XAML/.NET project files
- new persistence is entirely owned by Rust through `rusqlite`
- Windows-specific calls are isolated to the platform crate
- the UI uses the wsl-dashboard-derived Fluent component system with correct GPL provenance
- Windows is functionally usable and Linux/macOS architecture is not blocked by Windows coupling
- persistent project memory accurately describes the final architecture
