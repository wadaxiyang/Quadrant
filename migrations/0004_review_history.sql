-- SPDX-FileCopyrightText: Copyright (c) 2026 Quadrant contributors
-- SPDX-License-Identifier: GPL-3.0-only

ALTER TABLE task_completion_events
ADD COLUMN completed_local_date TEXT NULL;

ALTER TABLE task_completion_events
ADD COLUMN due_at_utc_snapshot INTEGER NULL;

ALTER TABLE task_completion_events
ADD COLUMN planned_on_snapshot TEXT NULL;

ALTER TABLE task_completion_events
ADD COLUMN was_overdue INTEGER NOT NULL DEFAULT 0 CHECK (was_overdue IN (0, 1));

ALTER TABLE task_completion_events
ADD COLUMN reverted_at_utc INTEGER NULL CHECK (
    reverted_at_utc IS NULL OR reverted_at_utc >= completed_at_utc
);

-- Early development rows did not retain a host-local date. UTC date is the
-- only deterministic backfill; all new rows receive the real host-local date.
UPDATE task_completion_events
SET completed_local_date = date(completed_at_utc, 'unixepoch')
WHERE completed_local_date IS NULL;

CREATE INDEX completion_events_review_date_active
    ON task_completion_events(completed_local_date, completed_at_utc)
    WHERE reverted_at_utc IS NULL;

CREATE INDEX completion_events_recent_active
    ON task_completion_events(completed_at_utc DESC, id DESC)
    WHERE reverted_at_utc IS NULL;
