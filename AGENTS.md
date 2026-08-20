# AGENTS.md — Mandatory Rules for Codex / Luna

本文件是仓库级最高优先级工程约束。**MUST / MUST NOT / DO NOT** 均为强制规则。

## 1. 每次工作前的读取顺序

在改任何代码前，MUST 按顺序读取：

1. `AGENTS.md`
2. `SPEC.md`
3. `ARCHITECTURE.md`
4. `DESIGN_SYSTEM.md`
5. `STATUS.md`
6. 当前 `stages/STAGE_XX_*.md`

如果当前 Stage 未明确指定，MUST 停止开发，只报告当前状态与建议的下一 Stage；不得自行挑多个 Stage 连做。

## 2. 一次只做一个 Stage

- MUST 只实现当前 Stage 明确列出的内容。
- MUST NOT “顺手实现”下一阶段功能。
- MUST NOT 在一个 Stage 中做大规模重构，除非当前 Stage 明确要求。
- 当前 Stage 验收未通过时，MUST 修复后再结束；不得继续下一个 Stage。
- 用户明确要求跨 Stage 时才允许跨越。

这是为了控制 Luna 长上下文注意力衰减。

## 3. 必须联网查证，不得只依靠模型记忆

### 3.1 强制联网的情况

遇到以下任何内容，**MUST 先联网查当前官方文档**：

- .NET / WPF 版本行为；
- Fluent Theme / `ThemeMode`；
- NuGet 包最新稳定版本、兼容性、弃用信息；
- Windows App SDK；
- App Notifications / Scheduled Notifications；
- Win32 API（如 `RegisterHotKey`）；
- 打包、部署、启动项；
- 任何编译错误显示 API 与记忆不一致；
- 任何“我记得应该这样写”的 Windows 平台细节。

### 3.2 来源优先级

必须按以下优先级：

1. `learn.microsoft.com`
2. `dotnet.microsoft.com` / `nuget.org` 官方包页
3. 官方 GitHub 仓库 / release notes
4. 其他可信资料仅用于补充

DO NOT 以博客、Stack Overflow、随机教程作为版本敏感 API 的唯一依据。

### 3.3 查证后记录

每个 Stage 结束时在 `STATUS.md` 的 `Sources checked` 中记录实际查阅的官方 URL 与日期。

如果官方文档与本包写法发生冲突：

- MUST 以当前官方文档为准；
- MUST 在 `STATUS.md` 记录差异；
- 若差异会改变产品架构或 SPEC，MUST 停止并向用户说明，不能静默改产品设计。

## 4. 技术栈不得漂移

V1 固定：

- C#
- .NET 10
- WPF
- WPF 原生 Fluent Theme
- CommunityToolkit.Mvvm
- Microsoft.Data.Sqlite
- Windows App SDK 仅用于需要的 Windows 能力（主要是通知）
- Win32 P/Invoke 仅用于没有合适托管 API 的小范围系统集成

MUST NOT 未经用户批准替换成：

- WinUI 3
- MAUI
- Avalonia
- Electron
- Tauri
- Flutter
- Qt
- Slint
- React/WebView
- EF Core

## 5. UI 设计禁止自由发挥

所有 UI MUST 遵守 `DESIGN_SYSTEM.md`。

DO NOT：

- “让界面更炫”；
- 自创 Design Language；
- 引入第三方 UI 主题库；
- 使用渐变背景；
- 大面积高饱和象限色；
- 过度阴影；
- 玻璃拟态；
- 无限圆角；
- 随意加入动画；
- Canvas/绝对定位搭主界面；
- 到处硬编码 Margin、FontSize、颜色；
- 用 emoji 代替正式 UI 图标。

如果视觉要求不明确，MUST 选择**更克制、更接近 Windows 11 Fluent 默认行为**的方案。

## 6. MVVM 与代码边界

### 6.1 ViewModel

优先使用 CommunityToolkit.Mvvm source generators：

- `[ObservableProperty]`
- `[RelayCommand]`

ViewModel 不得直接操作：

- SQLite；
- Win32；
- Windows App SDK；
- `MessageBox` 以外的平台 UI 细节；
- 文件系统路径拼接。

### 6.2 Code-behind 允许范围

WPF code-behind 仅允许处理纯 View / HWND 事件，例如：

- DragDrop 事件转发；
- Window source / message hook；
- Focus；
- 键盘事件转发；
- Window lifecycle 的平台桥接。

