# 03 — Domain & Data Memory

## Persistence policy

The rewrite starts with a **new database**. There is no migration requirement from the old C# database.

SQLite is used through **`rusqlite`**.

The schema is owned by the Rust rewrite and must be versioned through SQL migrations from day one.

## Domain identifiers

Use opaque typed IDs in domain/application code rather than passing arbitrary strings everywhere.

The exact ID generator may be UUID/UUIDv7 or another stable choice, but once selected it becomes a recorded decision in `05_DECISIONS.md`.

## Quadrant model

```text
Q1: Important + Urgent
Q2: Important + Not Urgent
Q3: Not Important + Urgent
Q4: Not Important + Not Urgent
```

Inbox is represented as no quadrant assignment.

Recommended Rust idea:

```text
enum Quadrant { Q1, Q2, Q3, Q4 }
Task.quadrant: Option<Quadrant>
```

Do not persist both quadrant enum and redundant important/urgent booleans unless a demonstrated query need justifies it.

## Task invariants

- title must not be empty after trimming
- quadrant, when present, is one of Q1..Q4
- completed task has a completion timestamp
- active task does not carry a current completion timestamp
- reminder semantics must be compatible with task lifecycle
- recurrence rule must validate before persistence
- planned date and due date have distinct meanings

## Time model

Use explicit time semantics.

Suggested storage strategy:

- absolute event timestamps stored as integer Unix UTC time
- date-only planning stored as ISO local date (`YYYY-MM-DD`)
- store a timezone identifier alongside due/reminder values when local-time recurrence behavior depends on timezone

Do not store locale-formatted date strings.

## Recurrence

Recurrence is a domain value object, not a raw UI string.

Initial product may support a simple set such as:

- daily
- weekdays
- weekly
- monthly
- custom interval

Store a versionable serialized representation (`recurrence_json`) unless/until normalized query requirements justify more tables.

Completion of a recurring task is a compound transaction:

1. record completion occurrence/history
2. advance/create next occurrence semantics
3. update task state/next planned/due values
4. update reminder state
5. commit atomically
6. signal reminder scheduler

## Baseline schema

### `schema_migrations`

Tracks applied migrations.

### `tasks`

Baseline columns:

| Column | Meaning |
|---|---|
| `id` | opaque task ID |
| `title` | required title |
| `notes` | free text |
| `quadrant` | NULL=Inbox; otherwise 1..4 |
| `status` | active/completed enum |
| `planned_on` | local date for Today/planning |
| `due_at_utc` | optional absolute due instant |
| `due_tz` | timezone semantics if needed |
| `reminder_at_utc` | optional reminder instant |
| `reminder_tz` | timezone semantics if needed |
| `recurrence_json` | versioned recurrence value |
| `sort_key` | stable manual ordering |
| `created_at_utc` | audit time |
| `updated_at_utc` | audit time |
| `completed_at_utc` | current completion time if completed |

Add database CHECK constraints where they improve integrity.

### `task_completion_events`

Immutable or append-oriented history needed by Review and recurrence.

Possible fields:

```text
id
task_id
task_title_snapshot
quadrant_snapshot
completed_at_utc
recurrence_occurrence_key
```

Snapshot only what Review/history truly needs.

### `focus_sessions`

Possible fields:

```text
id
task_id NULL
mode
started_at_utc
ended_at_utc
duration_seconds
outcome
```

Live timer state does not need to be written every second.

### `settings`

Recommended simple schema:

```text
key TEXT PRIMARY KEY
value_json TEXT NOT NULL
updated_at_utc INTEGER NOT NULL
```

Rust owns typed parsing/validation.

## SQLite configuration

Initialize every connection consistently:

- foreign keys ON
- busy timeout configured
- WAL considered/defaulted if it matches actual access pattern
- synchronous policy chosen intentionally

Do not scatter PRAGMA setup across repositories.

## Repositories

Prefer capability-oriented repository traits/ports rather than one giant repository.

Examples:

- task mutation repository
- task query repository
- focus session repository
- review query service
- settings repository

Read-heavy Review queries can use specialized SQL projections without pretending every query is a domain entity load.

## Ordering

Quadrant task ordering needs a stable persisted sort key so drag/reorder does not rewrite every task unnecessarily.

Choose and document a strategy when implementation begins (integer gaps/fractional rank/etc.).

## Backup

A backup must use an SQLite-consistent mechanism. Do not blindly file-copy an open DB.

Backup package should carry at least:

- database snapshot
- Quadrant app version
- schema version
- backup timestamp

## Delete policy

Decide explicitly whether task deletion is hard delete, soft delete, or archive when implementing it. Do not accidentally mix policies across pages.

Record the decision in `05_DECISIONS.md` before relying on it in schema/query design.

## Implementation state

M0 introduces only the pure placement vocabulary: `Quadrant::{Q1,Q2,Q3,Q4}` and `TaskPlacement::{Inbox, Quadrant}`. `TaskPlacement` defaults to Inbox and has focused unit coverage. Task IDs, full task invariants, timestamps, recurrence, schema, `rusqlite`, ordering, and delete policy remain M2 work; the M0 skeleton must not be mistaken for a finalized domain model.
