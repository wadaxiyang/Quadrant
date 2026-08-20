# Stage 11 — App Notification Activation, Buttons, and Single Instance

## Goal

建立**即时 Windows 原生通知 + activation routing + 单实例**。Scheduled reminder 留到 Stage 12。

## Before coding — MUST browse

重新查：

- “Use app notifications with a .NET app” WPF 当前步骤；
- `AppNotificationManager.Register` 的调用顺序；
- `NotificationInvoked`；
- `AppNotificationBuilder` buttons；
- Windows App SDK `AppInstance` single-instance / WPF activation redirection 官方示例。

特别注意：官方当前文档要求 notification handler/register 与 `GetActivatedEventArgs` 的先后顺序，不得凭记忆写。

## Architecture

建立：

```text
Infrastructure/Notifications/
  WindowsAppNotificationService.cs
  NotificationActivationParser.cs

Infrastructure/Windows/
  SingleInstanceService.cs (或 app lifecycle wrapper)
```

### App startup order

按最新 WPF 官方文档实现：

1. 必要 WASDK init；
2. notification invoked handler；
3. notification manager Register；
4. 获取 activation args / single-instance registration；
5. 若非 current instance，redirect activation 并干净退出；
6. current instance subscribe redirected activation；
7. 创建/激活 WPF windows。

如果最新官方 WPF 示例顺序不同，以最新文档为准并记 STATUS。

### Single instance

使用 `Microsoft.Windows.AppLifecycle.AppInstance.FindOrRegisterForKey("main")` / `RedirectActivationToAsync`，不要 named mutex + 自制 named pipe，除非官方 WPF 路径在实际版本不可用且用户批准替代方案。

普通第二次启动：激活已有 MainWindow。

### Notification immediate test

`WindowsAppNotificationService.ShowTaskNotification(task)`：即时 Toast，先用于 manual testing。

按钮：

- Complete
- Open

本 Stage 可以加入 Snooze action parser，但如果 scheduled scheduler 尚未实现，**不要在产品 toast 显示一个不能工作的 Snooze 按钮**。

Activation argument 至少包含：

- action
- taskId

必须 validate taskId，不信任 notification args。

### Complete action

收到 complete：后台调用 TaskService complete；如果主窗口存在，Dispatcher 更新 collections；不要强制 show window。

### Open action

激活 MainWindow + 打开 editor for id。

## Acceptance

Manual：

1. app running → immediate notification → Open；
2. app running → Complete；
3. app closed → notification activation 启动 app 并路由；
4. app 已开时再双击 exe → 不出现第二个主实例；
5. 激活重定向不 deadlock WPF Dispatcher；
6. elevated/admin 模式不是支持目标，若官方通知不支持 admin，在文档说明。

## DO NOT

- 不做 scheduled time；
- 不做 fake toast；
- 不为 single-instance 自建后台 server。

## Handoff

STATUS 必须写 startup/activation 顺序。下一 Stage 12。
