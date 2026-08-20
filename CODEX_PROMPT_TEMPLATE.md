# Codex / Luna Stage Prompt Template

建议每次开一个相对独立的 Codex 任务，只给一个 Stage。

## 标准提示词

```text
请严格按本仓库 AGENTS.md 工作。

本次只执行：stages/STAGE_XX_XXXXXXXX.md

开始前必须依次读取：
1. AGENTS.md
2. SPEC.md
3. ARCHITECTURE.md
4. DESIGN_SYSTEM.md
5. STATUS.md
6. 当前 Stage 文件

要求：
- 不得进入下一 Stage；
- 涉及 .NET/WPF/NuGet/Windows API/Windows App SDK 时，必须联网查当前官方文档，不得只依靠模型记忆；
- 优先 Microsoft Learn、NuGet 官方、官方 GitHub/release；
- 实现后必须 build/test，并执行当前 Stage 的 manual acceptance；无法在当前环境完成的手测必须写入 STATUS.md，不得假装通过；
- 完成后更新 STATUS.md，记录 files changed、tests、sources checked、known issues、next stage；
- 不得增加 SPEC 外功能；
- 不得修改固定 Design System；
- 最终回复只汇报本 Stage，不继续开发下一阶段。
```

## 修 Bug 提示词

如果一个 Stage 验收失败，不要让 Luna直接进入下一阶段：

```text
仍然停留在 Stage XX。以下验收项失败：……
请先读取 AGENTS.md、STATUS.md 和 Stage XX，只修复这些失败项。
不要实现任何下一阶段功能。修复后重新运行 Stage XX 的全部相关测试，并更新 STATUS.md。
```

## UI 偏差提示词

```text
不要重新设计。请依据 DESIGN_SYSTEM.md 做视觉一致性审计，只修复违反固定 token、spacing、typography、radius、Fluent resource、DPI 与 accessibility 规则的问题。不得引入第三方 UI 库。
```
