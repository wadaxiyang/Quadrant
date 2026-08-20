# Stage 17 — Performance, Reliability, Diagnostics

## Goal

在发布前用**测量而不是感觉**确认工作机常驻开销与故障行为。

## Before coding — MUST browse

查微软当前可用的 .NET 10 diagnostics：`dotnet-counters`, `dotnet-trace`/Visual Studio Profiler 或 Windows Performance Recorder。只需选能在当前机器执行的工具。

## Performance audit

### No polling

代码搜索确认：

- 无业务 `DispatcherTimer` 循环；
- 无 `System.Threading.Timer` reminder polling；
- 无 background Task while(true)；
- 无网络 client package。

### List virtualization

确认 ListBox virtualization 实际未被 ScrollViewer nesting 破坏。创建 synthetic 1000 task dev data 测：

- startup/load；
- scroll；
- filter；
- search；
- drag/drop。

### Idle measurement

至少记录：

- 主窗口 visible idle 60s CPU；
- tray-only idle 60s CPU；
- private working set / managed heap baseline；
- cold start subjective + measurable time（可用 Stopwatch 日志或 profiler）。

目标：CPU 长时间接近 0；SPEC 目标平均 <0.2% 是工程目标，不达标先找周期工作来源。

不要为了一个内存数字引复杂缓存/NativeAOT。

## Reliability

### SQLite

- DB directory missing → recreate；
- DB locked briefly → clear error/retry policy only if justified；
- migration failure → stop writes + diagnostic；
- invalid/corrupt DB → 不自动删除用户数据；给出路径并建议备份/修复。

### System integration failures

模拟/处理：

- RegisterHotKey conflict；
- Notification register fails；
- Scheduled notification fails；
- startup registry access denied；
- tray icon resource missing（build test should catch）。

功能降级但 app 不崩，DB 保存优先。

### Logging

轻量 rolling/log file or simple append logger：

- `%LOCALAPPDATA%\Quadrant\logs\`
- 默认只记录 warning/error/system integration；
- 不 telemetry；
- 不上传；
- 设置合理文件大小/保留策略，避免无限增长。

若引 logging package，必须先解释必要性；优先简单自有 logger 或 .NET logging abstractions 不加复杂 host。

## Acceptance artifact

创建 `docs/V1_PERFORMANCE.md`：机器、build、task count、CPU/memory/startup measurements、发现与修复。

所有数字必须实测，未测写 `Not measured`。

## DO NOT

- 不声称“0 MB”；
- 不做 micro-optimization 破坏可读性；
- 不加入 telemetry。

## Handoff

STATUS 标明 release blockers。下一 Stage 18。
