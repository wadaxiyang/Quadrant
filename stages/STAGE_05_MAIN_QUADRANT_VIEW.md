# Stage 05 — Real Main 2×2 Quadrant View

## Goal

把 SQLite active tasks 真正显示为 2×2 四象限。此阶段主要是**读取与显示**，不要把 CRUD、提醒、拖拽一起塞进来。

## Technical implementation

### Startup composition

`App.xaml.cs`：

1. init DB；
2. create repositories；
3. create `SystemClock` + `NoOpReminderScheduler`；
4. create TaskService；
5. create MainViewModel；
6. load active tasks async；
7. show MainWindow。

UI 线程不要同步 `.Result` / `.Wait()` 阻塞异步 DB 操作。

### ViewModels

- `MainViewModel`
- `QuadrantViewModel`
- `TaskCardViewModel`

MainVM 持有 4 个固定 QuadrantVM；每个有 ObservableCollection task cards。

不要让 entity 本身成为 WPF observable model。

### XAML

四个象限用明确 Grid row/column，不用 ItemsControl 自动生成 4 格，避免布局随机。

每个象限任务列表使用 `ListBox` / `ListView`：

- `ScrollViewer.CanContentScroll=true`
- VirtualizingStackPanel virtualization/recycling（核验 WPF 当前属性写法）
- DataTemplate 引用 `TaskCardStyle`

空状态：一句 Caption 文本。

### Sort

象限内 active task 默认排序：

1. DueAt non-null before null；
2. DueAt ascending；
3. CreatedAt ascending。

排序逻辑集中一个 comparer/service，别散落 XAML converter。

## Acceptance

准备含多个象限测试数据的 temp/dev DB，验证：

- 4 格显示正确；
- 任务不会串象限；
- due 信息格式正常；
- resize 仍保持四格；
- 100+ synthetic tasks 时列表独立滚动，不扩大整个窗口；
- 无 CRUD 按钮功能也可以，当前 Stage 只显示。

## DO NOT

- 不做 drag/drop；
- 不做 filter/search；
- 不做 reminder；
- 不在 View 构造函数里直接 query SQLite。

## Handoff

STATUS 写数据加载路径与 VM 结构。下一阶段 Stage 06。
