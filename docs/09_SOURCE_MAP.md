# 09 — Source & Provenance Map

This file tracks two different kinds of reference:

1. **Legacy Quadrant behavior reference** — C# code is inspected but not copied into the new architecture.
2. **GPL UI derivation** — wsl-dashboard Slint code may be directly copied/modified and therefore needs exact provenance.

## A. wsl-dashboard upstream

Repository: `https://github.com/owu/wsl-dashboard`  
Pinned planning baseline: `948589a255a4bd8a3ff9c3de49e2e13109378fcd`  
Version: `0.11.0`  
License: `GPL-3.0-only`

### Derived-file map

| Upstream | Quadrant destination | Intent | Status |
|---|---|---|---|
| `src/ui/components/sidebar.slint` | `ui/components/sidebar.slint` | Direct base; WSL navigation/font glyphs replaced by Quadrant routes and semantic images | Derived at pinned commit |
| `src/ui/theme.slint` | `ui/theme.slint` | Theme token subset; WSL structs/i18n and Windows-only font assumptions removed | Derived at pinned commit |
| `src/ui/components/common.slint` | `ui/components/common.slint` | SidebarItem interaction/layout adapted to SVG images; WSL-only controls omitted | Derived at pinned commit |
| `src/ui/components/title_bar.slint` | `ui/components/title_bar.slint` | Fluent title bar base | Planned |
| `src/ui/components/scrollbar.slint` | `ui/components/scrollbar.slint` | Scrollbar styling base | Planned |
| `src/ui/components/modal_manager.slint` | `ui/components/modal_manager.slint` | Modal visual/interaction base where useful | Planned |
| `src/ui/constants.slint` | `ui/constants.slint` | Sidebar dimensions retained; WSL-specific URLs removed | Derived at pinned commit |

When implementation occurs, change `Status` to Derived and record the **actual source commit** used if newer than the baseline.

### Provenance requirements

For direct derivatives:

- preserve existing `SPDX-FileCopyrightText` lines
- preserve `SPDX-License-Identifier: GPL-3.0-only`
- add Quadrant contributor copyright where appropriate
- include upstream in third-party notices/About

## B. Legacy Quadrant behavior map

Repository: current `wadaxiyang/Quadrant` legacy .NET implementation.

### Product surfaces

| Legacy source/surface | Target | Status |
|---|---|---|
| `Views/MainWindow.xaml` | `ui/app.slint` + sidebar/navigation | Behavior inventoried |
| `Views/Pages/QuadrantsPage.*` | `ui/views/quadrants.slint` + application task use cases | First task workflow inventoried |
| `Views/Pages/TodayPage.*` | `ui/views/today.slint` + Today query | To inspect deeply |
| `Views/Pages/FocusPage.*` | `ui/views/focus.slint` + focus application service | To inspect deeply |
| `Views/Pages/ReviewPage.*` | `ui/views/review.slint` + review query service | To inspect deeply |
| `Views/Pages/CompletedPage.*` | `ui/views/completed.slint` + completed queries | To inspect deeply |
| `Views/Pages/SettingsPage.*` | `ui/views/settings.slint` + typed settings | To inspect deeply |
| `Views/Pages/AboutPage.*` | `ui/views/about.slint` | Basic mapping known |
| `Views/QuickAddWindow.*` | `ui/components/quick_add.slint` / dedicated window | Capture behavior inventoried |
| `Views/TaskEditorWindow.*` | `ui/components/task_editor.slint` | First task-flow fields/validation inventoried |
| `Infrastructure/Windows/*` | `quadrant-platform` | Feature inventory known; no code port contract |
| `Infrastructure/Storage/*` | new `quadrant-storage` | **Do not preserve schema/API** |

### Legacy behavior already identified

- four quadrant management
- Today
- Focus: Stopwatch and Pomodoro
- Review
- Completed history
- Quick Add
- global hotkey
- notifications/reminders
- startup behavior
- tray behavior
- single-instance behavior
- local backup
- settings
- simple recurrence

### First vertical task workflow inventory

Inspected sources:

- `Quadrant.Core/Models/TaskItem.cs`, `TaskDraft.cs`, and `TaskUpdate.cs`
- `Quadrant.Core/Services/TaskRules.cs` and `TaskService.cs`
- `Quadrant.App/ViewModels/TaskEditorViewModel.cs` and `InboxPageViewModel.cs`
- `Quadrant.App/Views/QuickAddWindow.*`, `TaskEditorWindow.*`, and `Pages/QuadrantsPage.*`
- `Quadrant.App/Controls/TaskDestinationKeyboardShortcut.cs`

