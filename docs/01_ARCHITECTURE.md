# 01 — Architecture Memory

## Architectural style

Quadrant uses a pragmatic clean/hexagonal architecture with explicit ownership boundaries. The goal is not abstract layering for its own sake; it is to keep Slint, SQLite, and platform APIs from contaminating the domain and each other.

## Crates

### `quadrant-domain`

Pure Rust business model.

Allowed dependencies: small general-purpose crates that do not bind the domain to UI, async runtime, database, or OS.

Contains:

- `TaskId`
- `Task`
- `Quadrant`
- `TaskStatus`
- `RecurrenceRule`
- focus state/value types
- domain validation and calculations

No Slint types. No SQL. No Tokio channels. No path discovery. No notifications.

### `quadrant-application`

Coordinates use cases and defines boundaries.

Suggested modules:

```text
commands/
queries/
ports/
scheduler/
focus/
review/
state/
events/
```

Examples:

- `CreateTask`
- `UpdateTask`
- `MoveTask`
- `CompleteTask`
- `ReopenTask`
- `GetToday`
- `StartFocus`
- `FinishFocus`
- `GetReviewSummary`

Application events are typed Rust enums/structs.

### `quadrant-storage`

Concrete `rusqlite` implementation.

Suggested modules:

```text
connection.rs
migrations.rs
repositories/
queries/
backup.rs
mapping.rs
```

Owns SQL and database DTO mapping. Domain/application crates never see `rusqlite::Row` or `Connection`.

### `quadrant-platform`

Single platform integration boundary.

Suggested modules:

```text
capabilities.rs
hotkey/
tray/
notification/
startup/
single_instance/
window/
theme/
shell/
paths/
```

Each module can use target-specific implementations selected with `cfg`.

This is the only project crate that may use the `windows` crate or other OS FFI directly.

### `quadrant-ui`

Adapter between Slint presentation and the application layer.

Suggested modules:

```text
bindings/
models/
intents.rs
presenters/
navigation.rs
```

Responsibilities:

- bind Slint callbacks
- convert Slint structs/models to application command input
- receive state/events and update Slint models on the Slint event loop
- format presentation strings where appropriate

No SQL and no OS-specific behavior.

### `quadrant-app`

Composition root/binary.

Responsibilities:

- create async runtime
- initialize logging
- initialize data directory/storage/migrations
- instantiate concrete repositories/platform services
- construct application services
- create and bind UI
- coordinate shutdown

Keep this crate thin.

## State flow

Preferred control flow:

```text
User interaction
  -> Slint callback
  -> UiIntent
  -> application command/use case
  -> domain mutation + storage transaction / platform call
  -> ApplicationEvent or StateSnapshot
  -> UI presenter/model adapter
  -> Slint event loop update
```

Avoid two-way shared mutable state between Slint and application services.

## Application state

Do not build a class-for-class MVVM clone.

Use a small number of explicit state projections such as:

- `NavigationState`
- `QuadrantsViewState`
- `TodayViewState`
- `FocusViewState`
- `ReviewViewState`
- `CompletedViewState`
- `SettingsViewState`

The application/database remains source of truth. UI models are projections.

## Commands vs queries

Commands mutate state and should return enough result/event information to update projections.

Queries do not mutate domain state and may use optimized storage read models for Review/Completed rather than reconstructing every domain entity.

## Transactions

Compound state transitions must be atomic.

Examples:

- recurring task completion + completion event + next occurrence/reminder state
- task deletion + dependent reminder/history policy
- focus finish + session persistence + related task statistics if persisted

## Time

Never use UI frame/tick count as elapsed time truth.

Application/domain functions that depend on "now" should receive a clock abstraction or explicit time value so tests are deterministic.

Store absolute timestamps in a stable form; retain timezone semantics where recurrence/reminders require local-time interpretation.

## Event-driven background work

There is no generic periodic background polling loop.

Long-lived services are explicit:

- reminder scheduler
- platform event listeners
- optional update checker according to configured schedule/trigger

Each service sleeps/waits on real signals/deadlines and wakes when state changes.

## Legacy reference lifecycle

Legacy source is temporary. Before deleting a legacy subsystem, ensure:

1. behavior was captured in SPEC/docs or replacement tests,
2. Rust replacement exists or behavior was intentionally dropped,
3. `09_SOURCE_MAP.md` records the mapping.

Final active tree must be Rust/Slint only.

## Implemented foundation

As of 2026-08-31, the six-crate Cargo workspace exists with the required dependency direction encoded in manifests. `quadrant-ui/build.rs` compiles root `ui/app.slint`; `quadrant-app` is a thin binary that enters the UI adapter. Storage and platform crates currently expose boundary skeletons only—there is no SQL, async runtime, or OS API implementation yet.

The UI crate uses `deny(unsafe_code)` rather than `forbid(unsafe_code)` because generated Slint code contains internally scoped unsafe implementation details. Handwritten UI adapter code contains no unsafe code. Domain, application, and storage crates forbid unsafe code.
