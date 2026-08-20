# Stage 02 — Fixed Design Tokens and Reusable WPF Styles

## Goal

把 `DESIGN_SYSTEM.md` 落成 ResourceDictionary，让后续 Luna **只能复用 token，不再自行创造 UI 数字**。

## Before coding — MUST browse

查 WPF ResourceDictionary、StaticResource/DynamicResource、Fluent theme system resources 的当前官方文档。确认哪些系统 brush 名称可安全引用；不要凭记忆猜资源 key。

## Files

建议：

```text
Quadrant.App/Resources/
  Spacing.xaml
  Typography.xaml
  QuadrantColors.xaml
  ControlStyles.xaml
```

在 `App.xaml` merge；顺序要保证自定义资源能引用主题资源且不会覆盖整个 Fluent 控件模板。

## Implementation

### Tokens

固定：

- spacing 4/8/12/16/24/32；
- radius 4/8；
- typography 20/16/14/12；
- quadrant accents exactly from `DESIGN_SYSTEM.md`。

### Reusable styles

只做本项目真正会用的基础 style：

- `QuadrantPanelStyle`
- `TaskCardStyle`
- `SectionTitleTextStyle`
- `CaptionTextStyle`
- `IconButtonStyle`（尽量 BasedOn native button style，而不是重画完整模板）

TaskCard 本阶段只做空视觉示例，不接真实数据。

### Resource discipline

业务 View 中不允许出现大段硬编码：

```text
Margin="13"
FontSize="17"
Background="#..."
```

四象限 accent 之外，背景/文本/边框来自当前 WPF Fluent/System resources。

## Acceptance

- Light/Dark 下 tokens 生效；
- 四象限 accent 只小面积出现；
- MainWindow XAML 中不再有重复 magic numbers；
- Styles 不使用 DropShadowEffect；
- High Contrast 至少不出现白字白底/黑字黑底明显问题。

## DO NOT

- 不引 icon library；
- 不自绘全套 Button/TextBox template；
- 不加渐变；
- 不加第三方 theme。

## Handoff

STATUS 写明 Resource 文件及关键 system resource 选择。下一阶段 Stage 03。
