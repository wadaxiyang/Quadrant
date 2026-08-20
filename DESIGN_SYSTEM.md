# Quadrant Fixed Design System

本文件用于限制 Codex/Luna 的 UI 自由度。**V1 不允许重新设计这套规则。**

## 1. Design Language

- 基础：**Windows 11 Fluent / WPF 原生 Fluent Theme**。
- .NET：10。
- 主题：优先 `ThemeMode="System"`；设置中可选 Light / Dark。
- 不引入 WPF UI、MaterialDesignInXaml、HandyControl 等第三方主题库。
- 目标观感：安静、平整、克制、桌面工具感，不做网页 Dashboard 感。

## 2. Typography

优先系统字体与 Fluent 默认字体资源；需要明确时使用 `Segoe UI Variable` / 系统 UI 字体。

固定层级：

| Token | Size | Weight | 用途 |
|---|---:|---|---|
| `TitleLarge` | 20 | SemiBold | 窗口/页面主标题 |
| `TitleMedium` | 16 | SemiBold | 象限标题、对话框标题 |
| `Body` | 14 | Normal | 任务标题、普通输入 |
| `BodyStrong` | 14 | SemiBold | 需要强调的任务信息 |
| `Caption` | 12 | Normal | Due、Reminder、副标题 |

禁止在业务 XAML 中随意出现新的 FontSize 数字。

## 3. Spacing

只允许以下基础间距：

`4 / 8 / 12 / 16 / 24 / 32`

建议资源名：

- `SpaceXS = 4`
- `SpaceS = 8`
- `SpaceM = 12`
- `SpaceL = 16`
- `SpaceXL = 24`
- `SpaceXXL = 32`

禁止 5、7、13、17、19 等“AI 魔数”。

## 4. Corner Radius

只允许：

- `RadiusSmall = 4`
- `RadiusMedium = 8`

Task Card：8。

主窗口不自绘夸张圆角。

## 5. Color

背景、文本、边框、Hover、Selection 优先使用 WPF Fluent / System Resource。

自定义颜色只允许用于四象限 Accent，且只作为细线、圆点或小面积识别符：

- Q1：`#D13438`
- Q2：`#0F6CBD`
- Q3：`#CA5010`
- Q4：`#6B6B6B`

在 Dark/High Contrast 下，如果固定 Accent 可读性不足，可通过资源覆盖调整，但不能让整块卡片铺色。

## 6. Task Card

标准：

- Radius 8；
- Padding 12；
- 卡片间距 8；
- 3px Accent indicator；
- 正常状态不显示重阴影；
- Hover 只改变 Fluent surface/border，不做位移动画；
- 完成状态移出活跃象限，不长期以删除线堆在原列表中。

常态内容：

1. Complete toggle
2. Title
3. Due（若有）
4. Reminder（若有）

Hover 才显示 Edit / More。

## 7. Main Window

- 默认：1180 × 760；
- 最小：920 × 620；
- 主体严格 2 rows × 2 columns；
- 四象限等宽等高；
- 象限之间 12–16px 间距；
- 每个象限内部任务列表独立滚动；
- 不用 Canvas。

顶部工具区：

- 左：应用名；
- 中/左：All / Today / Overdue；
- 右：Search / Completed / Settings。

不做固定宽度左侧 NavigationRail。

## 8. Quadrant Panel

每个象限：

- Title 16 SemiBold；
- Subtitle 12；
- 右侧可显示当前任务数；
- 下方 ListBox；
- 空状态只显示一行轻提示，不放大插画。

## 9. Task Editor

建议尺寸：560 × 520（允许随内容微调）。

字段顺序不可变：

1. Title
2. Quadrant
3. Due date + time
4. Reminder
5. Note
6. Cancel / Save

所有 label 左对齐。不要双栏复杂表单。

## 10. Quick Add

建议：520 × 260 左右。

- 打开后 Title 自动聚焦；
- 象限按钮明确标 `Q1..Q4` + 名称；
- 时间区域默认折叠；
- Enter 保存，Esc 关闭；
- 禁止做成大型完整编辑器。

## 11. Date / Time Input

- Date：原生 WPF `DatePicker`（.NET 10 Fluent 已覆盖更多控件）。
- Time：`ComboBox IsEditable=true`；
- 候选以 15 分钟为步长；
- 接受 `9:05` / `09:05`；
- 解析失败在控件附近显示 validation，不用 MessageBox。

## 12. Filter

All / Today / Overdue 使用三个紧凑 Toggle/Radio 风格控件。

它们只改变过滤条件，不导航到另一个“页面”。

## 13. Animation

只允许：

- Fluent 控件自带状态过渡；
- Drag feedback；
- 对话框/窗口系统默认动画。

禁止：

- 卡片飞入；
- 背景动画；
- gradient animation；
- 页面切换大动画；
- 连续 pulse。

## 14. Shadow / Effects

- Task list 中禁止 `DropShadowEffect` 批量使用；
- 不做玻璃拟态；
- 如主窗口使用 Windows 系统级背景效果，必须单独验证性能和兼容性，V1 默认不要求 Mica。

## 15. Icons

- 优先 Windows/Fluent 风格矢量图标；
- 图标数量尽量少；
- 不用 emoji 当功能图标；
- 图标必须有 Tooltip/AutomationName（仅图标按钮）。

## 16. Accessibility / DPI

必须：

- 125%、150%、200% DPI 不错位；
- 不用固定像素高度把文本截断；
- 键盘 Tab 顺序合理；
- Focus visual 不删除；
- High Contrast 至少可用；
- 不只靠颜色表达 Overdue / Quadrant。

## 17. XAML Token Rule

所有共用的：

- Brush；
- Font size/style；
- Margin/Padding；
- Radius；
- 控件 Style；

必须集中在 `Resources/` 下 ResourceDictionary。

业务 View 中禁止复制粘贴成套 Style。
