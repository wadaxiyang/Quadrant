# Quadrant V1 Architecture

## 1. 固定技术栈

- **Language**: C#
- **Runtime**: .NET 10
- **UI**: WPF
- **Theme**: WPF native Fluent Theme / `ThemeMode`
- **MVVM**: CommunityToolkit.Mvvm
- **Storage**: Microsoft.Data.Sqlite, raw SQL
- **Notifications**: Windows App SDK App Notifications + Windows scheduled toast APIs as verified at implementation time
- **Global hotkey**: Win32 `RegisterHotKey` / `UnregisterHotKey`
- **Tray**: `System.Windows.Forms.NotifyIcon` hosted by WPF app
- **Startup**: user-level Windows startup mechanism, V1 default implementation HKCU Run entry unless current Microsoft guidance at implementation time gives a better unpackaged-desktop route

## 2. Solution Layout

```text
Quadrant.sln
Directory.Packages.props
global.json

src/
  Quadrant.Core/
    Models/
    Enums/
    Interfaces/
    Services/

  Quadrant.Infrastructure/
    Storage/
    Notifications/
    Windows/
    Logging/

  Quadrant.App/
    App.xaml
    App.xaml.cs
    Resources/
    Views/
    ViewModels/
    Controls/
    Behaviors/
    Converters/

Tests/
  Quadrant.Core.Tests/
  Quadrant.Infrastructure.Tests/

docs/
  (optional implementation notes)
```

Dependency direction:

```text
Quadrant.App --------------------> Quadrant.Core
      |                               ^
      +----> Quadrant.Infrastructure -+

Quadrant.Infrastructure ---------> Quadrant.Core
Quadrant.Core -------------------> no Windows/UI/SQLite implementation dependency
```

## 3. 为什么分三个项目

`Core` 保持纯业务：

- 便于 Luna 在短上下文中理解；
- 能独立单测；
- 防止 WPF/Win32 逻辑渗入任务规则。

`Infrastructure` 集中所有平台/持久化细节：

- SQLite；
- Notification；
- Hotkey；
- Tray；
- Startup；
- app data path。

`App` 只负责：

- WPF Views；
- ViewModels；
- composition root；
- window lifecycle。

## 4. 不使用 DI Container

V1 不引入 `Microsoft.Extensions.DependencyInjection`。

在 `App.xaml.cs` 做显式 composition root：

```text
TaskRepository
ReminderScheduler
TaskService
SettingsService
MainViewModel
```

通过构造函数注入。

原因：项目规模很小，显式依赖更容易让 Agent 理解，也减少隐藏生命周期问题。

## 5. Domain Model

建议：

```csharp
public sealed record TaskItem(
    long Id,
    string Title,
    int QuadrantId,
    DateTimeOffset? DueAt,
    DateTimeOffset? ReminderAt,
    string? Note,
    bool IsCompleted,
    DateTimeOffset? CompletedAt,
    DateTimeOffset CreatedAt,
    DateTimeOffset UpdatedAt);
```

实际实现可根据 Repository 更新模式使用 class / immutable record，但字段语义不得漂移。

Quadrant：固定 ID 1–4，仅 Name / Subtitle 可配置。

## 6. Application Services

### `ITaskRepository`

至少：

- `GetActiveAsync`
- `GetCompletedAsync`
- `GetByIdAsync`
- `CreateAsync`
- `UpdateAsync`
- `SetCompletedAsync`
- `DeleteAsync`

### `TaskService`

负责跨基础设施的一致性流程：

- create/update task；
- schedule/cancel reminder；
- complete task and cancel reminder；
- snooze reminder；
- validate reminder/due relationships。

ViewModel 不直接组合 Repository + Notification。

### `IReminderScheduler`

抽象：

- `ScheduleAsync(TaskItem task)`
- `CancelAsync(long taskId)`
- `RescheduleAsync(TaskItem task)`

Windows schedule 视为 DB 的派生状态。

### `IHotkeyService`

- Register
- Unregister
- HotkeyPressed event

### `IStartupService`

- `IsEnabled`
- `Enable`
- `Disable`

### `ITrayService`

可以在 App/Infrastructure 边界实现，不能把业务逻辑写进 NotifyIcon event handler。

## 7. SQLite

数据库路径：

`%LOCALAPPDATA%\Quadrant\quadrant.db`

建议 Schema：

```sql
CREATE TABLE schema_version (
  version INTEGER NOT NULL
);

CREATE TABLE quadrants (
  id INTEGER PRIMARY KEY CHECK (id BETWEEN 1 AND 4),
  name TEXT NOT NULL,
  subtitle TEXT NOT NULL
);

CREATE TABLE tasks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  title TEXT NOT NULL,
  quadrant_id INTEGER NOT NULL,
  due_at TEXT NULL,
  reminder_at TEXT NULL,
  note TEXT NULL,
  is_completed INTEGER NOT NULL DEFAULT 0,
  completed_at TEXT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (quadrant_id) REFERENCES quadrants(id)
);

CREATE INDEX ix_tasks_active_quadrant
ON tasks(is_completed, quadrant_id);

CREATE INDEX ix_tasks_due
ON tasks(is_completed, due_at);

CREATE INDEX ix_tasks_reminder
ON tasks(is_completed, reminder_at);
```

