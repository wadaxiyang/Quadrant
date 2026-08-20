# STATUS — Codex Handoff

## Current stage

`STAGE_08_FILTER_SEARCH_COMPLETED` — implementation complete; manual GUI checks pending

## Completed

- Created `Quadrant.sln` with the three source projects and two test projects.
- Added `global.json` pinning .NET SDK `10.0.400` with `latestPatch` roll-forward.
- Added central package management in `Directory.Packages.props`.
- Added `Directory.Build.props` with nullable and implicit usings enabled.
- Added an empty WPF application entry point and one smoke test per test project.
- Kept `Quadrant.Core` on `net10.0` with no project references.
- Kept Stage 00 free of business UI, database schema, notifications, hotkeys, and DI.
- Added a repository-local `NuGet.Config` so restore does not depend on inaccessible user-level NuGet configuration.
- Added explicit `using Xunit;` imports required by the smoke tests.
- Added the WPF native Fluent application shell with `ThemeMode="System"`.
- Added explicit composition from `App.xaml.cs` to `MainViewModel` and `MainWindow`.
- Added the 1180x760 main window shell with 920x620 minimum size, top tool area, and 2x2 placeholder grid.
- Added minimal MVVM bindings for application title and placeholder title.
- Added merged WPF ResourceDictionaries for spacing, typography, quadrant colors, and reusable control styles.
- Replaced MainWindow layout/font/border magic values with shared resource tokens and styles.
- Kept Fluent/system brushes as `DynamicResource` references so theme changes can update them.
- Fixed WPF layout resource typing by using `Thickness` tokens for `Margin`/`Padding`; the app now starts successfully.
- Added pure Core models: `TaskItem`, `TaskDraft`, `TaskUpdate`, and `QuadrantDefinition`.
- Added `TaskFilter` and `ReminderPreset` enums.
- Added repository, quadrant, reminder scheduler, clock, and task service contracts.
- Added `TaskRules` for title/quadrant validation, local Today/Overdue checks, completion/restore semantics, and initial reminder preset calculation.
- Added `TaskService` CRUD orchestration skeleton using injected `IClock` and reminder scheduler.
- Added `LocalAppDataPathProvider` using `%LOCALAPPDATA%\Quadrant\quadrant.db` resolution via `Environment.SpecialFolder.LocalApplicationData`.
- Added `SqliteConnectionFactory` with explicit read-write-create connection settings and disabled pooling for deterministic test cleanup.
- Added schema version initializer at version 1 with parameterized migration and default quadrants.
- Added `SqliteTaskRepository` and `SqliteQuadrantRepository` with parameterized SQL, transactions for create/migration, and ISO-8601 `DateTimeOffset` mapping.
- Added independent temporary-database infrastructure tests for migration, CRUD, nullable values, timestamps, completion/restore, deletion, foreign keys, idempotent startup, and apostrophe-containing titles.
- Added `SystemClock` and `NoOpReminderScheduler` for application composition.
- Added `QuadrantViewModel` and `TaskCardViewModel`; entities are not used as WPF observable objects.
- Composed startup as async DB initialization, repository creation, `TaskService` creation, and active-task loading before showing `MainWindow`.
- Replaced placeholder cells with four explicit 2x2 quadrant panels backed by real database data.
- Added independently scrolling virtualized/recycling `ListBox` task lists with due/reminder text and empty states.
- Added fixed due/created sorting inside each quadrant: dated tasks first, due ascending, created ascending.
- Fixed Stage 05 startup crash: `GridLength` is now used for the 3px quadrant accent column token instead of `Double`.
- Added Stage 06 task editor with title, quadrant, due date, editable 15-minute time suggestions, inline time validation, note, cancel, and save.
- Fixed date-only due semantics: a selected date without time is persisted at local 23:59; custom time accepts `H:mm` and `HH:mm`.
- Added MainViewModel create/edit/complete/delete command flow through TaskService; completion removes active cards and deletion uses a lightweight confirmation event from the View.
- Preserved existing ReminderAt when editing; Reminder controls remain deferred to Stage 09.
- Added TaskService completion/delete scheduler interaction coverage.
- Expanded .gitignore for nested generated output, IDE state, dumps, and scratch/test artifacts. Removing already-tracked generated files from the Git index was attempted but blocked by environment permission on `.git/index.lock` creation.
- Added `TaskService.MoveTaskAsync` with fixed quadrant validation, same-quadrant no-op, completed-task no-op, and reminder rescheduling through the existing update path.
- Added `MoveTaskCommand` request flow in `MainViewModel`; WPF Drop forwards only task id and target quadrant id.
- Added native WPF drag threshold handling, `DataObject` payload with internal task-id format, `AllowDrop`, `DragOver` feedback, `Drop`, and `DragLeave` cleanup.
- Kept drag/drop limited to quadrant movement; no manual order, multi-select, or drag-out delete was added.
- Added single `SelectedFilter` state with All/Today/Overdue while retaining the four quadrant collections.
- Added in-memory, case-insensitive Title/Note search combined with the selected filter; no debounce timer or advanced query syntax.
- Added Ctrl+F search focus and Esc search clear/filter reset behavior.
- Added explicit Today/Overdue status text on task cards without coloring entire cards.
- Added a separate completed-task window ordered by CompletedAt descending with restore and permanent delete commands.
- Restoring a completed task uses its original QuadrantId and reloads the active quadrant view.
- Fixed TaskEditorWindow runtime crash on open: `ColumnDefinition.Width` was incorrectly assigned the `Double` spacing resource `SpaceL`; added the dedicated `GridLength` resource `FormColumnGap`.
- Clarified task editor validation: title/name is required; due date is optional; no due date persists `DueAt = null`.
- Added recoverable error handling around new/edit save events so repository or refresh failures show a warning instead of escaping from `async void` and terminating the app.
- Fixed startup/runtime XAML crash caused by `StringToVisibilityConverter` scope in `TaskCardTemplate.xaml`; the converter is now declared in that resource dictionary before the template.

