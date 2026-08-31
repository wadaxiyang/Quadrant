# AGENTS.md — Quadrant

This file is the primary operating contract for Codex and all coding agents working in this repository.

## 1. Mission

Quadrant is being **fully rewritten** as a cross-platform, local-first desktop task manager using **Rust + Slint**.

The rewrite is not a C# port and not a compatibility exercise. The legacy .NET 10/WPF application exists only as a **read-only behavioral and visual reference** until its behavior has been captured in the Rust implementation and project memory.

The target is a genuinely lightweight, fast, always-available desktop application with a native-feeling Fluent interface.

## 2. Non-negotiable project rules

1. **Rust only for the new application.** Do not add C#, F#, .NET, WPF, WinForms, WinUI, Avalonia, MAUI, Tauri, Electron, or a webview-based UI layer.
2. **Slint is the UI framework.** UI structure and visual behavior should be derived primarily from `owu/wsl-dashboard`, especially its handwritten Fluent-style Slint components.
3. **GPL-3.0-only is the project license.** Directly adapting GPL-3.0-only UI code from `owu/wsl-dashboard` is allowed. Preserve upstream SPDX and copyright notices on derived files.
4. **No legacy database compatibility is required.** The old application has not shipped. Design a clean new database schema for Rust and implement it with `rusqlite`.
5. **No C# compatibility layer.** Never invoke the old executable, host CLR, use COM/.NET interop as an application layer, or wrap legacy services.
6. **Cross-platform architecture is first-class.** Windows is the first parity target, but domain/application/storage/UI code must not depend directly on Win32. OS integration belongs in the platform boundary.
7. **All Windows-native behavior must be centralized in the platform crate.** No direct Windows API use elsewhere.
8. **Do not introduce artificial performance gates or benchmark work unless requested.** Correct architecture, low idle work, dependency discipline, and event-driven behavior are required; a formal Performance Budget is not.
9. **Do not prematurely micro-optimize Slint Cargo features.** Start from a normal supported Slint configuration. Backend/renderer feature trimming is not an architectural goal. UI parity and cross-platform correctness take priority.
10. **Do not simplify architecture with ad-hoc background threads.** Use the application runtime, explicit services, typed messages/events, and clear ownership boundaries described in `SPEC.md` and `docs/01_ARCHITECTURE.md`.

## 3. Instruction precedence

When instructions conflict, use this order:

1. Explicit user instruction in the current conversation/task.
2. This `AGENTS.md`.
3. `SPEC.md`.
4. Accepted decisions in `docs/05_DECISIONS.md`.
5. Current state in `docs/00_PROJECT_MEMORY.md` and `docs/06_PROGRESS.md`.
6. Other `docs/` memory files.
7. Legacy C# implementation.
8. Upstream `wsl-dashboard` behavior outside the parts intentionally adopted by Quadrant.

Never silently override a higher-priority source. If a durable decision changes, update the decision log and memory files.

## 4. Mandatory reading protocol for every new Codex session

Before modifying code, read the following in order:

1. `AGENTS.md`
2. `SPEC.md`
3. `docs/00_PROJECT_MEMORY.md`
4. `docs/05_DECISIONS.md` — at minimum the latest active decisions
5. `docs/06_PROGRESS.md`
6. `docs/08_SESSION_HANDOFF.md`
7. Task-specific memory:
   - UI work → `docs/02_UI_UPSTREAM.md`
   - storage/domain work → `docs/03_DOMAIN_DATA.md`
   - runtime/platform work → `docs/04_PLATFORM_RUNTIME.md`
   - licensing/upstream copying → `docs/09_SOURCE_MAP.md` and `docs/10_LICENSE_RELEASE.md`

Do **not** begin from repository code alone when these files are present. They are the persistent project memory across Codex windows.

## 5. Mandatory memory write-back protocol

Before ending any session that changed code or project decisions:

