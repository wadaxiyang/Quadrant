# Stage 18 — Release Packaging and Final V1 Gate

## Goal

不再加功能，只做可重复 Release、部署说明、最终 V1 Gate。

## Before coding — MUST browse

重新查当日最新官方：

- .NET 10 publish / self-contained；
- Windows App SDK framework-dependent vs self-contained；
- unpackaged app deployment；
- single-file 当前约束；
- 若考虑 MSIX，先读 packaging/signing 要求，但 V1 不强制 MSIX。

## Release profiles

至少产生一个**真正能在目标 Windows 11 机器运行通知/托盘/热键**的 profile。

建议评估两个：

### A. Internal framework-dependent

适合自己的工作机：

- smaller app output；
- .NET 10 runtime / Windows App SDK runtime prerequisite 清晰记录；
- 用官方 runtime installer 或已安装 runtime。

### B. Self-contained folder

- .NET self-contained；
- Windows App SDK self-contained（按当前官方属性）；
- 输出会更大，但不要求用户预装对应 runtime；
- 不强求单 exe。

实际哪个作为 V1 默认由测试结果决定，写到 `docs/DEPLOYMENT.md`。

### Single-file

只有在当前官方明确支持 WPF + 所用 Windows App SDK 组合且实测通知 activation/scheduled notification 均正常时才启用。

**稳定正确 > 一个 exe。**

## Release build

- `Release` build/test；
- portable clean-machine/fresh-user smoke；
- DB migration from Stage 04/15 schema；
- startup path；
- notification when app closed；
- single instance；
- tray exit；
- hotkey；
- theme；
- 1000 task performance sanity。

## V1 Gate

逐条检查 `SPEC.md §6`。输出 `docs/V1_RELEASE_CHECKLIST.md`：每项 Pass/Fail/Not Tested。

任何 Fail 的 release-gate 项都不能把版本标为 1.0.0。

## Version

建议初始：

- development: `0.1.0`…
- feature complete RC: `0.9.0`
- gate all pass: `1.0.0`

不要在本 Stage 加 updater。

## Documentation

最终仓库至少：

- README：功能、截图位置、build、run；
- DEPLOYMENT；
- V1_RELEASE_CHECKLIST；
- V1_PERFORMANCE；
- license（由用户决定开源许可证；没有用户决定前不要擅自写 MIT）。

## Acceptance

在一台目标 Windows 11 环境完成最终手测并记录。不具备 clean machine 时必须写 Not Tested，不能虚构。

## DO NOT

- 不加最后一分钟新功能；
- 不引 auto updater；
- 不改 architecture；
- 不为了安装包尺寸删除 notification runtime 依赖。

## Final handoff

STATUS：`V1 READY` 或明确 release blockers。不要擅自开始 V1.1。