## Files changed

- `Quadrant.sln`
- `global.json`
- `Directory.Build.props`
- `Directory.Packages.props`
- `src/Quadrant.Core/Quadrant.Core.csproj`
- `src/Quadrant.Infrastructure/Quadrant.Infrastructure.csproj`
- `src/Quadrant.App/Quadrant.App.csproj`
- `src/Quadrant.App/App.xaml`
- `src/Quadrant.App/App.xaml.cs`
- `Tests/Quadrant.Core.Tests/Quadrant.Core.Tests.csproj`
- `Tests/Quadrant.Core.Tests/SmokeTests.cs`
- `Tests/Quadrant.Infrastructure.Tests/Quadrant.Infrastructure.Tests.csproj`
- `Tests/Quadrant.Infrastructure.Tests/SmokeTests.cs`
- `NuGet.Config`
- `src/Quadrant.App/ViewModels/MainViewModel.cs`
- `src/Quadrant.App/Views/MainWindow.xaml`
- `src/Quadrant.App/Views/MainWindow.xaml.cs`
- `src/Quadrant.App/Resources/Spacing.xaml`
- `src/Quadrant.Core/Interfaces/ITaskService.cs`
- `src/Quadrant.Core/Services/TaskService.cs`
- `Tests/Quadrant.Core.Tests/TaskServiceTests.cs`
- `src/Quadrant.App/Resources/TaskCardTemplate.xaml`
- `src/Quadrant.App/ViewModels/CompletedTaskViewModel.cs`
- `src/Quadrant.App/Views/CompletedWindow.xaml`
- `src/Quadrant.App/Views/CompletedWindow.xaml.cs`
- `src/Quadrant.App/Views/MainWindow.xaml.cs`
- `src/Quadrant.App/ViewModels/TaskEditorViewModel.cs`
- `src/Quadrant.App/Views/TaskEditorWindow.xaml`
- `src/Quadrant.App/Views/TaskEditorWindow.xaml.cs`
- `src/Quadrant.App/Views/MainWindow.xaml.cs`
- `src/Quadrant.App/Resources/Typography.xaml`
- `src/Quadrant.App/Resources/Typography.xaml`
- `src/Quadrant.App/Resources/QuadrantColors.xaml`
- `src/Quadrant.App/Resources/ControlStyles.xaml`
- `src/Quadrant.Core/Models/TaskItem.cs`
- `src/Quadrant.Core/Models/TaskDraft.cs`
- `src/Quadrant.Core/Models/TaskUpdate.cs`
- `src/Quadrant.Core/Models/QuadrantDefinition.cs`
- `src/Quadrant.Core/Enums/TaskFilter.cs`
- `src/Quadrant.Core/Enums/ReminderPreset.cs`
- `src/Quadrant.Core/Interfaces/IClock.cs`
- `src/Quadrant.Core/Interfaces/ITaskRepository.cs`
- `src/Quadrant.Core/Interfaces/IQuadrantRepository.cs`
- `src/Quadrant.Core/Interfaces/IReminderScheduler.cs`
- `src/Quadrant.Core/Interfaces/ITaskService.cs`
- `src/Quadrant.Core/Services/TaskValidationException.cs`
- `src/Quadrant.Core/Services/TaskRules.cs`
- `src/Quadrant.Core/Services/TaskService.cs`
- `Tests/Quadrant.Core.Tests/TaskRulesTests.cs`
- `Tests/Quadrant.Core.Tests/TaskServiceTests.cs`
- `src/Quadrant.Infrastructure/Storage/LocalAppDataPathProvider.cs`
- `src/Quadrant.Infrastructure/Storage/SqliteConnectionFactory.cs`
- `src/Quadrant.Infrastructure/Storage/SqliteDatabaseInitializer.cs`
- `src/Quadrant.Infrastructure/Storage/SqliteValueConverter.cs`
- `src/Quadrant.Infrastructure/Storage/SqliteTaskRepository.cs`
- `src/Quadrant.Infrastructure/Storage/SqliteQuadrantRepository.cs`
- `Tests/Quadrant.Infrastructure.Tests/SqliteStorageTests.cs`
- `src/Quadrant.Infrastructure/Windows/SystemClock.cs`
- `src/Quadrant.Infrastructure/Notifications/NoOpReminderScheduler.cs`
- `src/Quadrant.App/ViewModels/TaskCardViewModel.cs`
- `src/Quadrant.App/ViewModels/QuadrantViewModel.cs`
- `src/Quadrant.App/Resources/TaskCardTemplate.xaml`
- `src/Quadrant.App/Converters/StringToVisibilityConverter.cs`
- `src/Quadrant.App/Resources/Spacing.xaml`
- `STATUS.md`