Behavior to preserve or deliberately redesign:

- Capture defaults to Inbox when the surface allows an unclassified destination; Quick Add activates, focuses, and selects the title field.
- `Ctrl+1` through `Ctrl+4`, including numpad keys, changes the destination to Q1–Q4 in Quick Add/task editor. Inbox rows additionally use bare `1`–`4` for classification, `Enter` for edit, and `Delete` for confirmed permanent deletion.
- Title is trimmed and required. A destination must be Inbox or Q1–Q4. The editor carries note, planned date, estimated minutes, due/reminder, and recurrence fields even where the primary UI temporarily hides compatibility fields.
- A new due date defaults to a reminder five minutes before due time. Relative reminder choices cover five minutes, 1–12 hours, and 1–7 days; custom reminders are supported. Newly changed due/reminder/recurrence times must be in the future and reject invalid/ambiguous local DST times.
- Recurrence supports daily, weekly, monthly, and custom 1–365 day intervals. Recurring tasks use their recurrence start as the notification instant and do not persist a separate due/reminder pair for that occurrence.
- Inbox is displayed beside the four-quadrant matrix on wide layouts. Below 900px it becomes a collapsible panel capped at 180px; header actions reflow below 760px.
- Tasks can move Inbox → quadrant, quadrant → Inbox, and quadrant → quadrant by drag/drop. Invalid same-source drops are ignored. Classification/move feedback offers undo, guarded by re-reading current state so a stale undo does not overwrite later changes.
- Task cards support `Enter`/`Space` completion outside nested interactive controls. Inbox actions include complete, plan for today, edit, and delete.
- `Ctrl+F` focuses/selects search. Escape clears search, restores the All filter, and releases focus.
- Inbox exposes loading, empty, failure/retry, and recoverable operation errors. Its ordering is capture time then ID, and it listens for application changes rather than polling.
- Moving/classifying a task preserves its reminder and does not force an OS reminder rebuild. Create/update syncs reminders; completing/deleting cancels them. Reminder platform failures are logged and do not roll back the already-valid task mutation in the legacy behavior.
- Completing a recurring task records a completion snapshot and creates/advances the next occurrence atomically in the repository operation. Reopening reverts the completion snapshot but intentionally does not revive an old OS reminder schedule.
- The legacy delete prompt describes hard deletion. The Rust rewrite's final hard-delete/archive policy remains intentionally undecided until M2 schema design.

## C. Icon assets

Repository: `https://github.com/microsoft/fluentui-system-icons`  
Commit: `4d685f77b2cb8f3f412a74ec8d920c8c91149528`  
Release/package version: `1.1.339`  
License: MIT; copied to `assets/icons/LICENSE-MIT`

| Upstream asset | Destination | Semantic ID |
|---|---|---|
| `assets/Navigation/SVG/ic_fluent_navigation_24_regular.svg` | `assets/icons/navigation.svg` | `Icons.menu` |
| `assets/Grid/SVG/ic_fluent_grid_24_regular.svg` | `assets/icons/quadrants.svg` | `Icons.quadrants` |
| `assets/Calendar Today/SVG/ic_fluent_calendar_today_24_regular.svg` | `assets/icons/today.svg` | `Icons.today` |
| `assets/Timer/SVG/ic_fluent_timer_24_regular.svg` | `assets/icons/focus.svg` | `Icons.focus` |
| `assets/Chart Multiple/SVG/ic_fluent_chart_multiple_24_regular.svg` | `assets/icons/review.svg` | `Icons.review` |
| `assets/Checkmark Circle/SVG/ic_fluent_checkmark_circle_24_regular.svg` | `assets/icons/completed.svg` | `Icons.completed` |
| `assets/Settings/SVG/ic_fluent_settings_24_regular.svg` | `assets/icons/settings.svg` | `Icons.settings` |
| `assets/Info/SVG/ic_fluent_info_24_regular.svg` | `assets/icons/about.svg` | `Icons.about` |

Assets are copied without path-data conversion. Slint applies semantic colorization at render time through `ui/icons.slint` and `SidebarItem`, so no installed icon font is required.
