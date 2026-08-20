# Stage 09 — Reminder Domain Rules and Editor UX (No Windows API Yet)

## Goal

把 Reminder 作为业务能力完全写对，但**此 Stage 不接 Windows App SDK**。用 fake/in-memory scheduler 测试 orchestration。

## Implementation

### Presets

编辑器增加：

- None
- At due time
- 10 min before
- 1 hour before
- 1 day before
- Custom

规则：

- preset relative-to-due 要求 DueAt 存在；
- 若计算出的 reminder 已在过去，保存时要求用户确认/改为 custom；默认禁止新建一个已过期 schedule；
- Custom 可独立于 DueAt；
- ReminderAt 最终仍只存一个具体 `DateTimeOffset?`，不必把 preset 永久存 DB。

### ReminderCalculator

Core pure function：

`DateTimeOffset? Calculate(ReminderPreset preset, DateTimeOffset? due, DateTimeOffset? custom)`

用 IClock 验证 future/past。

### TaskService orchestration

- create/update 保存后调用 `IReminderScheduler.RescheduleAsync`；
- reminder null → Cancel；
- complete/delete → Cancel；
- restore 不自动恢复旧 reminder，因为旧 ReminderAt 可能过期；恢复任务时保留字段但若已过期只作为 in-app信息，用户需重新安排。

若你认为 restore 策略需要变更，必须写 STATUS 并请求批准。

### Fake scheduler tests

验证每个 TaskService mutation 的 scheduler 调用次数和 task id。

## UI

Reminder relative presets 只在 DueAt 存在时 enable；Custom 展开 DatePicker + TimeInput。

不要做复杂“多个提醒”。一个 task 最多一个 ReminderAt。

## Acceptance

自动测试覆盖六种 preset、跨日、提醒早于 due、custom 无 due、complete/delete cancel。

Manual：editor UX 清晰，Reminder 与 Due 明显是两个概念。

## DO NOT

- 不加 Microsoft.WindowsAppSDK；
- 不弹系统 Toast；
- 不做 polling timer；
- 不做 multiple reminders。

## Handoff

STATUS 记录 reminder rules。下一 Stage 10。