## Architecture decisions / deviations

- Package versions verified against NuGet stable version indexes on 2026-08-20:
  - `CommunityToolkit.Mvvm` 8.4.2
  - `Microsoft.Data.Sqlite` 10.0.11
  - `Microsoft.NET.Test.Sdk` 18.9.0
  - `xunit` 2.9.3
  - `xunit.runner.visualstudio` 3.1.3
- .NET official release metadata reports .NET 10 release `10.0.11` and SDK `10.0.400`; these are pinned in `global.json`.
- `Microsoft.WindowsAppSDK` was intentionally not added; it belongs to Stage 10.
- The .NET 10 SDK reported `NETSDK1137` for `Microsoft.NET.Sdk.WindowsDesktop`; the WPF project now uses `Microsoft.NET.Sdk` with `UseWPF=true`, as recommended by the SDK.
- WPF native Fluent is applied at application scope with `ThemeMode="System"`, based on current Microsoft Learn guidance. Light/Dark programmatic switching is deferred to the settings stage.
- `Spacing.xaml` defines the required 4/8/12/16/24/32 spacing tokens and 4/8 corner radii.
- `Typography.xaml` defines the required 20/16/14/12 text hierarchy.
- `QuadrantColors.xaml` defines the exact four quadrant accents from `DESIGN_SYSTEM.md`; no panel background is filled with these colors.
- `ControlStyles.xaml` defines `QuadrantPanelStyle`, `TaskCardStyle`, `SectionTitleTextStyle`, `CaptionTextStyle`, and a native-based `IconButtonStyle` without shadows or custom templates.
- WPF `DynamicResource` is used for Fluent system border brushes; no third-party theme package was added.
- Core remains ordinary C# on `net10.0`; it does not reference WPF, Windows App SDK, WinForms, SQLite, or Win32.
- `DateTimeOffset` is used for all task timestamps; Today compares local calendar dates and Overdue compares the instant against the injected clock.
- Reminder preset calculation is intentionally minimal; detailed scheduling validation belongs to Stage 09.
- Schema version is fixed at `1`; migration 001 creates `schema_version`, `quadrants`, `tasks`, and the three task indexes.
- Database time values use ISO-8601 round-trip (`O`) text with `DateTimeOffset.Parse` using invariant culture and round-trip styles.
- Each opened connection enables `PRAGMA foreign_keys = ON` and sets `PRAGMA busy_timeout = 5000`.
- Current Microsoft.Data.Sqlite async transaction factory exposed `DbTransaction`; implementation uses synchronous `BeginTransaction()` to obtain the concrete `SqliteTransaction` required by `SqliteCommand.Transaction`.
- Stage 05 remains read-only: no CRUD, filtering, search, drag/drop, or reminder scheduling was added.
- `ListBox` uses `ScrollViewer.CanContentScroll`, `VirtualizingPanel.IsVirtualizing`, and `VirtualizationMode=Recycling` per current WPF guidance.
- MainViewModel loads four fixed quadrant definitions from SQLite and partitions active tasks by `QuadrantId`; no automatic quadrant generation is used.
- WPF layout tokens use property-specific types: `Thickness` for margins/padding and `GridLength` for grid column widths.
- No architecture deviations recorded.
- Stage 06 uses the required local-date-only rule: DueAt is local 23:59 when DueDate is selected without a time.
- Stage 06 time parsing is implemented in the editor ViewModel with invariant `H:mm` / `HH:mm` exact parsing and inline errors.
- Reminder UI and reminder editing remain intentionally deferred to Stage 09; existing ReminderAt is preserved during task edits.
- Official Microsoft Learn browsing was attempted for DatePicker, WPF validation, and editable ComboBox behavior, but network socket access was blocked in this environment; existing project sources and recorded official WPF guidance were used.
- Stage 07 event boundary: `TaskCardTemplate.xaml` contains the card visual; `MainWindow.xaml` declares quadrant drop targets; `MainWindow.xaml.cs` handles mouse threshold, `DoDragDrop`, drop feedback, and forwarding; `MainViewModel.MoveTaskCommand` and `TaskService.MoveTaskAsync` handle business movement.
- No third-party DragDrop package was added.

