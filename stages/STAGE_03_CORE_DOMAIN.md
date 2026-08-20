# Stage 03 — Core Domain, Contracts, and Business Tests

## Goal

在**完全不引用 Windows/WPF/SQLite 实现**的 `Quadrant.Core` 中固定业务模型和服务契约。

## Before coding

此阶段不依赖版本敏感 Windows API，但仍应确认 CommunityToolkit.Mvvm 不需要进入 Core domain model。默认：**domain model 保持普通 C# 类型**，MVVM 只在 App ViewModel 使用。

## Implement

### Models

建立：

- `TaskItem`
- `TaskDraft` / `TaskUpdate`（避免 UI 直接构造带 Id 的完整 entity）
- `QuadrantDefinition`

字段遵守 SPEC。

### Enums / value concepts

- `TaskFilter { All, Today, Overdue }`
- `ReminderPreset { None, AtDueTime, TenMinutesBefore, OneHourBefore, OneDayBefore, Custom }`

### Interfaces

- `ITaskRepository`
- `IQuadrantRepository`
- `IReminderScheduler`
- 可选 `IClock`，强烈建议用于 today/overdue/reminder unit tests，生产实现用 system clock。

### Services

`TaskRules` / `TaskValidationService`：

- title trim 后不可空；
- quadrant only 1..4；
- 完成时设置 CompletedAt；恢复时清空；
- 计算 Today / Overdue；
- reminder preset 计算函数可以先放签名，详细策略 Stage 09 完善。

`TaskService`：先定义 CRUD orchestration 接口/骨架，使 Stage 06 起 ViewModel 不直接操作 repository。当前 `IReminderScheduler` 可由 App 传 `NoOpReminderScheduler`。

## Tests

Core tests 覆盖：

- 空标题失败；
- quadrant 越界失败；
- today 边界；
- overdue 只对未完成生效；
- complete/restore 时间语义；
- `IClock` fake 确保测试不依赖真实当前时间。

## DO NOT

- Core 不引用 WPF；
- Core 不引用 Microsoft.Data.Sqlite；
- Core 不包含 `MessageBox`；
- Core 不拼 `%LOCALAPPDATA%`；
- 不写 notification payload。

## Acceptance

- Core 可单独 `dotnet test`；
- project file 无 Windows-only package；
- TaskService contract 足以支撑后续 CRUD。

## Handoff

STATUS 记录最终 domain fields 和接口。下一阶段 Stage 04。
