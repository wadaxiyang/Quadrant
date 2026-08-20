# Stage 14 — System Tray and Window Lifecycle

## Goal

让应用长期常驻但安静：关闭可到托盘，托盘可恢复/新建/退出，真正退出清理系统资源。

## Before coding — MUST browse

查 .NET 10 `System.Windows.Forms.NotifyIcon` 官方 API。确认 WPF 项目混用 WinForms 所需 project property/reference。

## Implementation

### Project

按官方方式开启 WinForms interop，例如 `UseWindowsForms=true`（核验当前 .NET 10 属性）。避免 namespace 冲突使用完整命名或 alias。

### `TrayService`

- icon from embedded `.ico` resource；
- Text short；
- Visible true after app initialized；
- DoubleClick → show/restore/activate MainWindow；
- Context menu：Quick Add / Show / Exit。

系统托盘 context menu 可以是 WinForms 标准菜单，不为了 Fluent 一致性再造隐藏 WPF popup。

### Close behavior

设置尚未落地前先用默认：Close → hide to tray。

真正退出必须走统一 `ShutdownCoordinator`：

1. mark exiting；
2. dispose tray；
3. unregister hotkey；
4. unregister AppNotificationManager（按官方要求）；
5. release resources；
6. `Application.Shutdown()`。

关闭到托盘不能执行上述真正退出 cleanup。

### No zombie windows

- MainWindow hide 后可重复 show；
- QuickAdd owner/lifetime 不让 app shutdown unexpectedly；
- `ShutdownMode` 需要显式设置，不依赖默认 MainWindow close。

## Acceptance

- X 关闭 → taskbar 消失但 tray 存在；
- tray show 恢复同一个 window；
- tray Quick Add；
- tray Exit 后进程消失；
- exit 后 hotkey 释放；
- 重新启动只有一个 tray icon。

## DO NOT

- 不创建后台 service exe；
- 不每秒更新 tray tooltip；
- 不做 tray 动画。

## Handoff

STATUS 写 lifecycle 状态机。下一 Stage 15。