## Tests run + results

- `dotnet --info` — passed; SDK `10.0.400`, runtime `10.0.11`, MSBuild `18.9.6`.
- `dotnet restore --configfile NuGet.Config` — passed.
- `dotnet build Quadrant.sln -c Debug --no-restore` — passed; 0 warnings, 0 errors.
- `dotnet test Quadrant.sln -c Debug --no-build --no-restore` — passed; 2 tests passed, 0 failed.
- Manual project/reference inspection — passed.
- MainWindow XAML resource/style compilation — passed as part of build.
- `dotnet run --project .\src\Quadrant.App\Quadrant.App.csproj --no-restore` — initially reproduced a startup crash caused by invalid `Double` resources assigned to `Margin`; fixed with `Thickness` tokens.
- Built EXE launch smoke check — passed; process remained running for 5 seconds without exiting.
- `dotnet test .\Tests\Quadrant.Core.Tests\Quadrant.Core.Tests.csproj -c Debug --no-restore` — passed; 14 tests passed, 0 failed.
- `dotnet build Quadrant.sln -c Debug --no-restore` — passed; 0 warnings, 0 errors.
- `dotnet test Quadrant.sln -c Debug --no-build --no-restore` — passed; 15 tests passed, 0 failed.
- Core project boundary inspection — passed; `net10.0`, no Windows-only or SQLite implementation dependency.
- `dotnet test .\Tests\Quadrant.Infrastructure.Tests\Quadrant.Infrastructure.Tests.csproj -c Debug --no-restore` — passed; 4 tests passed, 0 failed.
- `dotnet build Quadrant.sln -c Debug --no-restore` — passed; 0 warnings, 0 errors.
- `dotnet test Quadrant.sln -c Debug --no-build --no-restore` — passed; 18 tests passed, 0 failed.
- `dotnet build Quadrant.sln -c Debug --no-restore` — passed; 0 warnings, 0 errors after Stage 06 implementation.
- `dotnet test Quadrant.sln -c Debug --no-restore` — passed; 19 tests passed, 0 failed.
- `dotnet build Quadrant.sln -c Debug --no-restore` — passed after Stage 07 implementation; 0 warnings, 0 errors.
- `dotnet test Quadrant.sln -c Debug --no-build --no-restore` — passed after Stage 07 implementation; 23 tests passed, 0 failed.
- `dotnet build Quadrant.sln -c Debug --no-restore` — passed after Stage 08 implementation; 0 warnings, 0 errors.
- `dotnet test Quadrant.sln -c Debug --no-build --no-restore` — passed after Stage 08 implementation; 26 tests passed, 0 failed.
- Direct EXE launch after TaskEditorWindow fix — passed; process remained running for 5 seconds.
- `dotnet run --project .\src\Quadrant.App\Quadrant.App.csproj --no-restore` — passed after converter scope fix; process remained running until manually stopped.
- Git generated-file cleanup — `.gitignore` updated; `git rm --cached` could not create `.git/index.lock` due to environment permission, so already-tracked bin/obj entries remain tracked until run in a normal Git-enabled shell.
- SQL parameterization inspection — passed; user values are bound parameters, not interpolated SQL.
- `dotnet build Quadrant.sln -c Debug --no-restore` — passed; 0 warnings, 0 errors.
- `dotnet test Quadrant.sln -c Debug --no-build --no-restore` — passed; 18 tests passed, 0 failed.
- Application EXE launch smoke check — passed; process remained running for 5 seconds after async database initialization.
- Reproduced `dotnet run` crash with `XamlParseException` caused by `Double` assigned to `ColumnDefinition.Width`; fixed with `GridLength`.
- `dotnet run --project .\src\Quadrant.App\Quadrant.App.csproj --no-restore` — passed after fix; process remained running for 8 seconds with empty stdout/stderr.
- Direct EXE launch — passed after fix; process remained running for 5 seconds.