时间格式固定为 ISO-8601 round-trip (`O`) 文本，读回 `DateTimeOffset`。

原因：可读、可逆、保留 offset，V1 数据量很小，无需为 epoch 微优化。

SQLite 初始化：

- `PRAGMA foreign_keys = ON`；
- 多语句 mutation 使用 transaction；
- migration 按整数版本逐步执行；
- 不使用 destructive migration。

## 8. Reminder Architecture

### 8.1 Source of truth

SQLite 中 `ReminderAt` 是业务事实。

Windows scheduled notification 是可重建的 side effect。

### 8.2 schedule id

Windows `ScheduledToastNotification.Id` 有长度限制（官方文档当前为 16 chars）。

建议从 task id 派生：

`q` + task id 的十六进制字符串。

例：task 1234 → `q4d2`。

### 8.3 Create/Update

保存任务流程：

1. validate；
2. transaction persist task；
3. cancel previous schedule（如有）；
4. schedule new reminder（如未来时间）；
5. 若 OS schedule 操作失败，记录错误并向用户显示“任务已保存，但提醒未注册”；
6. 不回滚任务数据来伪装 OS schedule 原子性。

### 8.4 Complete/Delete

任务完成/删除：

1. DB mutation；
2. cancel schedule；
3. schedule cancel 失败记录日志，下一启动做 reconciliation。

### 8.5 Snooze

Notification action `snooze10`：

- load task；
- if completed/deleted → no-op；
- `ReminderAt = Now + 10m`；
- persist；
- reschedule；
- 不弹主窗口。

### 8.6 Missed reminder

因为官方 Scheduled Notification 存在约 5 分钟 delivery window，V1 不承诺离线绝对补发。

启动时：

- query active tasks with past `ReminderAt`；
- 作为 in-app “可能错过的提醒” banner/list；
- 用户可 dismiss / open；
- 不自动重复 Toast。

## 9. Filtering

使用一个 `TaskFilter` value：

- All
- Today
- Overdue

过滤可以在 ViewModel 内存集合做，因为 V1 活跃任务量小；Repository 仍只负责读取 active set。

搜索同样在本地集合过滤。

1000+ 任务性能阶段若出现问题再优化 SQL query，不提前复杂化。

## 10. Drag & Drop

WPF 原生 DragDrop：

- View 捕获 Mouse/Drag events；
- payload 仅携带 task id；
- drop 到 quadrant 后调用 `MoveTaskCommand(taskId, quadrantId)`；
- TaskService 更新 DB；
- ViewModel collection 重排。

不引第三方 DragDrop 框架。

## 11. Global Hotkey

使用 Win32：

- `RegisterHotKey`
- `UnregisterHotKey`
- `WM_HOTKEY`
- `MOD_NOREPEAT`

WPF 通过 `WindowInteropHelper` + `HwndSource.AddHook` 接收消息。

默认：`Ctrl + Alt + Q`。

F12 与 Win-key 组合不作为默认值。

## 12. Tray / Lifecycle

使用 `System.Windows.Forms.NotifyIcon`：

- 需要 App 项目开启 WinForms reference；
- NotifyIcon 只负责 system tray shell；
- command 调用 App lifecycle service；
- 真正 Exit 时 dispose icon、unregister hotkey、unregister notification manager。

Close-to-tray 时不要结束进程。

## 13. Startup

V1 internal/unpackaged app：优先用户级 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`，value 指向当前 exe + `--background`。

要求：

- path 加引号；
- 失败不崩溃；
- 用户关闭设置时删除 value；
- `--background` 启动不弹 MainWindow。

实际 Stage 执行前仍必须联网核验最新 Microsoft 指南。

## 14. Theme

App-level `ThemeMode`：

- System
- Light
- Dark

不要自行维护一套完整 Dark Palette。

Quadrant accent 使用自定义资源，但背景、正文、border 使用 Fluent/system resources。

## 15. Packaging Strategy

开发阶段：unpackaged WPF 最简单。

Release Stage 再决定两种 profile：

1. **Framework-dependent/internal**：较小，但要求 .NET/Windows App SDK runtime；
2. **Self-contained**：部署目录更大，但依赖随 app 发布。

Windows App SDK 当前官方支持 self-contained 方案；实际发布属性必须在 Release Stage 重新查最新官方文档后落地。

V1 不要求“单 exe”作为硬目标，不能为单文件牺牲稳定通知和部署正确性。

## 16. Single Instance

V1 必须单实例，原因：避免重复 tray icon、重复全局热键注册、notification activation 多进程竞争和不必要的 SQLite 并发。

在 Windows App SDK 已接入后，优先使用当前官方 `Microsoft.Windows.AppLifecycle.AppInstance`：

- `FindOrRegisterForKey("main")`
- `RedirectActivationToAsync`
- current instance 接收 redirected activation

WPF 的具体启动/Dispatcher 顺序必须以执行时最新 Microsoft WPF 示例为准；不要照搬 WinUI 3 的 custom Main 模板。
