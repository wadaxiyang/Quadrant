# Stage 01 — .NET 10 WPF Native Fluent Shell

## Goal

得到能启动的 WPF 主窗口壳，并确认**原生 Fluent Theme、System/Light/Dark 切换基础**工作。此阶段不做真正任务列表。

## Before coding — MUST browse

重新查 Microsoft Learn：

- WPF .NET 10 What's New；
- WPF .NET 9+ Fluent Theme / `ThemeMode`；
- `Application.ThemeMode` 当前 API 语法。

不要用旧文章中 Aero/第三方 Fluent 方案代替。

## Technical implementation

### App startup

`App.xaml` 不引第三方 ResourceDictionary。优先直接使用当前官方推荐的 `ThemeMode` 方式，例如 System；若当前 .NET 10 API 与文档不同，以最新官方示例为准。

建立：

```text
Quadrant.App/
  App.xaml
  App.xaml.cs
  Views/MainWindow.xaml
  Views/MainWindow.xaml.cs
  ViewModels/MainViewModel.cs
```

### MainWindow shell

只做骨架：

- 默认 1180×760；
- MinWidth 920；MinHeight 620；
- 顶部 56 左右工具区域；
- 主体临时放一个 2×2 Grid，每格显示 Q1/Q2/Q3/Q4 placeholder；
- 四格等宽等高；
- 不自定义卡片 Style。

### MVVM

`MainViewModel` 使用 `ObservableObject` / `[ObservableProperty]`，只暴露当前 app title、placeholder title 等最小数据，证明 Binding 工作。

### Composition

`App.xaml.cs` 手工 new MainViewModel → MainWindow DataContext。不要 DI container。

## Acceptance

手工验证：

1. Windows 11 System Light 下启动，控件观感为 WPF Fluent；
2. 系统切 Dark 后重新启动/按当前 API 可见更新，主题正确；
3. 125/150% DPI 主框架无明显裁切；
4. resize 到最小尺寸仍保持 2×2；
5. 无第三方 theme package。

自动：`dotnet build` + `dotnet test`。

## DO NOT

- 不做 Design Tokens；
- 不做任务卡片；
- 不做数据库；
- 不做 Mica；
- 不自绘 title bar；
- 不为“现代感”加入动画。

## Handoff

STATUS 记录实际 Fluent Theme API 写法和官方 URL。下一阶段 Stage 02。