## Manual tests pending

- Windows GUI checks for four populated quadrants, task ordering, independent scrolling, resize, and 100+ synthetic tasks remain pending; no usable Windows UI automation session was available.

## Sources checked

- https://dotnet.microsoft.com/en-us/download/dotnet/10.0 — checked 2026-08-20.
- https://dotnetcli.blob.core.windows.net/dotnet/release-metadata/10.0/releases.json — checked 2026-08-20.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/overview/ — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/communitytoolkit.mvvm/index.json — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/microsoft.data.sqlite/index.json — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/microsoft.net.test.sdk/index.json — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/xunit/index.json — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/xunit.runner.visualstudio/index.json — checked 2026-08-20.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/whats-new/net90 — checked 2026-08-20; WPF Fluent Theme and `ThemeMode` syntax.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/whats-new/net100 — checked 2026-08-20; .NET 10 WPF changes.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/systems/xaml-resources-overview — checked 2026-08-20; ResourceDictionary guidance.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/staticresource-markup-extension — checked 2026-08-20; StaticResource behavior.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/dynamicresource-markup-extension — checked 2026-08-20; DynamicResource behavior.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/controls/control-styles-and-templates — checked 2026-08-20; WPF style/template guidance.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/wpf-brushes-overview — checked 2026-08-20; WPF brush/resource guidance.
- No version-sensitive external API was required for Stage 03; CommunityToolkit.Mvvm remains pinned from Stage 00 and is not used by the Core domain model.
- https://learn.microsoft.com/en-us/dotnet/standard/data/sqlite/ — checked 2026-08-20; Microsoft.Data.Sqlite ADO.NET usage.
- https://learn.microsoft.com/en-us/dotnet/standard/data/sqlite/connection-strings — checked 2026-08-20; connection configuration.
- https://learn.microsoft.com/en-us/dotnet/standard/data/sqlite/parameters — checked 2026-08-20; parameter binding.
- https://learn.microsoft.com/en-us/dotnet/standard/data/sqlite/transactions — checked 2026-08-20; transaction usage.
- https://www.sqlite.org/foreignkeys.html — checked 2026-08-20; SQLite foreign key behavior.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/optimizing-performance-controls — checked 2026-08-20; WPF control virtualization guidance.
- https://learn.microsoft.com/en-us/dotnet/api/system.windows.controls.virtualizingstackpanel.virtualizationmode — checked 2026-08-20; Recycling mode API.
- https://learn.microsoft.com/en-us/dotnet/api/system.windows.controls.virtualizingpanel.isvirtualizing — checked 2026-08-20; virtualization property API.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/drag-and-drop-overview — attempted 2026-08-20; blocked by environment network policy.
- https://learn.microsoft.com/en-us/dotnet/api/system.windows.dragdrop.dodragdrop — attempted 2026-08-20; blocked by environment network policy.
- https://learn.microsoft.com/en-us/dotnet/api/system.windows.uielement.allowdrop — attempted 2026-08-20; blocked by environment network policy.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/controls/datepicker — attempted 2026-08-20; blocked by environment network policy.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/controls/how-to-use-validation-rules-to-implement-validation — attempted 2026-08-20; blocked by environment network policy.
- https://learn.microsoft.com/en-us/dotnet/api/system.windows.controls.combobox.iseditable — attempted 2026-08-20; blocked by environment network policy.

## Known issues

- Working product name `Quadrant` remains provisional.
- Stage 05 GUI manual acceptance is pending because this environment did not expose a usable Windows UI automation session.
- Stage 06 GUI manual acceptance is pending: editor save/cancel, date-only 23:59, exact custom time, invalid time, multiline note, completion, and delete confirmation need Windows GUI verification.
- Already-tracked generated `bin/` and `obj/` files could not be removed from the Git index in this restricted environment.
- Stage 07 GUI manual acceptance is pending: Q1 to Q2, Q4 to Q1, empty-area drop, drag threshold/Esc behavior, 150% DPI, and persistence after restart.
- Stage 08 GUI manual acceptance is pending: filter/search combination, Ctrl+F, Esc reset, completed restore to original quadrant, permanent delete, and overdue status semantics.

## Next stage

`stages/STAGE_09_REMINDER_DOMAIN.md`
