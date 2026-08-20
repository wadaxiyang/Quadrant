# Stage 07 — Native WPF Drag & Drop Between Quadrants

## Goal

实现任务卡片拖入其他象限，形成四象限最核心交互。只搬象限，不实现任意排序。

## Before coding — MUST browse

查当前 WPF Drag-and-Drop 官方文档：`DragDrop.DoDragDrop`, `AllowDrop`, DragEnter/DragOver/Drop。不要引第三方 DragDrop NuGet。

## Implementation

### Payload

DataObject 只携带 task id（和必要的内部 format name），不要序列化整个 TaskItem。

### View code-behind boundary

允许在：

- TaskCard control；
- Quadrant panel；

的 code-behind 处理鼠标/drag event，但 Drop 后调用 ViewModel command：

`MoveTaskCommand(long taskId, int targetQuadrantId)`。

业务更新在 TaskService/Repository。

### UX

- 鼠标拖动达到系统阈值后开始 drag，避免单击变拖拽；
- 目标象限 DragOver 只做轻量 border/surface feedback；
- Drop 同象限 no-op；
- DB 更新失败时 UI 恢复原位置并显示错误，不允许前端看似移动但数据库没变。

### No manual order

V1 象限内仍按 DueAt/CreatedAt 排序。不要因为 drag/drop 顺手添加 `SortOrder` 字段。

## Tests

TaskService `MoveTask` unit test：

- valid 1..4；
- same quadrant no-op；
- invalid target rejected。

Manual：

- Q1 → Q2；
- Q4 → Q1；
- drop empty area；
- drag then Esc；
- 150% DPI；
- 触发后 DB 重启仍在新象限。

## DO NOT

- 不做拖拽任意排序；
- 不做 multi-select；
- 不做拖出窗口删除；
- 不使用动画库。

## Handoff

STATUS 记录事件处理所在文件，确保业务逻辑未进入 code-behind。下一 Stage 08。
