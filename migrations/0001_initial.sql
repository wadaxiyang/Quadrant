-- SPDX-FileCopyrightText: Copyright (c) 2026 Quadrant contributors
-- SPDX-License-Identifier: GPL-3.0-only

CREATE TABLE tasks (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0 AND length(title) <= 500),
    notes TEXT NOT NULL DEFAULT '',
    quadrant INTEGER NULL CHECK (quadrant IS NULL OR quadrant BETWEEN 1 AND 4),
    status INTEGER NOT NULL CHECK (status IN (0, 1)),
    planned_on TEXT NULL,
    due_at_utc INTEGER NULL,
    due_tz TEXT NULL,
    reminder_at_utc INTEGER NULL,
    reminder_tz TEXT NULL,
    recurrence_json TEXT NULL,
    sort_key INTEGER NOT NULL,
    created_at_utc INTEGER NOT NULL,
    updated_at_utc INTEGER NOT NULL,
    completed_at_utc INTEGER NULL,
    CHECK ((status = 0 AND completed_at_utc IS NULL) OR (status = 1 AND completed_at_utc IS NOT NULL)),
    CHECK ((due_at_utc IS NULL) = (due_tz IS NULL)),
    CHECK ((reminder_at_utc IS NULL) = (reminder_tz IS NULL))
) STRICT;

CREATE INDEX tasks_active_placement_order
    ON tasks(status, quadrant, sort_key, created_at_utc, id);
CREATE INDEX tasks_planned_on
    ON tasks(status, planned_on) WHERE planned_on IS NOT NULL;
CREATE INDEX tasks_reminder_at
    ON tasks(status, reminder_at_utc) WHERE reminder_at_utc IS NOT NULL;

CREATE TABLE task_completion_events (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NULL REFERENCES tasks(id) ON DELETE SET NULL,
    task_title_snapshot TEXT NOT NULL,
    quadrant_snapshot INTEGER NULL CHECK (quadrant_snapshot IS NULL OR quadrant_snapshot BETWEEN 1 AND 4),
    completed_at_utc INTEGER NOT NULL,
    recurrence_occurrence_key TEXT NULL
) STRICT;

CREATE INDEX completion_events_completed_at
    ON task_completion_events(completed_at_utc DESC, id);
CREATE INDEX completion_events_task
    ON task_completion_events(task_id, completed_at_utc DESC) WHERE task_id IS NOT NULL;

CREATE TABLE focus_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NULL REFERENCES tasks(id) ON DELETE SET NULL,
    mode INTEGER NOT NULL CHECK (mode IN (0, 1)),
    started_at_utc INTEGER NOT NULL,
    ended_at_utc INTEGER NOT NULL,
    duration_seconds INTEGER NOT NULL CHECK (duration_seconds >= 0),
    outcome INTEGER NOT NULL CHECK (outcome IN (0, 1, 2)),
    CHECK (ended_at_utc >= started_at_utc)
) STRICT;

CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    updated_at_utc INTEGER NOT NULL
) STRICT;
