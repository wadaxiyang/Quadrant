# 07 — Backlog

This file is ordered by implementation dependency, not by speculative feature desirability.

## P0 — Foundation

1. [x] Copy bootstrap documentation into repository root.
2. [x] Replace MIT license with GPL-3.0-only and add provenance/third-party notice structure.
3. [x] Create Cargo workspace and crate skeletons.
4. [x] Decide initial stable Rust toolchain policy and record it.
5. [x] Add Slint build integration and minimal main window.
6. [x] Create `quadrant-ui` binding boundary.
7. [x] Vendor/adapt Sidebar + Theme + Common primitives from wsl-dashboard baseline commit with SPDX retained.
8. [x] Create semantic icon abstraction suitable for Windows/Linux/macOS.

## P0 — Product parity inventory

1. [x] Inspect and record the first Capture/Inbox/Quadrants/task-editor workflow.
2. [ ] Inspect Today, Focus, Review, Completed, Settings, and platform behavior in depth when their milestone begins.
3. [x] Map every legacy surface at a high level; keep detailed mappings current as inspection proceeds.
4. [ ] Identify remaining settings/edge cases that must survive the redesign.

## P0 — M1 UI shell completion

1. Add the derived/adapted title bar and window controls.
2. Add System/Light/Dark state flow between application, platform capability, and Slint Theme.
3. Split placeholder route content into view files without adding business logic.
4. Establish shared modal/toast/error presentation primitives.
5. Add the keyboard-first Quick Add shell and typed UI intent boundary.

## P1 — Domain/storage

1. Finalize Task ID strategy.
2. Finalize timestamp/timezone representation.
3. Finalize recurrence representation.
4. Write migration `0001_initial.sql`.
5. Implement storage connection/migration bootstrap.
6. Implement task mutations/queries.
7. Implement completion-event history.
8. Implement focus-session persistence.
9. Implement typed settings persistence.

## P1 — Main task UI

1. Quadrants view.
2. Inbox presentation inside task workflow.
3. drag/move/order behavior.
4. task editor.
5. Quick Add.
6. user-facing validation/errors.

## P1 — Runtime/platform

1. application event/runtime wiring.
2. single instance and activation semantics.
3. global Quick Add shortcut.
4. tray.
5. reminder scheduler.
6. notifications.
7. autostart.
8. system theme/accent feed.

## P2 — Today/Focus/Review

1. Today rules/tests/UI.
2. focus state machine.
3. stopwatch/Pomodoro UI.
4. Review projections and charts/cards.
5. Completed history/reopen.

## P2 — Persistence quality

1. backup format.
2. restore validation.
3. corruption/startup error UX.
4. migration failure diagnostics.

## P3 — Cross-platform completion

1. Linux capability implementations and packaging.
2. macOS capability implementations and packaging.
3. verify icon/font/rendering parity.
4. verify global shortcut/tray/notification graceful behavior per platform.

## Later / only when requested

- cloud sync
- mobile
- collaboration
- plugins
- formal performance benchmark program
