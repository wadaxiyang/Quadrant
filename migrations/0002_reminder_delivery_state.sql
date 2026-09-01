-- SPDX-FileCopyrightText: Copyright (c) 2026 Quadrant contributors
-- SPDX-License-Identifier: GPL-3.0-only

ALTER TABLE tasks
ADD COLUMN reminder_delivered_for_utc INTEGER NULL
CHECK (reminder_delivered_for_utc IS NULL OR reminder_at_utc IS NOT NULL);
