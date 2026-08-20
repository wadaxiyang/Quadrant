# Stage 12 — Scheduled Notifications, Snooze, Cancel/Reschedule, Missed Reminder UX

## Goal

把 `IReminderScheduler` 实现为 Windows scheduled notification，并完成 V1 提醒闭环。

## Before coding — MUST browse

重新查最新：

- Schedule an app notification；
- `ScheduledToastNotification`；
- schedule add/cancel APIs；
- ID/tag/group 限制；
- 当前 WPF/unpackaged compatibility；
- delivery window 说明。

如果旧 UWP API 页与 Windows App SDK 2026 指南存在差异，以最新跨 WPF 指南为准。

## Implementation

### `WindowsReminderScheduler`

Implement：

- ScheduleAsync
- CancelAsync
- RescheduleAsync

Schedule ID：`q` + task id hex，必须 <= 当前官方 ID limit（规划时文档为 16 chars）。

Payload 使用 `AppNotificationBuilder` 生成 text/buttons，再取 payload 创建 scheduled toast（按当前官方示例）。

### Buttons

最终 scheduled notification：

- Complete
- Snooze 10 min
- Open

Snooze activation：

1. load task；
2. if missing/completed → no-op；
3. ReminderAt = Clock.Now + 10 min；
4. persist；
5. reschedule；
6. 不 show MainWindow。

### Cancel/Reschedule

编辑 ReminderAt / 完成 / 删除必须取消旧 schedule。

OS 操作失败策略：

- task DB 保存仍成功；
- UI 显示“任务已保存，但提醒未注册”；
- log technical error；
- 不吞异常。

### Startup reconciliation

DB 是 source of truth。

启动时：

- future ReminderAt → 确保 schedule 存在；
- past ReminderAt + active → 加入 `PossiblyMissedReminders` in-app banner/list；
- 不自动重新 Toast past reminder。

如果当前 API 不可靠支持 enumerate scheduled items，应采取幂等 cancel-by-id/re-add 或其他官方支持方法，并记录技术差异；不得编造 API。

### 5-minute window

代码旁注和 About/README developer note 保留官方事实：scheduled notification delivery window ~5 min；V1 missed list 是补救，不是 guaranteed delivery。

## Tests

Core with fake scheduler：snooze/complete/update。

Integration 不能假装 Windows Toast 可在 CI 自动验证；把 manual test 写明。

Manual：

- 设 2 分钟后提醒，关闭 app，时间到；
- Open/Complete/Snooze；
- 改时间后旧提醒不出现；
- 完成后不出现旧提醒；
- 电脑休眠跨提醒时间后启动，出现 in-app missed 信息。

## DO NOT

- 不做 background polling；
- 不声称关机多久都能保证补发；
- 不加第二个 reminder。

## Handoff

STATUS 写实际 scheduling API。下一 Stage 13。
