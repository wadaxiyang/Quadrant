# STATUS — Codex Handoff

## Current stage

`STAGE_00_REPO_BOOTSTRAP` — passed

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
- No architecture deviations recorded.

## Tests run + results

- `dotnet --info` — passed; SDK `10.0.400`, runtime `10.0.11`, MSBuild `18.9.6`.
- `dotnet restore --configfile NuGet.Config` — passed.
- `dotnet build Quadrant.sln -c Debug --no-restore` — passed; 0 warnings, 0 errors.
- `dotnet test Quadrant.sln -c Debug --no-build --no-restore` — passed; 2 tests passed, 0 failed.
- Manual dependency-direction and target-framework inspection — passed.

## Manual tests pending

- No Stage 00 manual tests remain. GUI behavior is intentionally deferred to Stage 01.

## Sources checked

- https://dotnet.microsoft.com/en-us/download/dotnet/10.0 — checked 2026-08-20.
- https://dotnetcli.blob.core.windows.net/dotnet/release-metadata/10.0/releases.json — checked 2026-08-20.
- https://learn.microsoft.com/en-us/dotnet/desktop/wpf/overview/ — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/communitytoolkit.mvvm/index.json — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/microsoft.data.sqlite/index.json — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/microsoft.net.test.sdk/index.json — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/xunit/index.json — checked 2026-08-20.
- https://api.nuget.org/v3-flatcontainer/xunit.runner.visualstudio/index.json — checked 2026-08-20.

## Known issues

- Working product name `Quadrant` remains provisional.

## Next stage

After installing/verifying the SDK and passing Stage 00 acceptance, the next stage is:

`stages/STAGE_01_FLUENT_SHELL.md`