- **Always update `docs/08_SESSION_HANDOFF.md`.** Replace its working-state sections with a concise, exact handoff for the next agent.
- Update `docs/06_PROGRESS.md` when milestone/task status changed.
- Update `docs/07_BACKLOG.md` when priorities, dependencies, or newly discovered work changed.
- Append to `docs/05_DECISIONS.md` when an architectural/product decision was made or superseded.
- Update `docs/00_PROJECT_MEMORY.md` only when stable project truth changed. Keep it concise; it is not a diary.
- Update `docs/09_SOURCE_MAP.md` whenever legacy behavior or upstream GPL files are mapped to new Rust/Slint files.
- Update the relevant specialized memory file when its subsystem changed materially.

Memory files must contain **decisions and current state, not raw terminal logs**. Record exact failing commands only when they remain unresolved and are necessary for the next session.

## 6. Required repository architecture

The target workspace is:

```text
Quadrant/
├─ AGENTS.md
├─ SPEC.md
├─ Cargo.toml
├─ Cargo.lock
├─ LICENSE
├─ crates/
│  ├─ quadrant-domain/
│  ├─ quadrant-application/
│  ├─ quadrant-storage/
│  ├─ quadrant-platform/
│  ├─ quadrant-ui/
│  └─ quadrant-app/
├─ ui/
│  ├─ app.slint
│  ├─ theme.slint
│  ├─ constants.slint
│  ├─ components/
│  └─ views/
├─ assets/
├─ migrations/
├─ docs/
└─ legacy/                 # temporary reference only; must disappear before final rewrite completion
```

`quadrant-app` is the composition root. It wires concrete implementations together; it must not become a second domain layer.

## 7. Dependency direction

Allowed high-level dependency direction:

```text
quadrant-app
  ├─> quadrant-ui
  ├─> quadrant-application
  ├─> quadrant-storage
  └─> quadrant-platform

quadrant-ui ---------> quadrant-application/domain DTOs
quadrant-application -> quadrant-domain
quadrant-storage ----> quadrant-domain/application ports
quadrant-platform ---> quadrant-application ports
quadrant-domain ------> no project crate
```

The domain crate must not depend on Slint, SQLite, Tokio, `windows`, tray libraries, notification libraries, or OS APIs.

## 8. Runtime and state model

Use a deliberate two-world model:

- **Slint UI thread:** owns Slint components/models and performs UI mutations only.
- **Application runtime:** owns asynchronous application work, timers, update checks, platform events, and orchestration.
- **Storage boundary:** all `rusqlite` work stays outside the UI thread and is exposed through repository/application services.

The preferred flow is unidirectional:

```text
Slint callback
  -> typed UiIntent/ApplicationCommand
  -> application service/use case
  -> repository/platform service
  -> ApplicationEvent / updated state
  -> UI adapter
  -> Slint model update on UI event loop
```

Do not let `.slint` callbacks directly run SQL or OS APIs.

Use one application-owned async runtime rather than scattered per-feature runtimes. Blocking work must be isolated explicitly. Reminder scheduling must be event/deadline driven, not periodic database polling.

## 9. Storage rules

- Use **`rusqlite`** as the SQLite API.
- The schema is new; **do not implement import/migration from the legacy C# database** unless the user later asks.
- Use embedded, versioned SQL migrations.
- Enable and configure SQLite intentionally (foreign keys, busy timeout, WAL policy where appropriate).
- Keep SQL in the storage crate/migration files, not in UI or platform code.
- Every persistent domain mutation that must remain coherent must use a transaction.

## 10. UI rules

### 10.1 Upstream visual baseline

The main UI reference is:

- Repository: `https://github.com/owu/wsl-dashboard`
- Baseline snapshot for this rewrite plan: commit `948589a255a4bd8a3ff9c3de49e2e13109378fcd` (`v0.11.0`, 2026-08-25)
- License: GPL-3.0-only

High-value upstream files include:

- `src/ui/components/sidebar.slint`
- `src/ui/components/common.slint`
- `src/ui/components/title_bar.slint`
- `src/ui/components/scrollbar.slint`
- `src/ui/components/modal_manager.slint`
- `src/ui/theme.slint`
- `src/ui/constants.slint`

The Quadrant sidebar should be **directly based on the upstream handwritten Fluent sidebar**, then adapted to Quadrant navigation and branding.

### 10.2 Quadrant navigation

Primary sidebar:

- Quadrants
- Today
- Focus
- Review

Footer:

