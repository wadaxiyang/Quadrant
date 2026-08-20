# Stage 00 — Repository Bootstrap and Version Pinning

## Goal

只建立一个**可重复构建、版本明确、边界正确**的空解决方案。不要做业务 UI，不要做数据库，不要做通知。

## Before coding — MUST browse

重新查：

- .NET 10 SDK 当前 stable patch；
- `CommunityToolkit.Mvvm` NuGet 当前 stable；
- `Microsoft.Data.Sqlite` 10.x 当前 stable；
- xUnit / Microsoft.NET.Test.Sdk 当前 stable；
- WPF .NET 10 target framework 官方说明。

官方来源优先 Microsoft Learn / NuGet。把实际版本写入 `Directory.Packages.props`，并在 `STATUS.md` 记录 URL。

## Technical implementation

创建：

```text
Quadrant.sln
global.json
Directory.Packages.props
src/Quadrant.Core/Quadrant.Core.csproj
src/Quadrant.Infrastructure/Quadrant.Infrastructure.csproj
src/Quadrant.App/Quadrant.App.csproj
Tests/Quadrant.Core.Tests/Quadrant.Core.Tests.csproj
Tests/Quadrant.Infrastructure.Tests/Quadrant.Infrastructure.Tests.csproj
```

### Target frameworks

- `Quadrant.Core`: `net10.0`
- `Quadrant.Core.Tests`: `net10.0`
- `Quadrant.Infrastructure`: `net10.0-windows10.0.19041.0`
- `Quadrant.Infrastructure.Tests`: same Windows TFM
- `Quadrant.App`: `net10.0-windows10.0.19041.0`, `UseWPF=true`

`Core` 不得带 `-windows`，这是架构约束测试的一部分。

### Package policy

使用 Central Package Management：

```xml
<ManagePackageVersionsCentrally>true</ManagePackageVersionsCentrally>
```

Stage 00 只允许引入：

- `CommunityToolkit.Mvvm`（App/Core 按需要）；
- `Microsoft.Data.Sqlite`（Infrastructure；本 Stage 可先 pin 但不要写 DB）；
- test packages。

**不要添加 `Microsoft.WindowsAppSDK`**，它在 Stage 10 单独联网核验并引入。

### Build defaults

建议在 `Directory.Build.props`（可选创建）固定：

- Nullable enabled；
- ImplicitUsings enabled；
- TreatWarningsAsErrors 可先 false，但 CI/release stage 再收紧；
- LangVersion 不指定 preview。

`global.json` pin 当前 .NET 10 SDK feature band，并允许 `latestPatch` roll-forward，避免无意义锁死一个已修补漏洞的 patch。

### Test smoke

每个 test project 写一个最小 smoke test，证明测试发现器工作。

## Acceptance

必须成功：

```powershell
dotnet --info
dotnet restore
dotnet build Quadrant.sln -c Debug
dotnet test Quadrant.sln -c Debug
```

检查项目依赖方向：

- Core 无 project reference；
- Infrastructure → Core；
- App → Core + Infrastructure；
- tests 只引用被测项目。

## DO NOT

- 不写 MainWindow 业务布局；
- 不写 SQLite schema；
- 不写 notification；
- 不写全局热键；
- 不创建 DI 容器；
- 不引第三方 UI 包。

## Handoff

更新 `STATUS.md`，记录：实际 SDK、所有 NuGet 版本、build/test 结果。下一阶段只能是 Stage 01。
