# Quadrant WPF Codex Development Pack

> 工作代号：**Quadrant**。后续可改名；改名不得改变本包已经固定的产品边界与技术架构。

这是一套面向 **Codex / Luna 分阶段开发**的仓库级文档。目标不是一次性让模型“把整个软件写完”，而是把项目拆成足够短、足够明确、能独立验收的小阶段，降低长上下文注意力衰减带来的架构漂移和 UI 失控。

## 文档阅读顺序

Codex 每次进入仓库后必须按以下顺序读取：

1. `AGENTS.md` — 全局行为规则，优先级最高。
2. `SPEC.md` — 产品功能、非目标和 V1 验收边界。
3. `ARCHITECTURE.md` — 固定技术栈、模块边界、依赖方向、数据模型。
4. `DESIGN_SYSTEM.md` — 固定 UI 设计系统，禁止自由发挥。
5. `STATUS.md` — 上一次阶段结束后的项目状态和交接信息。
6. `STAGE_INDEX.md` — 找到当前应执行的 Stage。
7. `stages/STAGE_XX_*.md` — **只执行当前一个 Stage**。
8. 需要版本/API 信息时查 `REFERENCES.md`，并按 `AGENTS.md` 要求重新联网核验。

## 核心原则

**Quadrant 不是 TickTick 的复制品。**

它只解决一个闭环：

**快速记录任务 → 放入四象限 → 安排截止/提醒 → Windows 到点提醒 → 完成/延后/打开 → 归档。**

V1 明确不做：账号、云同步、AI、团队协作、复杂项目管理、标签体系、附件、Markdown、日历月视图、番茄钟、习惯打卡、统计报表、插件、移动端。

## 推荐执行方式

每次只把一个 Stage 交给 Luna，例如：

> 按仓库 AGENTS.md 执行，仅完成 `stages/STAGE_04_MAIN_QUADRANT_VIEW.md`。开始前读取 SPEC、ARCHITECTURE、DESIGN_SYSTEM、STATUS；涉及版本或 Windows API 时必须联网查官方文档。完成后运行该 Stage 的测试/验收，并更新 STATUS.md，不得进入下一 Stage。

不要一次要求 Luna 连续做多个 Stage。

## 本包生成依据

本方案以 **2026-08-20** 可核验的微软官方资料为基线，重点确认了：

- .NET 10 WPF 继续维护，并改进 Fluent 样式与性能；
- WPF .NET 9+ 可直接使用原生 Fluent Theme 与 `ThemeMode`；
- WPF/.NET 可通过 Windows App SDK 使用本地 App Notifications；
- unpackaged WPF 的 AppNotification 注册流程已有官方支持；
- Scheduled App Notification 可以在应用未运行时按计划显示，但投递窗口约 5 分钟；
- Win32 `RegisterHotKey` 用于系统级快捷键；
- `System.Windows.Forms.NotifyIcon` 可用于 Windows 通知区域图标；
- CommunityToolkit.Mvvm 与 Microsoft.Data.Sqlite 均有稳定 .NET 10 可用版本。

所有版本敏感信息仍必须在实际开发阶段重新联网确认。

## 当前应用

Quadrant 当前包含四象限任务、Due/Reminder、Today/Overdue、搜索、已完成列表、Quick Add、全局热键、Windows 通知、托盘、开机启动和 Light/Dark/System 主题。

开发运行：

```powershell
dotnet run --project .\src\Quadrant.App\Quadrant.App.csproj
```

发布与运行前置条件见 [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)。
