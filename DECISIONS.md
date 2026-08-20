# Fixed Decisions / Mini ADRs

## D-001 — WPF over Slint/Tauri/Electron

**Decision:** C# + .NET 10 + WPF native Fluent.

**Reason:** 在 Windows 单平台、性能重要、同时希望 Codex/Luna 能利用大量成熟 XAML/MVVM 语料自动生成更成熟 UI 的约束下，WPF 是性能、AI 可实现性、Windows 集成和 UI 成熟度的平衡点。

## D-002 — Native Fluent only for V1

**Decision:** 不引 WPF UI / Material Design 等第三方主题。

**Reason:** .NET 9+ WPF 已提供原生 Fluent Theme；.NET 10 继续补足样式。减少依赖，也防止 Agent 在多个设计体系间混写。

## D-003 — SQLite without EF Core

**Decision:** Microsoft.Data.Sqlite + explicit SQL + small migration runner.

**Reason:** 数据模型极小；EF Core 增加概念、依赖、迁移机制和上下文负担，没有必要。

## D-004 — DB is source of truth for reminders

**Decision:** Windows schedule 是可重建 side effect。

**Reason:** DB 与 OS schedule 无法真正跨系统原子提交；任务数据不能因通知 API 暂时失败而丢失。

## D-005 — No polling reminder loop

**Decision:** 使用 Windows Scheduled Notification；启动时做 missed-reminder in-app recovery。

**Reason:** 减少常驻 CPU wakeup；符合高性能工作机目标。

## D-006 — No recurring tasks in V1

**Decision:** 延后 V1.1。

**Reason:** 功能表面很小，但涉及 recurrence generation、edit scope、月底、reminder inheritance 等规则，显著放大上下文和测试面。

## D-007 — Quick Add is first-class

**Decision:** 全局热键和 Quick Add 属于 V1 release gate。

**Reason:** 软件价值不仅是四象限显示，更是降低任务记录摩擦。

## D-008 — Internal/unpackaged first

**Decision:** 开发阶段先使用 unpackaged WPF；Release Stage 再形成可部署 profile。

**Reason:** 先验证功能和 Windows integration，避免一开始把 MSIX/签名/部署复杂度混入核心开发。