- Completed
- Settings
- About

Quick Add is a separate lightweight window/surface opened by global shortcut and application actions.

### 10.3 Cross-platform icon rule

Do not make `Segoe Fluent Icons` a hard runtime requirement. It is Windows-specific. Preserve the upstream visual language by routing icons through a Quadrant icon abstraction and vendoring/distributing legally usable Fluent-style assets for all platforms. Record asset provenance in `docs/09_SOURCE_MAP.md`.

### 10.4 UI separation

`.slint` files own layout, visual states, lightweight presentation logic, and callbacks. Rust owns business logic, persistence, scheduling, OS integration, and long-running operations.

Do not rebuild a WPF/MVVM object graph inside Rust. Prefer explicit application state and typed intents/events.

## 11. Platform rules

`quadrant-platform` is the only crate allowed to contain OS-specific implementation details.

It owns abstractions and implementations for:

- global hotkeys
- tray/status item
- notifications
- startup/autostart
- single-instance behavior and activation forwarding
- native window integration/backdrop/theme hints
- opening paths/URLs in the OS
- system theme/accent observation where supported
- platform directories
- power/session events if required by reminder/focus correctness

Prefer portable crates when they provide correct behavior. Put unavoidable Win32 code under `cfg(target_os = "windows")` inside `quadrant-platform`.

## 12. Legacy C# rules

The legacy .NET 10/WPF source is located at **`legacy/dotnet-reference/`**. Treat that entire directory as read-only unless the user explicitly requests a legacy-source change. Its `src/`, `Tests/`, and `installer/` trees may be inspected to recover behavior, edge cases, visual details, packaging expectations, and test scenarios.

Legacy C# exists to answer questions such as:

- What did this feature do?
- What fields/settings existed?
- What did the old screen look like?
- Which edge cases were already handled?

It is **not** the new architecture specification.

When legacy behavior is understood, record it in the appropriate `docs/` file so future agents do not need to rediscover it.

Do not modify legacy C# unless the user explicitly requests it. Do not add new features to it. The final rewritten repository must not require `.NET`, `.csproj`, `.sln`, XAML, or C# source files.

## 13. Coding standards

- Rust edition: **2024**.
- Prefer stable Rust.
- `cargo fmt --all` must pass.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` should pass before declaring implementation work complete, unless a documented platform/toolchain limitation blocks it.
- `cargo test --workspace` should pass for touched behavior.
- Use typed errors; avoid stringly typed control flow.
- Avoid `unwrap()`/`expect()` in normal runtime paths unless the invariant is local and explicitly justified.
- No hidden global mutable application state.
- Keep unsafe code isolated, minimized, and documented; OS FFI belongs in `quadrant-platform`.
- Prefer small cohesive modules and explicit ownership over large manager/service god objects.

## 14. Correctness requirements

Formal performance benchmarking is not required, but correctness tests are.

At minimum, test:

- task quadrant/inbox transitions
- completion and recurrence transitions
- Today selection logic
- reminder rescheduling logic
- focus timer state transitions
- review aggregation
- storage migrations and transactional mutations
- settings serialization/validation

Platform-specific behavior should be hidden behind interfaces so non-OS logic can be tested deterministically.

## 15. Definition of done for a feature

A feature is complete only when:

1. Behavior is implemented in Rust/Slint.
2. No UI callback performs storage/platform work directly.
3. Persistent behavior has storage coverage where applicable.
4. Relevant tests pass.
5. User-visible error states are handled.
6. Cross-platform implications are considered; Windows-only behavior is isolated.
7. Project memory is updated according to Section 5.
8. Any copied/adapted GPL upstream file has correct provenance/SPDX information.

## 16. Anti-patterns

Do not:

- translate C# class-for-class into Rust
- recreate dependency injection containers merely because the C# version used service registration
- put database connections inside Slint state
- poll the database every N seconds for reminders
- scatter Win32 calls across the app
- create one background thread per feature
- use string page names as application control flow when an enum/type can represent them
- depend on Windows-installed fonts for essential cross-platform UI rendering
- copy GPL code and remove attribution
- let `docs/08_SESSION_HANDOFF.md` become stale after substantive work
