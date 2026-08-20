# Stage Index

**Luna 每次只执行一个 Stage。** 每个 Stage 都必须先读 `AGENTS.md / SPEC.md / ARCHITECTURE.md / DESIGN_SYSTEM.md / STATUS.md`，再读当前 Stage。

| Stage | 文件 | 目标 |
|---:|---|---|
| 00 | `stages/STAGE_00_REPO_BOOTSTRAP.md` | 建解决方案、联网固定稳定版本、测试骨架 |
| 01 | `stages/STAGE_01_FLUENT_SHELL.md` | .NET 10 WPF 原生 Fluent 主窗口壳 |
| 02 | `stages/STAGE_02_DESIGN_TOKENS.md` | ResourceDictionary / 固定 Design Tokens |
| 03 | `stages/STAGE_03_CORE_DOMAIN.md` | 纯 Core 模型、规则、服务接口、单测 |
| 04 | `stages/STAGE_04_SQLITE_STORAGE.md` | SQLite schema/migrations/repository |
| 05 | `stages/STAGE_05_MAIN_QUADRANT_VIEW.md` | DB → 2×2 四象限真实只读视图 |
| 06 | `stages/STAGE_06_TASK_EDITOR_CRUD.md` | Task Editor、CRUD、Due date/time、Note |
| 07 | `stages/STAGE_07_DRAG_DROP.md` | WPF 原生拖拽换象限 |
| 08 | `stages/STAGE_08_FILTER_SEARCH_COMPLETED.md` | All/Today/Overdue、Search、Completed |
| 09 | `stages/STAGE_09_REMINDER_DOMAIN.md` | Reminder preset/规则/scheduler abstraction |
| 10 | `stages/STAGE_10_WINDOWS_APP_SDK_FOUNDATION.md` | 联网核验并接入 Windows App SDK |
| 11 | `stages/STAGE_11_APP_NOTIFICATIONS_SINGLE_INSTANCE.md` | 即时 App Notification + activation + 单实例 |
| 12 | `stages/STAGE_12_SCHEDULED_REMINDERS.md` | Scheduled reminder + snooze + missed UX |
| 13 | `stages/STAGE_13_QUICK_ADD_HOTKEY.md` | Quick Add + Win32 RegisterHotKey |
| 14 | `stages/STAGE_14_TRAY_LIFECYCLE.md` | NotifyIcon + close-to-tray + clean exit |
| 15 | `stages/STAGE_15_SETTINGS_STARTUP.md` | Settings、Theme、Quadrant naming、开机启动 |
| 16 | `stages/STAGE_16_ACCESSIBILITY_POLISH.md` | DPI、键盘、Focus、High Contrast、视觉收口 |
| 17 | `stages/STAGE_17_PERFORMANCE_RESILIENCE.md` | 性能实测、虚拟化、故障恢复、诊断 |
| 18 | `stages/STAGE_18_RELEASE_PACKAGING.md` | Release/部署 profile/V1 Gate |

## 拆分原则

1. **一次只引入一种新的复杂性。** UI、DB、Reminder、Windows App SDK、Hotkey、Tray、Startup 分开。
2. **Windows 通知拆成 4 段。** Reminder 业务规则 → SDK 基础 → activation/单实例 → schedule，避免 Luna 一次处理过多 WinRT 生命周期细节。
3. **每阶段都能验收。** 不依赖“下一阶段写完以后再看看”。
4. **STATUS.md 是上下文压缩层。** 下一次会话不依赖 Luna 记住上一个长会话。
