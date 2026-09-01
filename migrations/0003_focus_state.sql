-- SPDX-FileCopyrightText: Copyright (c) 2026 Quadrant contributors
-- SPDX-License-Identifier: GPL-3.0-only

ALTER TABLE focus_sessions RENAME TO focus_sessions_m2;

CREATE TABLE focus_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NULL REFERENCES tasks(id) ON DELETE SET NULL,
    task_title_snapshot TEXT NULL,
    quadrant_snapshot INTEGER NULL CHECK (quadrant_snapshot IS NULL OR quadrant_snapshot BETWEEN 1 AND 4),
    mode INTEGER NOT NULL CHECK (mode IN (0, 1)),
    pomodoro_kind INTEGER NULL CHECK (pomodoro_kind IS NULL OR pomodoro_kind IN (0, 1, 2)),
    started_at_utc INTEGER NOT NULL,
    active_segment_started_at_utc INTEGER NULL,
    ended_at_utc INTEGER NULL,
    target_duration_seconds INTEGER NULL CHECK (target_duration_seconds IS NULL OR target_duration_seconds > 0),
    duration_seconds INTEGER NOT NULL CHECK (duration_seconds >= 0),
    status INTEGER NOT NULL CHECK (status IN (0, 1, 2, 3)),
    created_local_date TEXT NOT NULL CHECK (
        length(created_local_date) = 10
        AND date(created_local_date, '+0 days') = created_local_date
    ),
    CHECK (
        (mode = 0 AND pomodoro_kind IS NULL AND target_duration_seconds IS NULL)
        OR (mode = 1 AND pomodoro_kind IS NOT NULL AND target_duration_seconds IS NOT NULL)
    ),
    CHECK (pomodoro_kind NOT IN (1, 2) OR task_id IS NULL),
    CHECK (
        (status = 0 AND active_segment_started_at_utc IS NOT NULL AND ended_at_utc IS NULL)
        OR (status = 1 AND active_segment_started_at_utc IS NULL AND ended_at_utc IS NULL)
        OR (status IN (2, 3) AND active_segment_started_at_utc IS NULL AND ended_at_utc IS NOT NULL)
    ),
    CHECK (active_segment_started_at_utc IS NULL OR active_segment_started_at_utc >= started_at_utc),
    CHECK (ended_at_utc IS NULL OR ended_at_utc >= started_at_utc),
    CHECK (target_duration_seconds IS NULL OR duration_seconds <= target_duration_seconds)
) STRICT;

INSERT INTO focus_sessions (
    id, task_id, task_title_snapshot, quadrant_snapshot, mode, pomodoro_kind,
    started_at_utc, active_segment_started_at_utc, ended_at_utc,
    target_duration_seconds, duration_seconds, status, created_local_date
)
SELECT old.id,
       old.task_id,
       tasks.title,
       tasks.quadrant,
       old.mode,
       CASE WHEN old.mode = 1 THEN 0 ELSE NULL END,
       old.started_at_utc,
       NULL,
       old.ended_at_utc,
       CASE WHEN old.mode = 1 THEN MAX(old.duration_seconds, 1) ELSE NULL END,
       old.duration_seconds,
       CASE WHEN old.outcome = 0 THEN 2 ELSE 3 END,
       date(old.started_at_utc, 'unixepoch')
FROM focus_sessions_m2 AS old
LEFT JOIN tasks ON tasks.id = old.task_id;

DROP TABLE focus_sessions_m2;

CREATE UNIQUE INDEX focus_sessions_one_current
    ON focus_sessions((1)) WHERE status IN (0, 1);
CREATE INDEX focus_sessions_current_deadline
    ON focus_sessions(status, active_segment_started_at_utc)
    WHERE status IN (0, 1);
CREATE INDEX focus_sessions_local_summary
    ON focus_sessions(created_local_date, status, mode, pomodoro_kind);
CREATE INDEX focus_sessions_task
    ON focus_sessions(task_id, started_at_utc DESC)
    WHERE task_id IS NOT NULL;