业务逻辑必须调用 ViewModel / Service。

### 6.3 Core

`Quadrant.Core` MUST 不引用：

- WPF；
- Windows App SDK；
- WinForms；
- SQLite 实现包；
- Win32。

## 7. 数据库规则

- MUST 使用 `Microsoft.Data.Sqlite`，不引入 EF Core。
- MUST 使用参数化 SQL，禁止字符串拼 SQL 值。
- MUST 有 `schema_version` / migration 机制。
- MUST 在多语句写操作中使用 transaction。
- 数据库是任务与提醒的业务事实来源。
- 任何 Windows Notification schedule 都视为可重建的外部派生状态。

## 8. 时间规则

- 代码内部统一使用 `DateTimeOffset` 处理有具体时刻的 Due/Reminder。
- 数据库存储使用稳定、可逆格式；优先 ISO-8601 文本或明确的 UTC epoch 方案，但全项目只能选一种。
- UI 显示为 Windows 当前本地时区。
- 禁止把 `DueAt` 与 `ReminderAt` 合成一个字段。
- 不实现自然语言日期解析。

## 9. 通知规则

- MUST 使用 Windows 官方 App Notification / Scheduled Notification API。
- MUST NOT 用隐藏窗口 + Timer 模拟系统提醒。
- MUST NOT 创建每秒或每分钟 Reminder polling loop。
- Notification action 必须通过稳定 task id 解析。
- 编辑/删除/完成任务后必须处理旧 schedule，不能遗留幽灵提醒。
- Scheduled Notification 的 5 分钟投递窗口必须在代码注释/文档中保留事实说明。

## 10. 性能规则

MUST NOT：

- 后台网络请求；
- telemetry；
- analytics；
- 每秒刷新全界面；
- 空闲状态周期性 DB query；
- 高频 DispatcherTimer；
- 阴影 Effect 大量用于列表卡片；
- 为少量任务关闭列表虚拟化。

列表优先 `ListBox/ListView + VirtualizingStackPanel + Recycling`。

性能 Stage 前不得为了“优化”引入复杂缓存框架；先测量再改。

## 11. 错误处理

- 用户可恢复错误：显示清晰、短的 UI 提示。
- 系统集成失败（热键冲突、通知注册失败、开机启动写入失败）不能导致应用崩溃。
- DB 初始化/迁移失败属于高优先级错误，必须阻止继续写入并给出可诊断日志。
- 不允许 catch 后静默吞异常。

## 12. 依赖管理

- 使用 `Directory.Packages.props` 集中固定 NuGet 版本。
- 只使用 stable 版本，除非用户明确要求 preview。
- 添加任何新依赖前，MUST 说明：为什么标准库/现有依赖不够。
- 一个 Stage 不得无理由添加多个第三方包。
- 包版本必须在联网核验后固定。

## 13. 测试与验收

每个 Stage：

1. MUST `dotnet build`；
2. 有单元测试时 MUST `dotnet test`；
3. MUST 执行该 Stage 文件中的 Manual Acceptance；
4. 不允许以“看起来应该可以”为验收。

如果本环境无法完成 Windows GUI 手动测试，必须：

- 完成可执行的自动测试；
- 把未完成的手工测试逐条写到 `STATUS.md`；
- 不谎称通过。

## 14. Stage 结束必须更新 STATUS.md

每个 Stage 结束时 MUST 更新：

- Current stage；
- Completed；
- Files changed；
- Architecture decisions / deviations；
- Tests run + results；
- Manual tests pending；
- Sources checked；
- Known issues；
- Exact next stage。

`STATUS.md` 必须保持简洁，建议不超过约 180 行；它是下一次 Luna 会话的上下文交接文件。

## 15. 禁止事项

除非用户明确批准，MUST NOT：

- 修改 SPEC 的产品边界；
- 添加云/账号/AI；
- 添加自动更新；
- 添加重复任务；
- 添加复杂 Tag/Project；
- 改用第三方 UI framework；
- 删除测试来让 CI 通过；
- 用 placeholder/stub 冒充已实现功能；
- 在用户未要求时重写整个项目；
- 把所有业务逻辑堆到 `MainWindow.xaml.cs`。

## 16. 完成回复格式

每次完成一个 Stage 后，最终回复必须简洁给出：

- 完成的 Stage；
- 关键实现；
- build/test 结果；
- 未验证项；
- 更新过的 `STATUS.md`；
- 下一 Stage 文件名。

不得主动继续下一 Stage。
