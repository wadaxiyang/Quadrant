# Stage 16 — Accessibility, DPI, Keyboard, Visual Polish

## Goal

不加新功能，只把已有功能收口成成熟 Windows 软件。

## Before coding — MUST browse

查 WPF accessibility / UI Automation、DPI、Keyboard navigation、High Contrast 当前官方指南。若需要 `AutomationProperties.Name` 等，按官方 API。

## Checklist implementation

### DPI / layout

实际测试：100 / 125 / 150 / 200%。

- 不出现固定高度文字截断；
- 2×2 在 920×620 最小尺寸仍可用；
- TaskEditor 不越屏；
- DatePicker/ComboBox 不裁切。

### Keyboard

- Tab 顺序；
- Enter/Esc；
- Ctrl+F；
- Ctrl+1..4 QuickAdd；
- Space/Enter 完成任务；
- Focus visual 保留。

### Accessibility names

Icon-only button MUST `AutomationProperties.Name` + Tooltip。

Task Card 对 screen reader 暴露 title + due status，不只 accent color。

### High Contrast

- 不 hardcode text/background；
- quadrant accent 不是唯一信息；
- Overdue 有文字/semantic cue。

### Visual cleanup

按 `DESIGN_SYSTEM.md` 审计：

- magic numbers；
- repeated style；
- gradients；
- DropShadowEffect；
- emoji icons；
- inconsistent radius/spacing；
- overly dense card。

### Localization

V1 不做多语言框架。但所有用户文本不要散落在业务 service；至少集中 UI resource/string location，便于未来国际化。不要在此 Stage 引 resx 大工程，除非已有明确需要。

## Acceptance

产生一个 `docs/V1_UI_ACCEPTANCE.md`，逐项标 Pass/Fail/Not tested，不能口头“应该没问题”。

## DO NOT

- 不改功能范围；
- 不加动画；
- 不换主题库。

## Handoff

STATUS 列未通过的视觉/可访问性问题。下一 Stage 17。
