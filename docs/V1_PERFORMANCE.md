# V1 Performance And Reliability

Stage 17 measurement record. Measurements were taken on 2026-08-21 from the local Debug build.

## Environment

- OS: Windows, x64 process
- SDK: .NET SDK 10.0.400
- Runtime: .NET 10.0.11
- Build: `Debug`, `dotnet build Quadrant.sln --no-restore`
- Diagnostics tools: `dotnet-counters` and `dotnet-trace` were not installed on this machine, so no EventPipe counter or trace result is claimed.

## Measurements

| Scenario | Result | Method |
|---|---:|---|
| Cold start to first main window | 1062 ms | Start the built EXE, poll for a non-zero `MainWindowHandle`, stop after observation. |
| 1000 active task SQLite load | 4 ms | Temporary database, 1000 parameterized inserts, then `GetActiveAsync` with `Stopwatch`. |
| Visible idle, 60 seconds, cumulative CPU | 1.609 s at 40 s; 2.297 s at 50-60 s | `Get-Process` samples against a real app process. This is about 2.7 seconds of CPU over 60 seconds, not the SPEC target average. |
| Visible idle Working Set | 157.78-161.52 MB | Same 60-second process sample. |
| Visible idle Private Memory | 86.08-87.79 MB | Same 60-second process sample. |
| Tray-only idle, 60 seconds | Not measured | Requires interactive tray session and reliable process-state confirmation. |
| Managed heap baseline | Not measured | `dotnet-counters`/`dotnet-trace` unavailable. |
| 1000-task scroll/filter/search/drag-drop | Not measured | Requires interactive WPF GUI automation; SQLite load baseline is covered. |

## Audit

- No `DispatcherTimer` or `System.Threading.Timer` business loop found.
- No background `while (true)` loop found.
- No `HttpClient`, `WebClient`, or network package found.
- Main quadrant lists use `ListBox` with `CanContentScroll`, virtualization enabled, and Recycling mode.
- Database initialization recreates the missing parent directory before opening SQLite.
- Unsupported/newer schema versions fail closed; the app shows the database path and stops rather than writing.
- Reminder, notification, tray, startup, and hotkey failures degrade the affected integration and are recorded in `%LOCALAPPDATA%\Quadrant\logs\quadrant.log`.
- Diagnostic logs are warning/error only, have a 1 MB active-file limit, and retain three rotated files.
- No telemetry or upload path was added.

## Findings And Follow-up

- The 1000-row database read is comfortably measurable and does not justify a cache or SQL redesign.
- The visible idle CPU sample is not sufficient to claim the SPEC target of less than 0.2% average CPU; repeat with `dotnet-counters` or WPR/WPA in a release-like interactive session before release sign-off.
- Working Set and Private Memory are recorded as observed values, not fixed product limits.

