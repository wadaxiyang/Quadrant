# Stage 10 — Windows App SDK Foundation and Deployment Mode Verification

## Goal

只把 Windows App SDK 正确接入 **unpackaged WPF .NET 10**，确认 runtime 初始化方式和 Release 方向。暂不实现真实通知业务。

## Before coding — MUST browse fresh official docs

必须查当日最新：

- Windows App SDK stable release/downloads；
- WPF/.NET app notification quickstart；
- unpackaged deployment / bootstrapper；
- self-contained deployment；
- 当前 `Microsoft.WindowsAppSDK` NuGet stable package version。

**不要把“Windows App SDK runtime 2.3.1”直接假设成 NuGet package version。** 以官方 release/nuget 为准。

## Package / csproj

在 `Directory.Packages.props` pin verified stable `Microsoft.WindowsAppSDK`。

App/Infrastructure 按需要引用。

开发期保持 unpackaged，按官方推荐设置例如：

```xml
<WindowsPackageType>None</WindowsPackageType>
```

但确切 MSBuild 属性必须依据本 Stage 查到的官方 2026 文档。

### Smoke service

做一个 `WindowsAppSdkEnvironmentProbe`（仅诊断/测试，不是用户功能）确认 app 启动能访问需要的 WinRT/WASDK API，不抛 REGDB_E_CLASSNOTREG。

不要在 UI 留“测试 SDK”按钮；可用 DEBUG-only command 或 test harness。

## Deployment decision record

在 STATUS 记录：

- 当前开发机 WASDK runtime 是否已装；
- framework-dependent debug 如何启动；
- self-contained release 需要哪些属性；
- V1 最终是否能做 folder deployment。

不要在本 Stage 做 MSIX。

## Acceptance

- Debug app 正常启动；
- 不出现 SDK initialization error；
- build/test 全绿；
- 包版本与官方来源已记录。

## DO NOT

- 不实现通知 UI；
- 不实现 schedule；
- 不做 package identity；
- 不做 Store/MSIX；
- 不改 WPF UI framework。

## Handoff

下一 Stage 11。
