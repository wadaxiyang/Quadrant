# V1 Release Checklist

Status for the Stage 18 release candidate. `Not Tested` means the current environment did not provide the required clean Windows 11 or interactive system session.

| SPEC V1 Gate | Status | Evidence |
|---|---|---|
| Four-quadrant CRUD | Pass | Existing Core/Infrastructure tests and Release build. |
| Drag between quadrants | Not Tested | Requires interactive WPF GUI. |
| Due and Reminder separation | Pass | Existing editor, service, repository, and scheduler tests. |
| Windows native notifications | Not Tested | Requires interactive notification session on the target machine. |
| Notification Complete/Snooze/Open | Not Tested | Requires notification activation session. |
| Today/Overdue | Pass | Existing Core tests. |
| Search | Not Tested | Requires interactive GUI verification. |
| Completed list and restore | Pass | Existing repository/service tests. |
| Quick Add and global hotkey | Not Tested | Requires foreground-app and hotkey session. |
| Tray | Not Tested | Requires interactive notification-area session. |
| Startup | Not Tested | Requires target-machine HKCU Run and background-focus verification. |
| SQLite persistence | Pass | Migration, CRUD, timestamp, foreign-key, and idempotence tests. |
| Light/Dark/System | Not Tested | Requires interactive visual session. |
| DPI/keyboard/accessibility | Not Tested | See `docs/V1_UI_ACCEPTANCE.md`. |
| Performance/resilience | Partial | See `docs/V1_PERFORMANCE.md`; GUI profiling and clean-machine fault drills remain. |
| Release build and deployment docs | Pass | Release build/publish profile and `docs/DEPLOYMENT.md`. |

## Release decision

`0.9.0` feature-complete release candidate. Not `1.0.0`: interactive Windows notification/tray/hotkey/DPI/accessibility and clean-machine deployment checks remain open.
