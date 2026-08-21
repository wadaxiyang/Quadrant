# Quadrant Deployment

## Recommended V1 profile

The V1 release candidate uses a `win-x64` framework-dependent folder publish:

```powershell
dotnet publish .\src\Quadrant.App\Quadrant.App.csproj `
  -p:PublishProfile=Quadrant-win-x64 `
  --no-restore
```

Output:

`artifacts/publish/Quadrant-win-x64/`

Copy the complete folder. Do not copy only the EXE: the folder contains the WPF, SQLite, Windows App SDK, and native runtime dependencies selected for `win-x64`.

## Prerequisites

- Windows 11 x64.
- .NET 10 x64 runtime, because this profile is framework-dependent.
- Windows App Runtime 2.4.0 x64, matching the package used by this build.
- Microsoft Visual C++ Redistributable for the target architecture.

For an unpackaged Windows App SDK app, install the official Windows App SDK runtime installer before launching Quadrant. The installer can be run silently with `WindowsAppRuntimeInstall.exe --quiet` when it is integrated into an approved deployment process.

## First run

1. Extract the complete publish folder.
2. Install the prerequisites above.
3. Run `Quadrant.App.exe`.
4. The database is created at `%LOCALAPPDATA%\Quadrant\quadrant.db`.
5. Diagnostic warnings and errors are written to `%LOCALAPPDATA%\Quadrant\logs\quadrant.log`.

The app uses a user-level startup entry only when enabled in Settings. It does not require administrator rights for normal task storage or startup registration.

## Verification commands

```powershell
Get-Item .\artifacts\publish\Quadrant-win-x64\Quadrant.App.exe
Get-ChildItem .\artifacts\publish\Quadrant-win-x64 -Recurse | Measure-Object
Start-Process .\artifacts\publish\Quadrant-win-x64\Quadrant.App.exe
```

For a background-start smoke check:

```powershell
Start-Process .\artifacts\publish\Quadrant-win-x64\Quadrant.App.exe -ArgumentList '--background'
```

## Deliberate non-options

- Single-file publish is disabled. WPF, WinForms tray support, embedded resources, and Windows App SDK activation have not been validated as a single-file deployment combination.
- Trimming and ReadyToRun are disabled to protect WPF reflection/resource behavior and startup integration.
- MSIX and an installer are out of V1 scope. This repository does not include signing keys or an updater.

## Data and uninstall

The publish folder contains binaries only. User data remains under `%LOCALAPPDATA%\Quadrant` and is not deleted when the publish folder is removed. Back up `quadrant.db` before manually removing user data.
