# Stage 06 — Task Editor and Core CRUD Workflow

## Goal

完成日常任务 CRUD：新建、编辑、完成、恢复基础、删除、Due Date/Time、Note。**Reminder UI 暂不加入**，Stage 09 单独实现。

## Before coding — MUST browse

重新核验 WPF .NET 10 `DatePicker` Fluent 支持、WPF validation pattern、editable ComboBox 行为。不要找第三方 TimePicker。

## Implementation

### TaskEditorDialog

新建 `TaskEditorWindow` 或 owner-bound dialog（保持单个简单 Window，不引 dialog framework）。

字段：

- Title `TextBox`；
- Quadrant 1..4；
- Due Date `DatePicker`；
- Due Time editable `ComboBox`；
- Note multiline plain TextBox；
- Cancel / Save。

### Time input control

建立可复用 `TimeInputControl` 或 ViewModel helper：

- 候选每 15 分钟；
- 接受 `9:05`, `09:05`；
- 输出 `TimeOnly?`；
- validation inline；
- blank 表示：如果有日期但无时间，产品语义必须固定。

**V1 决策：只选日期、不选时间时，DueAt 使用该本地日期的 23:59。** 这避免“有日期但 DateTimeOffset? 又缺 time”的双重模型。UI 显示时可标为“当天”。如果实现者认为该决策有问题，必须先报告，不得静默换规则。

### TaskService

所有 create/update/complete/delete 经 `TaskService`。

当前 reminder scheduler 是 NoOp；完成/删除仍调用 Cancel，确保未来接入不用重构 ViewModel。

### Main updates

成功保存后只更新受影响 collection，不必每次重载整个 DB；若实现复杂可在 V1 先可靠地 refresh active list，但必须测量并避免 UI 卡顿。

### Delete

使用简单 confirmation。删除完成后卡片消失。

### Complete

完成立即从 active 四象限消失；CompletedAt = clock.Now。

## Tests

Core：TaskService create/update/complete/delete interaction 用 fake repository/scheduler。

Infrastructure：DB mutation 已在 Stage 04。

Manual：

- title trim；
- Chinese/English title；
- quote 字符；
- note 多行；
- due date only；
- due date + exact custom time；
- invalid time 不可保存；
- Esc cancel 无修改。

## DO NOT

- 不加 reminder control；
- 不加 recurrence；
- 不加 tag/priority；
- 不加 autosave-on-every-keystroke。

## Handoff

STATUS 记录日期-only 语义与 time parser。下一阶段 Stage 07。
