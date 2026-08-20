# V1 Requirements → Stage Traceability

| V1 Requirement | Primary Stage | Supporting Stage |
|---|---:|---:|
| WPF .NET 10 native Fluent | 01 | 02,16 |
| Fixed design system | 02 | 16 |
| SQLite local persistence | 04 | 17 |
| 2×2 quadrants | 05 | 07 |
| Create/Edit/Delete/Complete | 06 | 03,04 |
| Due date/time | 06 | 16 |
| Drag task between quadrants | 07 | 03 |
| All/Today/Overdue | 08 | 03 |
| Search | 08 | 17 |
| Completed history/restore | 08 | 06 |
| Reminder preset/custom | 09 | 12 |
| Windows App SDK foundation | 10 | 18 |
| Native Windows notification | 11 | 12 |
| Single instance | 11 | 14 |
| Complete/Open notification actions | 11 | 12 |
| Scheduled reminder | 12 | 09,10 |
| Snooze 10 min | 12 | 11 |
| Missed reminder in-app recovery | 12 | 17 |
| Global Quick Add | 13 | 06,09 |
| RegisterHotKey | 13 | 14 |
| System tray | 14 | 15 |
| Close-to-tray / clean exit | 14 | 17 |
| Startup with Windows | 15 | 18 |
| Theme System/Light/Dark | 15 | 01,16 |
| Custom quadrant names/subtitles | 15 | 05 |
| DPI/accessibility/keyboard | 16 | all UI stages |
| Idle performance / no polling | 17 | 12,14 |
| Release deployment | 18 | 10 |

任何 Release Gate 功能必须能追溯到至少一个已完成 Stage 和相应验收记录。
